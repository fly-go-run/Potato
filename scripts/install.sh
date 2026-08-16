#!/usr/bin/env bash
# Potato Installer
# Usage: curl -fsSL <url>/install.sh | bash
#    or: bash install.sh [--version X.Y.Z] [--from-source]
#
# Installs Potato into ~/.potato with a uv-managed Python environment.
# Users do NOT need Python pre-installed — uv handles everything.
set -euo pipefail

# ── Colors ────────────────────────────────────────────────────────────────────
if [ -t 1 ]; then
    BOLD="\033[1m"
    GREEN="\033[0;32m"
    YELLOW="\033[0;33m"
    RED="\033[0;31m"
    RESET="\033[0m"
else
    BOLD="" GREEN="" YELLOW="" RED="" RESET=""
fi

info()  { printf "${GREEN}[potato]${RESET} %s\n" "$*"; }
warn()  { printf "${YELLOW}[potato]${RESET} %s\n" "$*"; }
error() { printf "${RED}[potato]${RESET} %s\n" "$*" >&2; }
die()   { error "$@"; exit 1; }

# ── Defaults ──────────────────────────────────────────────────────────────────
# New installs use ~/.potato. Upgrades reuse an existing QwenPaw installation
# directory so its environment and user data are not silently orphaned.
if [ -n "${POTATO_HOME:-}" ]; then
    :
elif [ -n "${QWENPAW_HOME:-}" ]; then
    POTATO_HOME="$QWENPAW_HOME"
elif [ -d "$HOME/.potato" ]; then
    POTATO_HOME="$HOME/.potato"
elif [ -d "$HOME/.qwenpaw" ]; then
    POTATO_HOME="$HOME/.qwenpaw"
else
    POTATO_HOME="$HOME/.potato"
fi
POTATO_VENV="$POTATO_HOME/venv"
POTATO_BIN="$POTATO_HOME/bin"
PYTHON_VERSION="3.12"
POTATO_REPO="https://github.com/fly-go-run/Potato.git"

# New: Intelligent selection of PyPI source (automatically using Alibaba Cloud mirror for domestic users, and official source for overseas users)
choose_pypi_mirror() {
    # Test the connectivity of the official PyPI source (timeout 3 seconds, no output)
    if curl -s --connect-timeout 3 https://pypi.org/simple/ > /dev/null 2>&1; then
        echo "https://pypi.org/simple/"
        info "Using official PyPI source (network is good)" >&2
    else
        echo "https://mirrors.aliyun.com/pypi/simple/"
        info "Using Aliyun PyPI mirror (official source is unreachable)" >&2
    fi
}
PYPI_MIRROR=$(choose_pypi_mirror)

# New: Automatically clear old virtual environments and skip interactive prompts
export UV_VENV_CLEAR=1




VERSION=""
FROM_SOURCE=false
SOURCE_DIR=""
EXTRAS=""
PRERELEASE=false

# ── Parse args ────────────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --version)
            VERSION="$2"; shift 2 ;;
        --from-source)
            FROM_SOURCE=true
            # Accept optional path argument (next arg that doesn't start with --)
            if [[ $# -ge 2 && "$2" != --* ]]; then
                SOURCE_DIR="$(cd "$2" && pwd)" || die "Directory not found: $2"
                shift
            fi
            shift ;;
        --extras)
            EXTRAS="$2"; shift 2 ;;
        --prerelease)
            PRERELEASE=true; shift ;;
        -h|--help)
            cat <<EOF
Potato Installer

Usage: bash install.sh [OPTIONS]

Options:
  --version <VER>       Install a specific version (e.g. 0.0.2)
  --from-source [DIR]   Install from source. If DIR is given, use that local
                        directory; otherwise clone from GitHub.
  --extras <EXTRAS>     Comma-separated optional extras to install
                        (e.g. dev, whisper)
  --prerelease          Install the latest PyPI release, including pre-releases
  -h, --help            Show this help

