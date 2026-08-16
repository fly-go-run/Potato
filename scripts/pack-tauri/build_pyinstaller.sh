#!/usr/bin/env bash
# Build Potato backend with PyInstaller for Tauri sidecar
# Creates an onedir backend bundle with embedded Python runtime
#
# Usage:
#   ./scripts/pack-tauri/build_pyinstaller.sh
#
# Prerequisites:
#   - Python 3.10+ with virtual environment
#   - PyInstaller 6.0+ (will be installed if not present)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

DIST="${DIST:-dist}"
VERSION=$(sed -n 's/^__version__[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' src/potato/__version__.py)

BACKEND_DIR="${DIST}/pyinstaller/potato-backend"
BINARIES_DIR="${REPO_ROOT}/console/src-tauri/binaries"
DEST="${BINARIES_DIR}/potato-backend"
SIGNED_STAMP="${BINARIES_DIR}/.potato-backend-signed.stamp"
PENDING_STAMP="${BINARIES_DIR}/.potato-backend-pending.stamp"
REBUILT_MARKER="${BINARIES_DIR}/.potato-backend-rebuilt"
REUSED_MARKER="${BINARIES_DIR}/.potato-backend-reused"

rm -f "${REBUILT_MARKER}" "${REUSED_MARKER}"

echo "========================================="
echo "Potato PyInstaller Build"
echo "========================================="
echo "Version: ${VERSION}"
echo "Repository: ${REPO_ROOT}"
echo ""

# Check prerequisites
echo "== Checking prerequisites =="

# Create venv if missing (prefer uv if available)
PYTHON_BIN="${REPO_ROOT}/.venv/bin/python"
if [ ! -f "$PYTHON_BIN" ]; then
    if command -v uv &>/dev/null; then
        echo "Creating virtual environment with uv..."
        uv venv "${REPO_ROOT}/.venv"
    else
        echo "ERROR: Python not found in .venv"
        echo "Please create virtual environment first: python -m venv .venv"
        exit 1
    fi
fi

echo "Python: $("$PYTHON_BIN" --version)"

if [ ! -f "${REPO_ROOT}/app/dist/index.html" ]; then
    echo "ERROR: app/dist/index.html not found"
    echo "Build the app frontend first: (cd app && npm run build)"
    exit 1
fi

calculate_fingerprint() {
    {
        printf 'platform=%s/%s\n' "$(uname -s)" "$(uname -m)"
        printf 'python=%s\n' "$("$PYTHON_BIN" --version 2>&1)"
        printf 'codesign=%s\n' "${PYINSTALLER_CODESIGN_IDENTITY:-}"

        for input in \
            "pyproject.toml" \
            "uv.lock" \
            "scripts/pack-tauri/build_pyinstaller.sh" \
            "scripts/pack-tauri/potato.spec" \
            "scripts/pack-tauri/sign_macos_bundle.sh" \
            "scripts/pack-tauri/stage_python_runtime.py" \
            "scripts/pack-tauri/stage_node_runtime.py"; do
            if [ -f "${REPO_ROOT}/${input}" ]; then
                printf 'input=%s\n' "${input}"
                shasum -a 256 "${REPO_ROOT}/${input}"
            fi
        done

        for root in "${REPO_ROOT}/src/potato" "${REPO_ROOT}/app/dist"; do
            if [ -d "${root}" ]; then
                while IFS= read -r file; do
                    printf 'input=%s\n' "${file#"${REPO_ROOT}/"}"
                    shasum -a 256 "${file}"
                done < <(
                    find "${root}" -type f ! -path '*/__pycache__/*' -print |
                        LC_ALL=C sort
                )
            fi
        done
    } | shasum -a 256 | awk '{print $1}'
}

calculate_dependency_fingerprint() {
    {
        printf 'python=%s\n' "$("$PYTHON_BIN" --version 2>&1)"
        shasum -a 256 "${REPO_ROOT}/pyproject.toml"
        shasum -a 256 "${REPO_ROOT}/uv.lock"
        printf 'extras=local\n'
    } | shasum -a 256 | awk '{print $1}'
}

BACKEND_FINGERPRINT="$(calculate_fingerprint)"
BACKEND_CACHE_HIT=0
if [[ "${POTATO_FORCE_PYINSTALLER:-0}" != "1" &&
    -f "${SIGNED_STAMP}" &&
    "$(<"${SIGNED_STAMP}")" == "${BACKEND_FINGERPRINT}" &&
    -f "${DEST}/potato-backend" &&
    -f "${DEST}/potato" &&
    -f "${BACKEND_DIR}/potato-backend" &&
    -f "${BACKEND_DIR}/potato" ]]; then
    BACKEND_CACHE_HIT=1
    touch "${REUSED_MARKER}"
    echo "Reusing signed PyInstaller backend (inputs unchanged)"
fi

install_python_packages() {
    if command -v uv &>/dev/null; then
        uv pip install --python "$PYTHON_BIN" "$@"
    else
        "$PYTHON_BIN" -m pip install "$@"
    fi
}

uninstall_python_package() {
    if command -v uv &>/dev/null; then
        uv pip uninstall --python "$PYTHON_BIN" -y "$1" >/dev/null 2>&1 || true
    else
        "$PYTHON_BIN" -m pip uninstall -y "$1" >/dev/null 2>&1 || true
    fi
}

