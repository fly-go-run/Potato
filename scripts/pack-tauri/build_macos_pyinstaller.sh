#!/usr/bin/env bash
# Build Potato with Tauri for macOS (bundled CPython backend)
# Creates a self-contained desktop app with bundled Python backend
#
# Usage:
#   ./scripts/pack-tauri/build_macos_pyinstaller.sh
#
# Optional local-build controls:
#   POTATO_FORCE_NPM_CI=1         Reinstall frontend dependencies explicitly.
#   POTATO_STAGE_APP=1            Keep a copied app under dist/tauri-macos.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

VERSION=$(sed -n 's/^__version__[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' src/potato/__version__.py)

echo "========================================="
echo "Potato Tauri Build - macOS"
echo "========================================="
echo "Version: ${VERSION}"
echo ""

SIGN_MACOS_BUNDLE="${REPO_ROOT}/scripts/pack-tauri/sign_macos_bundle.sh"

CONSOLE_DIR="${REPO_ROOT}/console"
APP_DIR="${REPO_ROOT}/app"

ensure_npm_dependencies() {
    local package_dir="$1"
    local required_binary="$2"
    local hidden_lock="${package_dir}/node_modules/.package-lock.json"

    if [[ "${POTATO_FORCE_NPM_CI:-0}" == "1" ||
        ! -x "${package_dir}/node_modules/.bin/${required_binary}" ||
        ! -f "${hidden_lock}" ||
        "${package_dir}/package.json" -nt "${hidden_lock}" ||
        ( -f "${package_dir}/package-lock.json" &&
            "${package_dir}/package-lock.json" -nt "${hidden_lock}" ) ]]; then
        echo "Installing ${package_dir##*/} npm dependencies..."
        (cd "${package_dir}" && npm ci)
    else
        echo "Reusing ${package_dir##*/}/node_modules (lockfile unchanged)"
    fi
}

app_frontend_needs_build() {
    local output="${APP_DIR}/dist/index.html"

    [[ ! -f "${output}" ]] && return 0

    for input in \
        "${APP_DIR}/package.json" \
        "${APP_DIR}/package-lock.json" \
        "${APP_DIR}/vite.config.ts" \
        "${APP_DIR}/tsconfig.json" \
        "${APP_DIR}/tsconfig.app.json"; do
        if [[ -f "${input}" && "${input}" -nt "${output}" ]]; then
            return 0
        fi
    done

    find "${APP_DIR}/src" "${APP_DIR}/public" -type f -newer "${output}" \
        -print -quit 2>/dev/null | grep -q .
}

# Step 0: Prerequisites
echo "== Step 0: Checking Prerequisites =="
missing=()

if command -v npm &>/dev/null; then
    echo "  [OK] npm ($(npm --version))"
else
    echo "  [MISSING] npm"
    echo "    Install Node.js: https://nodejs.org/"
    missing+=("npm")
fi

if command -v rustc &>/dev/null; then
    echo "  [OK] rustc ($(rustc --version))"
else
    echo "  [MISSING] rustc (Rust)"
    echo "    Install: https://rustup.rs"
    missing+=("rustc")
fi

if command -v uv &>/dev/null; then
    echo "  [OK] uv ($(uv --version))"
else
    echo "  [MISSING] uv"
    echo "    Install: https://docs.astral.sh/uv/getting-started/installation/"
    missing+=("uv")
fi