Environment:
  POTATO_HOME        Installation directory (default: ~/.potato)
EOF
            exit 0 ;;
        *)
            die "Unknown option: $1 (try --help)" ;;
    esac
done

# ── OS check ──────────────────────────────────────────────────────────────────
OS="$(uname -s)"
case "$OS" in
    Linux|Darwin) ;;
    *) die "Unsupported OS: $OS. This installer supports Linux and macOS only." ;;
esac

printf "${GREEN}[potato]${RESET} Installing Potato into ${BOLD}%s${RESET}\n" "$POTATO_HOME"

# ── Step 1: Ensure uv is available ───────────────────────────────────────────
ensure_uv() {
    if command -v uv &>/dev/null; then
        info "uv found: $(command -v uv)"
        return
    fi

    # Check common install locations not yet on PATH
    for candidate in "$HOME/.local/bin/uv" "$HOME/.cargo/bin/uv"; do
        if [ -x "$candidate" ]; then
            export PATH="$(dirname "$candidate"):$PATH"
            info "uv found: $candidate"
            return
        fi
    done

    info "Installing uv..."
    curl -LsSf https://astral.sh/uv/install.sh | sh

    # Source the env file uv's installer creates, or add common paths
    if [ -f "$HOME/.local/bin/env" ]; then
        # shellcheck disable=SC1091
        . "$HOME/.local/bin/env"
    fi
    export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

    command -v uv &>/dev/null || die "Failed to install uv. Please install it manually: https://docs.astral.sh/uv/"
    info "uv installed successfully"
}

ensure_uv

# ── Step 2: Create / update virtual environment ──────────────────────────────
if [ -d "$POTATO_VENV" ]; then
    info "Existing environment found, upgrading..."
else
    info "Creating Python $PYTHON_VERSION environment..."
fi

uv venv "$POTATO_VENV" --python "$PYTHON_VERSION" --quiet

# Verify the venv was created
[ -x "$POTATO_VENV/bin/python" ] || die "Failed to create virtual environment"
info "Python environment ready ($("$POTATO_VENV/bin/python" --version))"

# ── Step 3: Install Potato ────────────────────────────────────────────────────
# Build extras suffix: "" or "[dev,whisper]"
EXTRAS_SUFFIX=""
if [ -n "$EXTRAS" ]; then
    EXTRAS_SUFFIX="[$EXTRAS]"
fi

