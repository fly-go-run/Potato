# Stage the Potato desktop backend runtime for Tauri (Windows)
# Bundled standalone CPython + Potato site install + Node + cua-driver
#
# Usage:
#   powershell ./scripts/pack-tauri/build_pyinstaller.ps1
#
# Prerequisites:
#   - Python 3.10+ with virtual environment

param()

$ErrorActionPreference = "Stop"
$REPO_ROOT = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $REPO_ROOT

$DIST = if ($env:DIST) { $env:DIST } else { "dist" }
if (-not [System.IO.Path]::IsPathRooted($DIST)) {
    $DIST = Join-Path $REPO_ROOT $DIST
}
$VERSION_FILE = "src\potato\__version__.py"

# Extract version
if (Test-Path $VERSION_FILE) {
    $content = Get-Content $VERSION_FILE -Raw
    if ($content -match '__version__\s*=\s*"([^"]+)"') {
        $VERSION = $Matches[1]
    } else {
        throw "Failed to extract version from $VERSION_FILE"
    }
} else {
    throw "Version file not found: $VERSION_FILE"
}

Write-Host "=========================================" -ForegroundColor Cyan
Write-Host "Potato Backend Runtime Staging - Windows" -ForegroundColor Cyan
Write-Host "=========================================" -ForegroundColor Cyan
Write-Host "Version: $VERSION"
Write-Host "Repository: $REPO_ROOT"
Write-Host ""

# Check prerequisites
Write-Host "== Checking prerequisites ==" -ForegroundColor Yellow

$UV_BIN = (Get-Command uv -ErrorAction SilentlyContinue).Source
$PYTHON_BIN = Join-Path $REPO_ROOT ".venv\Scripts\python.exe"
if (-not (Test-Path $PYTHON_BIN)) {
    if ($UV_BIN) {
        Write-Host ".venv not found, creating virtual environment with uv" -ForegroundColor Yellow
        & $UV_BIN venv "$REPO_ROOT\.venv"
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to create virtual environment with uv"
        }
    } else {
        Write-Host ".venv not found, using system Python" -ForegroundColor Yellow
        $PYTHON_BIN = (Get-Command python -ErrorAction SilentlyContinue).Source
    }
    if (-not $PYTHON_BIN -or -not (Test-Path $PYTHON_BIN)) {
        Write-Host "ERROR: Python not found in .venv or PATH" -ForegroundColor Red
        Write-Host "Please create virtual environment first: python -m venv .venv"
        exit 1
    }
}

$pythonVersion = & $PYTHON_BIN --version
Write-Host "Python: $pythonVersion" -ForegroundColor Green

function Test-PythonImport {
    param([string]$Statement)
    $previousErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        & $PYTHON_BIN -c $Statement *> $null
        return $LASTEXITCODE -eq 0
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
}

function Assert-LastExit {
    param([string]$Message)
    if ($LASTEXITCODE -ne 0) { throw $Message }
}

function Install-PythonPackages {
    param([string[]]$Packages)
    if ($UV_BIN) {
        & $UV_BIN pip install --python $PYTHON_BIN @Packages
    } else {
        & $PYTHON_BIN -m pip install @Packages
    }
    Assert-LastExit "Failed to install Python packages: $($Packages -join ', ')"
}

function Uninstall-PythonPackage {
    param([string]$Package)
    $previousErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        if ($UV_BIN) {
            & $UV_BIN pip uninstall --python $PYTHON_BIN -y $Package *> $null
        } else {
            & $PYTHON_BIN -m pip uninstall -y $Package *> $null
        }
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
}

$BINARIES_DIR = Join-Path $REPO_ROOT "console\src-tauri\binaries"
New-Item -ItemType Directory -Force -Path $BINARIES_DIR | Out-Null

# The desktop backend runs from the bundled standalone CPython
# (binaries\python-runtime). The PyInstaller-frozen sidecar was dropped in
# 2.0.7: it doubled the installer size and is no longer a fallback. Remove a
# stale copy so it cannot leak into the bundle.
$STALE_FROZEN = Join-Path $BINARIES_DIR "potato-backend"
if (Test-Path $STALE_FROZEN) {
    Write-Host "Removing stale frozen backend: $STALE_FROZEN" -ForegroundColor Yellow
    Remove-Item -Recurse -Force $STALE_FROZEN
}

# stage_*_runtime.py need `packaging` for release lookups.
if (-not (Test-PythonImport "import packaging")) {
    Install-PythonPackages -Packages @("packaging")
}
Write-Host ""

# Stage a standalone CPython (same X.Y/arch as this build's interpreter):
# it runs the backend and installs third-party plugin dependencies at runtime.
Write-Host "== Staging bundled Python runtime ==" -ForegroundColor Yellow
& $PYTHON_BIN (Join-Path $REPO_ROOT "scripts\pack-tauri\stage_python_runtime.py") `
    --dest (Join-Path $BINARIES_DIR "python-runtime")
Assert-LastExit "Failed to stage bundled Python runtime"
Write-Host ""

$bundledPy = Join-Path $BINARIES_DIR "python-runtime\python\python.exe"
if (Test-Path $bundledPy) {
    Write-Host "== Installing Potato into bundled CPython ==" -ForegroundColor Yellow
    & $PYTHON_BIN (Join-Path $REPO_ROOT "scripts\pack-tauri\stage_potato_runtime.py") `
        --python $bundledPy `
        --repo $REPO_ROOT
    # Fatal: a silent fallback to the frozen sidecar hides both the 18s
    # cold start and missing VC++ runtime DLLs (2.0.6 shipped without
    # msvcp140.dll and crashed on clean Windows machines).
    Assert-LastExit "Bundled CPython Potato install or native import check failed"
    Write-Host ""
} else {
    throw "bundled python.exe not found at $bundledPy"
}

Write-Host "== Staging bundled Node runtime ==" -ForegroundColor Yellow
& $PYTHON_BIN (Join-Path $REPO_ROOT "scripts\pack-tauri\stage_node_runtime.py") `
    --dest (Join-Path $BINARIES_DIR "node-runtime")
Assert-LastExit "Failed to stage bundled Node runtime"
Write-Host ""

Write-Host "== Staging bundled cua-driver ==" -ForegroundColor Yellow
& $PYTHON_BIN (Join-Path $REPO_ROOT "scripts\pack-tauri\stage_cua_driver.py") `
    --dest (Join-Path $BINARIES_DIR "cua-driver")
Assert-LastExit "Failed to stage bundled cua-driver"
Write-Host ""

Write-Host "=========================================" -ForegroundColor Cyan
Write-Host "Backend Runtime Staging Complete!" -ForegroundColor Green
Write-Host "=========================================" -ForegroundColor Cyan
Write-Host "Output: $BINARIES_DIR"
Write-Host ""