if [ ${#missing[@]} -gt 0 ]; then
    echo ""
    echo "Missing prerequisites: ${missing[*]}"
    echo "Install the missing tools and re-run this script."
    exit 1
fi
echo ""

if [ ! -f "${SIGN_MACOS_BUNDLE}" ]; then
    echo "ERROR: macOS signing helper not found at ${SIGN_MACOS_BUNDLE}"
    exit 1
fi

if [ -z "${APPLE_SIGNING_IDENTITY:-}" ] && [ -z "${APPLE_CERTIFICATE:-}" ]; then
    # The Tauri app and bundled CPython are native Mach-O executables.
    # Keep their signature state consistent with ad-hoc signatures when no
    # Developer ID certificate is configured. This matches the legacy desktop
    # package behavior: signed enough for local loading, not notarized.
    export APPLE_SIGNING_IDENTITY="-"
    echo "Using ad-hoc macOS code signing"
fi
echo ""

# Step 1: Prepare frontend assets
echo "== Step 1: Preparing Frontend Assets =="
ensure_npm_dependencies "${APP_DIR}" "vite"
if app_frontend_needs_build; then
    echo "Building app frontend..."
    (cd "${APP_DIR}" && npm run build)
else
    echo "Reusing app/dist (frontend sources unchanged)"
fi

ensure_npm_dependencies "${CONSOLE_DIR}" "tauri"
ICON_SOURCE="${REPO_ROOT}/scripts/pack/assets/icon.svg"
ICON_OUTPUT="${CONSOLE_DIR}/src-tauri/icons/icon.png"
if [[ ! -f "${ICON_OUTPUT}" || "${ICON_SOURCE}" -nt "${ICON_OUTPUT}" ]]; then
    echo "Generating Tauri icons..."
    (cd "${CONSOLE_DIR}" && ./node_modules/.bin/tauri icon ../scripts/pack/assets/icon.svg)
else
    echo "Reusing generated Tauri icons (source unchanged)"
fi
echo "Syncing Tauri version..."
(cd "${CONSOLE_DIR}" && node ../scripts/pack-tauri/sync_tauri_version.mjs)
echo "Bundled Potato app will be built once by Tauri's beforeBuildCommand"
echo ""

# Step 2: Stage backend runtime (bundled CPython + Potato)
echo "== Step 2: Staging Backend Runtime =="
bash scripts/pack-tauri/build_pyinstaller.sh
echo "Backend runtime staged"
echo ""

# Step 3: Build Tauri app
echo "== Step 3: Building Tauri App =="
BUNDLE_DIR="${REPO_ROOT}/console/src-tauri/target/release/bundle"
rm -rf "${BUNDLE_DIR}/dmg" "${BUNDLE_DIR}/macos"
cd console
echo "Building for macOS..."
./node_modules/.bin/tauri build \
    --config src-tauri/tauri.version.conf.json \
    --bundles app
cd ..
echo "Tauri app built"
echo ""

# Keep the staged app name in sync with the desktop shell's public product name.
# Leaving the old Potato Desktop path here makes it very easy to launch a stale
# bundle (and, in particular, its old opaque menu-bar icon) after a rebuild.
APP_NAME="Potato"
APP_PATH="${BUNDLE_DIR}/macos/${APP_NAME}.app"
if [ ! -d "${APP_PATH}" ]; then
    echo "ERROR: No Tauri macOS app found at ${APP_PATH}"
    exit 1
fi

echo "== Step 3b: Signing Final macOS App =="
bash "${SIGN_MACOS_BUNDLE}" \
    "${APP_PATH}" \
    "${APPLE_SIGNING_IDENTITY}"
echo "Final macOS app signed and verified"
echo ""

# Step 4: Collect distribution artifacts
echo "== Step 4: Collecting Distribution Artifacts =="
DIST="${DIST:-dist}"
if [[ "${DIST}" = /* ]]; then
    DIST_ROOT="${DIST}"
else
    DIST_ROOT="${REPO_ROOT}/${DIST}"
fi
DIST_DIR="${DIST_ROOT}/tauri-macos"
ARCHIVE_SOURCE="${APP_PATH}"
if [[ "${POTATO_STAGE_APP:-0}" == "1" ]]; then
    rm -rf "${DIST_DIR}"
    mkdir -p "${DIST_DIR}"
    cp -R "${APP_PATH}" "${DIST_DIR}/"
    ARCHIVE_SOURCE="${DIST_DIR}/$(basename "${APP_PATH}")"
    echo ".app copied to ${ARCHIVE_SOURCE}"
else
    echo "Archiving Tauri app in place (set POTATO_STAGE_APP=1 to copy it)"
fi

# Match the legacy macOS package shape: one zip containing one .app bundle.

# Create ZIP archive
ZIP_NAME="${DIST_ROOT}/Potato-Tauri-${VERSION}-macOS.zip"
if [ -f "${ZIP_NAME}" ]; then
    rm -f "${ZIP_NAME}"
fi
if command -v ditto &>/dev/null; then
    ditto -c -k --sequesterRsrc --keepParent "${ARCHIVE_SOURCE}" "${ZIP_NAME}"
else
    cd "$(dirname "${ARCHIVE_SOURCE}")"
    zip -r "${ZIP_NAME}" "$(basename "${ARCHIVE_SOURCE}")"
    cd "${REPO_ROOT}"
fi

if [ -f "${ZIP_NAME}" ]; then
    SIZE=$(du -sh "${ZIP_NAME}" | cut -f1)
    echo "Created ${ZIP_NAME} (${SIZE})"
else
    echo "ERROR: Failed to create ZIP archive"
    exit 1
fi
echo ""

UPDATER_NAME="${DIST_ROOT}/Potato-Tauri-${VERSION}-macOS.app.tar.gz"
case "$(uname -m)" in
    arm64 | aarch64) UPDATER_TARGET="darwin-aarch64" ;;
    *) UPDATER_TARGET="darwin-x86_64" ;;
esac
# 更新包(*.app.tar.gz)只在配置了 Tauri 更新签名密钥的构建里产出;
# 本地无密钥构建没有该产物,跳过清单生成(不影响 .app 与 zip)。
if compgen -G "${BUNDLE_DIR}/macos/"*.app.tar.gz > /dev/null; then
    python3 "${REPO_ROOT}/scripts/pack-tauri/generate_update_manifest.py" stage \
        --bundle-dir "${BUNDLE_DIR}/macos" \
        --pattern '*.app.tar.gz' \
        --target "${UPDATER_TARGET}" \
        --output "${UPDATER_NAME}" \
        --pubkey-config "${REPO_ROOT}/console/src-tauri/tauri.version.conf.json"
else
    UPDATER_NAME="(skipped: no updater artifact; set TAURI_SIGNING_PRIVATE_KEY to produce one)"
    echo "Updater artifact not found; skipping update manifest staging"
fi

echo ""
echo "========================================="
echo "Build Complete!"
echo "========================================="
echo "App:          ${APP_PATH}"
if [[ "${POTATO_STAGE_APP:-0}" == "1" ]]; then
    echo "Staged app:   ${ARCHIVE_SOURCE}"
else
    echo "Staged app:   (disabled; set POTATO_STAGE_APP=1 to create one)"
fi
echo "Archive:      ${ZIP_NAME}"
echo "Updater:      ${UPDATER_NAME}"
echo ""
echo "Test: open \"${ARCHIVE_SOURCE}\""
echo ""