## Ensure the default web app is in the package-data directory for source
## installs. Sets _WEB_ASSETS_COPIED=1 so local source trees are cleaned up.
_WEB_ASSETS_COPIED=0
_WEB_UI_AVAILABLE=0
prepare_web_app() {
    local repo_dir="$1"
    local app_src="$repo_dir/app/dist"
    # potato/console is the historical package-data location for /console.
    local web_dest="$repo_dir/src/potato/console"

    # Copy a current app build if available (e.g. a developer already built it).
    if [ -d "$app_src" ] && [ -f "$app_src/index.html" ]; then
        info "Copying default web app assets..."
        rm -rf "$web_dest"/*
        mkdir -p "$web_dest"
        cp -R "$app_src/"* "$web_dest/"
        _WEB_ASSETS_COPIED=1
        _WEB_UI_AVAILABLE=1
        return
    fi

    # Try to build if npm is available
    if [ ! -f "$repo_dir/app/package.json" ]; then
        warn "Web app source not found — the web UI won't be available."
        return
    fi

    if ! command -v npm &>/dev/null; then
        warn "npm not found — skipping web app build."
        warn "Install Node.js from https://nodejs.org/ then re-run this installer,"
        warn "or run 'cd app && npm ci && npm run build' manually."
        return
    fi

    info "Building default web app (npm ci && npm run build)..."
    (cd "$repo_dir/app" && npm ci && npm run build)
    if [ -f "$app_src/index.html" ]; then
        rm -rf "$web_dest"/*
        mkdir -p "$web_dest"
        cp -R "$app_src/"* "$web_dest/"
        _WEB_ASSETS_COPIED=1
        _WEB_UI_AVAILABLE=1
        info "Default web app built successfully"
        return
    fi

    warn "Web app build completed but index.html not found — the web UI won't be available."
}

## Remove web assets copied into the source tree for an installation.
cleanup_web_app() {
    local repo_dir="$1"
    if [ "$_WEB_ASSETS_COPIED" = 1 ]; then
        rm -rf "$repo_dir/src/potato/console/"*
    fi
}

## Ensure docs are available in src/potato/docs/ for source installs.
_DOCS_COPIED=0
prepare_docs() {
    local repo_dir="$1"
    local docs_src="$repo_dir/website/public/docs"
    local docs_dest="$repo_dir/src/potato/docs"

    if [ -d "$docs_dest" ] && ls "$docs_dest"/*.md >/dev/null 2>&1; then
        return
    fi

    if [ -d "$docs_src" ] && ls "$docs_src"/*.md >/dev/null 2>&1; then
        mkdir -p "$docs_dest"
        cp "$docs_src/"*.md "$docs_dest/"
        _DOCS_COPIED=1
    fi
}

cleanup_docs() {
    local repo_dir="$1"
    if [ "$_DOCS_COPIED" = 1 ]; then
        rm -rf "$repo_dir/src/potato/docs"
    fi
}

if [ "$FROM_SOURCE" = true ]; then
    if [ -n "$SOURCE_DIR" ]; then
        info "Installing Potato from local source: $SOURCE_DIR"
        prepare_web_app "$SOURCE_DIR"
        prepare_docs "$SOURCE_DIR"
        info "Installing package from source..."
        uv pip install "${SOURCE_DIR}${EXTRAS_SUFFIX}" --python "$POTATO_VENV/bin/python" --index-url "$PYPI_MIRROR"
        cleanup_web_app "$SOURCE_DIR"
        cleanup_docs "$SOURCE_DIR"
    else
        info "Installing Potato from source (GitHub)..."
        CLONE_DIR="$(mktemp -d)"
        trap 'rm -rf "$CLONE_DIR"' EXIT
        git clone --depth 1 "$POTATO_REPO" "$CLONE_DIR"
        prepare_web_app "$CLONE_DIR"
        prepare_docs "$CLONE_DIR"
        info "Installing package from source..."
        uv pip install "${CLONE_DIR}${EXTRAS_SUFFIX}" --python "$POTATO_VENV/bin/python" --index-url "$PYPI_MIRROR"
        # CLONE_DIR is cleaned up by trap; no need for cleanup_web_app/cleanup_docs
    fi
else
    PACKAGE="qwenpaw"
    if [ -n "$VERSION" ]; then
        PACKAGE="qwenpaw==$VERSION"
    fi

    PRERELEASE_ARGS=()
    if [ "$PRERELEASE" = true ]; then
        PRERELEASE_ARGS=(--prerelease=allow)
    fi

    info "Installing ${PACKAGE}${EXTRAS_SUFFIX} from PyPI..."
    uv pip install "${PACKAGE}${EXTRAS_SUFFIX}" --python "$POTATO_VENV/bin/python" --quiet --index-url "$PYPI_MIRROR" --refresh-package qwenpaw ${PRERELEASE_ARGS[@]+"${PRERELEASE_ARGS[@]}"}
fi

# Verify the CLI entry point exists
[ -x "$POTATO_VENV/bin/potato" ] || die "Installation failed: potato CLI not found in venv"
info "Potato installed successfully"

# Check web UI availability (for PyPI installs, check the installed package).
if [ "$_WEB_UI_AVAILABLE" = 0 ]; then
    WEB_UI_CHECK="$("$POTATO_VENV/bin/python" -c "import importlib.resources, potato; p=importlib.resources.files('potato')/'console'/'index.html'; print('yes' if p.is_file() else 'no')" 2>/dev/null || echo 'no')"
    if [ "$WEB_UI_CHECK" = "yes" ]; then
        _WEB_UI_AVAILABLE=1
    fi
fi

# ── Step 4: Create wrapper script ────────────────────────────────────────────
mkdir -p "$POTATO_BIN"

cat > "$POTATO_BIN/potato" << 'WRAPPER'
#!/usr/bin/env bash
# Potato CLI wrapper — delegates to the uv-managed environment.
set -euo pipefail

if [ -z "${POTATO_HOME:-}" ]; then
    if [ -n "${QWENPAW_HOME:-}" ]; then
        POTATO_HOME="$QWENPAW_HOME"
    elif [ -d "$HOME/.qwenpaw" ] && [ ! -d "$HOME/.potato" ]; then
        POTATO_HOME="$HOME/.qwenpaw"
    else
        POTATO_HOME="$HOME/.potato"
    fi
fi
REAL_BIN="$POTATO_HOME/venv/bin/potato"

if [ ! -x "$REAL_BIN" ]; then
    echo "Error: Potato environment not found at $POTATO_HOME/venv" >&2
    echo "Please reinstall: curl -fsSL <install-url> | bash" >&2
    exit 1
fi

exec "$REAL_BIN" "$@"
WRAPPER

chmod +x "$POTATO_BIN/potato"
info "Wrapper created at $POTATO_BIN/potato"

# ── Step 5: Update PATH in shell profile ─────────────────────────────────────
PATH_ENTRY="export PATH=\"$POTATO_BIN:\$PATH\""

add_to_profile() {
    local profile="$1"
    if [ -f "$profile" ] && grep -qF "$POTATO_BIN" "$profile"; then
        return 0  # already present
    fi
    if [ -f "$profile" ] || [ "$2" = "create" ]; then
        printf '\n# Potato\n%s\n' "$PATH_ENTRY" >> "$profile"
        info "Updated $profile"
        return 0
    fi
    return 1
}

UPDATED_PROFILE=false

case "$OS" in
    Darwin)
        add_to_profile "$HOME/.zshrc" "create" && UPDATED_PROFILE=true
        # Also update bash profile if it exists
        add_to_profile "$HOME/.bash_profile" "no-create" || true
        ;;
    Linux)
        add_to_profile "$HOME/.bashrc" "create" && UPDATED_PROFILE=true
        # Also update zshrc if it exists
        add_to_profile "$HOME/.zshrc" "no-create" || true
        ;;
esac

# ── Done ──────────────────────────────────────────────────────────────────────
echo ""
printf "${GREEN}${BOLD}Potato installed successfully!${RESET}\n"
echo ""

# Install summary
printf "  Install location:  ${BOLD}%s${RESET}\n" "$POTATO_HOME"
printf "  Python:            ${BOLD}%s${RESET}\n" "$("$POTATO_VENV/bin/python" --version 2>&1)"
if [ "$_WEB_UI_AVAILABLE" = 1 ]; then
    printf "  Web UI:             ${GREEN}available${RESET}\n"
else
    printf "  Web UI:             ${YELLOW}not available${RESET}\n"
    echo "                     Install Node.js and re-run to enable the web UI."
fi
echo ""

if [ "$UPDATED_PROFILE" = true ]; then
    echo "To get started, open a new terminal or run:"
    echo ""
    printf "  ${BOLD}source ~/.zshrc${RESET}  # or ~/.bashrc\n"
    echo ""
fi

echo "Then run:"
echo ""
printf "  ${BOLD}potato init${RESET}       # first-time setup\n"
printf "  ${BOLD}potato app${RESET}        # start Potato\n"
echo ""
printf "To upgrade later, re-run this installer.\n"
printf "To uninstall, run: ${BOLD}potato uninstall${RESET}\n"