# Build PyInstaller only when the backend inputs changed. Frontend-only edits
# therefore reuse the already signed sidecar instead of rebuilding it.
if [ "${BACKEND_CACHE_HIT}" -eq 0 ]; then
    # Install PyInstaller if not present
    echo "== Installing PyInstaller =="
    if ! "$PYTHON_BIN" -c "import PyInstaller" 2> /dev/null; then
        echo "Installing PyInstaller..."
        install_python_packages "pyinstaller>=6.0.0"
    fi
    echo "PyInstaller installed"

    # Install project dependencies only when pyproject.toml or uv.lock changed.
    DEPENDENCY_STAMP="${REPO_ROOT}/.venv/.potato-pack-deps.stamp"
    DEPENDENCY_FINGERPRINT="$(calculate_dependency_fingerprint)"
    if [ ! -f "${DEPENDENCY_STAMP}" ] ||
        [ "$(<"${DEPENDENCY_STAMP}")" != "${DEPENDENCY_FINGERPRINT}" ]; then
        echo "== Installing project dependencies =="
        install_python_packages -e ".[local]"
        printf '%s\n' "${DEPENDENCY_FINGERPRINT}" > "${DEPENDENCY_STAMP}"
        echo "Project dependencies installed with local extras"
    else
        echo "Reusing Python dependencies (pyproject and uv.lock unchanged)"
    fi

    # Fix agent-client-protocol namespace collision
    # PyPI has an empty 'acp' stub that shadows the real package
    if ! "$PYTHON_BIN" -c "from acp import Agent" 2> /dev/null; then
        echo "Fixing agent-client-protocol namespace..."
        uninstall_python_package acp
        install_python_packages "agent-client-protocol>=0.9.0,<0.11.0"
        printf '%s\n' "${DEPENDENCY_FINGERPRINT}" > "${DEPENDENCY_STAMP}"
    fi

    # The repository .venv may have been used for a previous full desktop
    # build. Remove Whisper and its heavyweight dependency chain before
    # PyInstaller analyzes the environment.
    for package in openai-whisper torch numba llvmlite triton tiktoken; do
        uninstall_python_package "${package}"
    done
    echo ""

    # Run PyInstaller
    echo "== Running PyInstaller =="
    echo "Building onedir backend bundle..."

    SPEC_FILE="${REPO_ROOT}/scripts/pack-tauri/potato.spec"
    if [ ! -f "$SPEC_FILE" ]; then
        echo "ERROR: Spec file not found at ${SPEC_FILE}"
        exit 1
    fi

    PYINSTALLER_ARGS=(
        "${SPEC_FILE}"
        --distpath "${DIST}/pyinstaller"
        --workpath "${DIST}/pyinstaller-build"
        --noconfirm
    )
    if [[ "${POTATO_FAST:-0}" != "1" ]]; then
        PYINSTALLER_ARGS+=(--clean)
    else
        echo "Fast mode: retaining PyInstaller analysis cache"
    fi

    "$PYTHON_BIN" -m PyInstaller "${PYINSTALLER_ARGS[@]}"

    echo "PyInstaller build complete"
    echo ""
else
    echo "Skipping PyInstaller and dependency installation"
    echo ""
fi

for forbidden_dir in torch whisper; do
    if [ -d "${BACKEND_DIR}/_internal/${forbidden_dir}" ]; then
        echo "ERROR: Forbidden ${forbidden_dir} directory found in PyInstaller bundle"
        exit 1
    fi
done

# Verify output
BACKEND_EXE="${BACKEND_DIR}/potato-backend"
CLI_EXE="${BACKEND_DIR}/potato"
if [ ! -d "${BACKEND_DIR}" ]; then
    echo "ERROR: Backend bundle directory not found at ${BACKEND_DIR}"
    exit 1
fi
if [ ! -f "${BACKEND_EXE}" ]; then
    echo "ERROR: Backend executable not found at ${BACKEND_EXE}"
    exit 1
fi
if [ ! -f "${CLI_EXE}" ]; then
    echo "ERROR: CLI executable not found at ${CLI_EXE}"
    exit 1
fi

echo "Backend bundle created: ${BACKEND_DIR}"

# Get size
SIZE=$(du -sh "${BACKEND_DIR}" | cut -f1)
echo "Bundle size: ${SIZE}"
echo ""

if [ "${BACKEND_CACHE_HIT}" -eq 0 ]; then
    # Copy to Tauri resources directory
    echo "== Copying to Tauri binaries directory =="
    mkdir -p "${BINARIES_DIR}"

    # 整目录删除后重建:find -exec rm -rf 在大目录上会因边遍历边删触发 fts_read 竞态
    rm -rf "${DEST}"
    mkdir -p "${DEST}"
    cp -R "${BACKEND_DIR}/." "${DEST}/"
    chmod +x "${DEST}/potato-backend"
    chmod +x "${DEST}/potato"
    printf '%s\n' "${BACKEND_FINGERPRINT}" > "${PENDING_STAMP}"
    touch "${REBUILT_MARKER}"
    echo "Copied to: ${DEST}"
    echo ""
else
    echo "Reusing Tauri backend resource: ${DEST}"
    echo ""
fi

# Stage a standalone CPython (same X.Y/arch as this build's interpreter) so the
# frozen backend can install third-party plugin dependencies at runtime.
echo "== Staging bundled Python runtime =="
"$PYTHON_BIN" "${REPO_ROOT}/scripts/pack-tauri/stage_python_runtime.py" \
    --dest "${BINARIES_DIR}/python-runtime"
echo ""

echo "== Staging bundled Node runtime =="
"$PYTHON_BIN" "${REPO_ROOT}/scripts/pack-tauri/stage_node_runtime.py" \
    --dest "${BINARIES_DIR}/node-runtime"
echo ""

echo "========================================="
echo "PyInstaller Build Complete!"
echo "========================================="
echo "Output:"
echo "  Bundle: ${BACKEND_DIR}"
echo "  Tauri resource: ${DEST}"
echo ""
