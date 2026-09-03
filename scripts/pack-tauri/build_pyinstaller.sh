#!/usr/bin/env bash
# Stage the Potato desktop backend runtime for Tauri
# Bundled standalone CPython + Potato site install + Node + cua-driver
#
# Usage:
#   ./scripts/pack-tauri/build_pyinstaller.sh
#
# Prerequisites:
#   - Python 3.10+ with virtual environment

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

DIST="${DIST:-dist}"
VERSION=$(sed -n 's/^__version__[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' src/potato/__version__.py)

BINARIES_DIR="${REPO_ROOT}/console/src-tauri/binaries"

echo "========================================="
echo "Potato Backend Runtime Staging"
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

# The desktop backend runs from the bundled standalone CPython
# (binaries/python-runtime). The PyInstaller-frozen sidecar was dropped in
# 2.0.7: it doubled the bundle size and is no longer a fallback.
mkdir -p "${BINARIES_DIR}"
if [ -d "${BINARIES_DIR}/potato-backend" ]; then
    echo "Removing stale frozen backend: ${BINARIES_DIR}/potato-backend"
    rm -rf "${BINARIES_DIR}/potato-backend"
fi
rm -f "${BINARIES_DIR}"/.potato-backend-*

install_python_packages() {
    if command -v uv &>/dev/null; then
        uv pip install --python "$PYTHON_BIN" "$@"
    else
        "$PYTHON_BIN" -m pip install "$@"
    fi
}

# stage_*_runtime.py need `packaging` for release lookups.
if ! "$PYTHON_BIN" -c "import packaging" 2> /dev/null; then
    install_python_packages packaging
fi

# Stage a standalone CPython (same X.Y/arch as this build's interpreter):
# it runs the backend and installs third-party plugin dependencies at runtime.
echo "== Staging bundled Python runtime =="
"$PYTHON_BIN" "${REPO_ROOT}/scripts/pack-tauri/stage_python_runtime.py" \
    --dest "${BINARIES_DIR}/python-runtime"
echo ""

# Install Potato into that CPython so the desktop shell can skip PyInstaller
# on the hot path. Failure is fatal: a silent fallback to the frozen sidecar
# would hide the 18s cold start and native-extension breakage.
BUNDLED_PY="${BINARIES_DIR}/python-runtime/python/bin/python3"
if [ ! -x "${BUNDLED_PY}" ]; then
    BUNDLED_PY="${BINARIES_DIR}/python-runtime/python/bin/python"
fi
if [ ! -x "${BUNDLED_PY}" ]; then
    echo "ERROR: bundled python not found under ${BINARIES_DIR}/python-runtime"
    exit 1
fi
echo "== Installing Potato into bundled CPython =="
"$PYTHON_BIN" "${REPO_ROOT}/scripts/pack-tauri/stage_potato_runtime.py" \
    --python "${BUNDLED_PY}" \
    --repo "${REPO_ROOT}"
echo ""

echo "== Staging bundled Node runtime =="
"$PYTHON_BIN" "${REPO_ROOT}/scripts/pack-tauri/stage_node_runtime.py" \
    --dest "${BINARIES_DIR}/node-runtime"
echo ""

echo "== Staging bundled cua-driver =="
"$PYTHON_BIN" "${REPO_ROOT}/scripts/pack-tauri/stage_cua_driver.py" \
    --dest "${BINARIES_DIR}/cua-driver"
echo ""

echo "========================================="
echo "Backend Runtime Staging Complete!"
echo "========================================="
echo "Output: ${BINARIES_DIR}"
echo ""
