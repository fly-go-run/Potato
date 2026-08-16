@echo off
setlocal EnableDelayedExpansion

REM Potato Installer for Windows (cmd.exe / batch)
REM Usage: install.bat [-Version X.Y.Z] [-FromSource] [-SourceDir DIR]
REM                         [-Extras "dev,whisper"] [-UvPath PATH] [-Help]
REM
REM Installs Potato into %USERPROFILE%\.potato with a uv-managed Python environment.
REM Users do NOT need Python pre-installed -- uv handles everything.
REM
REM uv is obtained automatically (no action required from the user):
REM   1. Found on PATH or in common locations
REM   2. Downloaded via https://astral.sh/uv/install.ps1
REM   3. Downloaded via GitHub Releases if astral.sh is unreachable (e.g. in China)

REM ── Defaults ──────────────────────────────────────────────────────────────────
if defined POTATO_HOME (
    set "POTATO_HOME=%POTATO_HOME%"
) else if defined QWENPAW_HOME (
    set "POTATO_HOME=%QWENPAW_HOME%"
) else if exist "%USERPROFILE%\.potato" (
    set "POTATO_HOME=%USERPROFILE%\.potato"
) else if exist "%USERPROFILE%\.qwenpaw" (
    set "POTATO_HOME=%USERPROFILE%\.qwenpaw"
) else (
    set "POTATO_HOME=%USERPROFILE%\.potato"
)
set "POTATO_VENV=%POTATO_HOME%\venv"
set "POTATO_BIN=%POTATO_HOME%\bin"
set "PYTHON_VERSION=3.12"
set "POTATO_REPO=https://github.com/fly-go-run/Potato.git"

REM ──── Argument defaults ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────
set "ARG_VERSION="
set "ARG_FROM_SOURCE=0"
set "ARG_SOURCE_DIR="
set "ARG_EXTRAS="
set "ARG_UV_PATH="
set "ARG_PRERELEASE=0"
set "WEB_ASSETS_COPIED=0"
set "WEB_UI_AVAILABLE=0"

REM ──── Parse arguments ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
:parse_args
if "%~1"=="" goto :done_args
if /i "%~1"=="-Version"    goto :arg_version
if /i "%~1"=="-FromSource" goto :arg_fromsource
if /i "%~1"=="-SourceDir"  goto :arg_sourcedir
if /i "%~1"=="-Extras"     goto :arg_extras
if /i "%~1"=="-Prerelease" goto :arg_prerelease
if /i "%~1"=="-UvPath"     goto :arg_uvpath
if /i "%~1"=="-Help"       goto :show_help
shift
goto :parse_args

:arg_version
set "ARG_VERSION=%~2"
shift & shift
goto :parse_args

:arg_fromsource
set "ARG_FROM_SOURCE=1"
shift
goto :parse_args

:arg_sourcedir
set "ARG_SOURCE_DIR=%~2"
shift & shift
goto :parse_args

:arg_extras
set "ARG_EXTRAS=%~2"
shift & shift
goto :parse_args

:arg_prerelease
set "ARG_PRERELEASE=1"
shift
goto :parse_args

:arg_uvpath
set "ARG_UV_PATH=%~2"
shift & shift
goto :parse_args

:done_args
goto :main

REM ──── Help ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
:show_help
echo Potato Installer for Windows
echo.
echo Usage: install.bat [OPTIONS]
echo.
echo Options:
echo   -Version ^<VER^>        Install a specific version (e.g. 0.0.2)
echo   -FromSource           Install from source (requires git, or use -SourceDir)
echo   -SourceDir ^<DIR^>      Local source directory (used with -FromSource)
echo   -Extras ^<EXTRAS^>      Comma-separated optional extras to install
echo                          (e.g. dev, whisper)
echo   -Prerelease           Install the latest PyPI release, including pre-releases
echo   -UvPath ^<PATH^>        Path to a pre-installed uv.exe (skips all auto-install)
echo   -Help                 Show this help
echo.
echo Environment:
echo   POTATO_HOME            Installation directory (default: %%USERPROFILE%%\.potato)
exit /b 0

REM ──── Helper functions ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
:write_info
echo [potato] %~1
exit /b 0

:write_warn
echo [potato] WARNING: %~1
exit /b 0

:write_err
echo [potato] ERROR: %~1
exit /b 0

:stop_with_error
echo [potato] ERROR: %~1
exit /b 1

REM ──── Download uv from GitHub Releases ────────────────────────────────────────────────────────────────────────────────────
REM Subroutine: called when astral.sh is unreachable (e.g. in China).
REM On success: uv.exe is in %LOCALAPPDATA%\uv and that dir is prepended to PATH.
:download_uv_github
if /i "%PROCESSOR_ARCHITECTURE%"=="ARM64" (
    set "_DL_ARCH=aarch64"
) else (
    set "_DL_ARCH=x86_64"
)
set "_DL_URL=https://github.com/astral-sh/uv/releases/latest/download/uv-!_DL_ARCH!-pc-windows-msvc.zip"
set "_DL_DEST=%LOCALAPPDATA%\uv"
set "_DL_ZIP=%TEMP%\uv-gh-%RANDOM%.zip"

echo [potato] Downloading uv ^(!_DL_ARCH!^) from GitHub Releases...

REM Try curl.exe (built into Windows 10+), then fall back to PowerShell
where curl >nul 2>&1
if not errorlevel 1 (
    curl -L --progress-bar -o "!_DL_ZIP!" "!_DL_URL!"
    if not errorlevel 1 goto :download_uv_extract
    echo [potato] curl failed, retrying with PowerShell...
    del "!_DL_ZIP!" >nul 2>&1
)

powershell -NoProfile -Command "$ProgressPreference='SilentlyContinue'; Invoke-WebRequest -Uri '!_DL_URL!' -OutFile '!_DL_ZIP!' -UseBasicParsing"
if errorlevel 1 (
    echo [potato] ERROR: GitHub download also failed.
    echo [potato] Download uv manually from: https://github.com/astral-sh/uv/releases/latest
    del "!_DL_ZIP!" >nul 2>&1
    exit /b 1
)

:download_uv_extract
if not exist "!_DL_DEST!" mkdir "!_DL_DEST!"
echo [potato] Extracting uv...
powershell -NoProfile -Command "Expand-Archive -Force -Path '!_DL_ZIP!' -DestinationPath '!_DL_DEST!'"
set "_DL_ERR=%errorlevel%"
del "!_DL_ZIP!" >nul 2>&1
if %_DL_ERR% neq 0 (
    echo [potato] ERROR: Extraction failed.
    exit /b 1
)
if not exist "!_DL_DEST!\uv.exe" (
    echo [potato] ERROR: uv.exe not found after extraction.
    exit /b 1
)
set "PATH=!_DL_DEST!;!PATH!"
echo [potato] uv installed: !_DL_DEST!\uv.exe
exit /b 0

REM ──── Ensure uv ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
:ensure_uv
REM 0. User-supplied path (-UvPath)
if defined ARG_UV_PATH (
    if not exist "%ARG_UV_PATH%" (
        echo [potato] ERROR: Specified uv not found: %ARG_UV_PATH%
        exit /b 1
    )
    for %%I in ("%ARG_UV_PATH%") do set "PATH=%%~dpI;!PATH!"
    echo [potato] uv found: %ARG_UV_PATH%
    goto :ensure_uv_done
)

REM 1. Already on PATH
where uv >nul 2>&1
if %errorlevel%==0 (
    for /f "delims=" %%p in ('where uv 2^>nul') do (
        echo [potato] uv found: %%p
        goto :ensure_uv_done
    )
)

REM 2. Common install locations not yet on PATH
for %%c in ("%USERPROFILE%\.local\bin\uv.exe" "%USERPROFILE%\.cargo\bin\uv.exe" "%LOCALAPPDATA%\uv\uv.exe") do (
    if exist %%c (
        set "_UV_DIR=%%~dpc"
        set "PATH=!_UV_DIR!;!PATH!"
        echo [potato] uv found: %%~c
        goto :ensure_uv_done
    )
)

REM 3. Try astral.sh (standard installer, fast outside China)
echo [potato] Installing uv via astral.sh...
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://astral.sh/uv/install.ps1 -TimeoutSec 15 | iex"
if not errorlevel 1 goto :ensure_uv_refresh

REM 4. astral.sh failed -- fall back to GitHub Releases (works in China)
echo [potato] astral.sh unreachable, falling back to GitHub Releases...
call :download_uv_github
if errorlevel 1 (
    echo [potato] ERROR: Failed to install uv automatically.
    echo [potato] Please install uv manually: https://docs.astral.sh/uv/
    exit /b 1
)
goto :ensure_uv_done

:ensure_uv_refresh
REM Refresh PATH after astral.sh install
for %%p in ("%USERPROFILE%\.local\bin" "%USERPROFILE%\.cargo\bin" "%LOCALAPPDATA%\uv") do (
    if exist %%p (
        echo "!PATH!" | findstr /i /c:"%%~p" >nul 2>&1
        if errorlevel 1 set "PATH=%%~p;!PATH!"
    )
)
where uv >nul 2>&1
if errorlevel 1 (
    echo [potato] ERROR: Failed to install uv. Please install it manually: https://docs.astral.sh/uv/
    exit /b 1
)
echo [potato] uv installed via astral.sh

:ensure_uv_done
exit /b 0

REM ──── Prepare the default web app ───────────────────────────────────────────────────────────────────────────────────────────────
:prepare_web_app
REM %~1 = RepoDir
set "_REPO_DIR=%~1"
set "_WEB_SRC=%_REPO_DIR%\app\dist"
set "_WEB_DEST=%_REPO_DIR%\src\potato\console"

REM Already populated
if exist "%_WEB_DEST%\index.html" (
    set "WEB_UI_AVAILABLE=1"
    exit /b 0
)

REM Copy pre-built assets if available
if exist "%_WEB_SRC%\index.html" (
    echo [potato] Copying default web app assets...
    if not exist "%_WEB_DEST%" mkdir "%_WEB_DEST%"
    xcopy /s /e /y /q "%_WEB_SRC%\*" "%_WEB_DEST%\" >nul
    set "WEB_ASSETS_COPIED=1"
    set "WEB_UI_AVAILABLE=1"
    exit /b 0
)

REM Try to build if npm is available
if not exist "%_REPO_DIR%\app\package.json" (
    echo [potato] WARNING: Web app source not found - the web UI won't be available.
    exit /b 0
)

where npm >nul 2>&1
if errorlevel 1 (
    echo [potato] WARNING: npm not found - skipping web app build.
    echo [potato] WARNING: Install Node.js from https://nodejs.org/ then re-run this installer,
    echo [potato] WARNING: or run 'cd app ^&^& npm ci ^&^& npm run build' manually.
    exit /b 0
)

echo [potato] Building default web app (npm ci ^&^& npm run build)...
pushd "%_REPO_DIR%\app"
npm ci
if errorlevel 1 (
    popd
    echo [potato] WARNING: npm ci failed - the web UI won't be available.
    exit /b 0
)
npm run build
if errorlevel 1 (
    popd
    echo [potato] WARNING: npm run build failed - the web UI won't be available.
    exit /b 0
)
popd

if exist "%_WEB_SRC%\index.html" (
    if not exist "%_WEB_DEST%" mkdir "%_WEB_DEST%"
    xcopy /s /e /y /q "%_WEB_SRC%\*" "%_WEB_DEST%\" >nul
    set "WEB_ASSETS_COPIED=1"
    set "WEB_UI_AVAILABLE=1"
    echo [potato] Default web app built successfully
    exit /b 0
)

echo [potato] WARNING: Web app build completed but index.html not found - the web UI won't be available.
exit /b 0

REM ──── Cleanup web app assets ───────────────────────────────────────────────────────────────────────────────────────────────────
:cleanup_web_app
REM %~1 = RepoDir
if "%WEB_ASSETS_COPIED%"=="1" (
    set "_CLEANUP_DEST=%~1\src\potato\console"
    if exist "!_CLEANUP_DEST!" rd /s /q "!_CLEANUP_DEST!" 2>nul
)
exit /b 0

REM ══════════════════════════════ MAIN ═════════════════════════════════════════
:main
echo [potato] Installing Potato into %POTATO_HOME%

REM ──── Step 1: Ensure uv ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────
call :ensure_uv
if errorlevel 1 exit /b 1

REM ──── Step 2: Create / update virtual environment ──────────────────────────────────────────────────────────────
if exist "%POTATO_VENV%" (
    echo [potato] Existing environment found, upgrading...
) else (
    echo [potato] Creating Python %PYTHON_VERSION% environment...
)

uv venv "%POTATO_VENV%" --python %PYTHON_VERSION% --quiet --clear
if errorlevel 1 (
    echo [potato] ERROR: Failed to create virtual environment
    exit /b 1
)

set "VENV_PYTHON=%POTATO_VENV%\Scripts\python.exe"
if not exist "%VENV_PYTHON%" (
    echo [potato] ERROR: Failed to create virtual environment
    exit /b 1
)

for /f "delims=" %%v in ('"%VENV_PYTHON%" --version 2^>^&1') do set "PY_VERSION=%%v"
echo [potato] Python environment ready (%PY_VERSION%)

REM ──── Step 3: Install Potato ──────────────────────────────────────────────────────────────────────────────────────────────────────────
set "EXTRAS_SUFFIX="
if defined ARG_EXTRAS set "EXTRAS_SUFFIX=[%ARG_EXTRAS%]"

set "VENV_POTATO=%POTATO_VENV%\Scripts\potato.exe"

REM Use goto-based branching to avoid nested parenthesized blocks,
REM which break when %vars% expand to values containing "(" or ")".
if "%ARG_FROM_SOURCE%"=="1" goto :install_from_source
goto :install_from_pypi

:install_from_source
powershell -NoProfile -Command "$e=$env:EXTRAS_SUFFIX; if($e -and $e -notmatch '^\[[A-Za-z0-9,_-]+\]$'){exit 1}"
if errorlevel 1 goto :invalid_extras_suffix
if defined ARG_SOURCE_DIR goto :install_from_local
goto :install_from_github_potato

:install_from_local
echo [potato] Installing package from source...

rem === Secure Input Validation (Prevents Argument Injection) ===
rem 1. Ensure non-empty
if not defined ARG_SOURCE_DIR set "ARG_SOURCE_DIR=."
if not defined EXTRAS_SUFFIX set "EXTRAS_SUFFIX="

rem 2. Validate source path without echoing untrusted text through cmd.exe.
rem PowerShell receives the value through the environment, so shell metacharacters
rem are inspected as data instead of being reparsed as a batch command.

powershell -NoProfile -Command "$p=$env:ARG_SOURCE_DIR; foreach($c in 33,34,37,38,40,41,60,62,124,94){if($p.IndexOf([char]$c) -ge 0){exit 1}}"
if errorlevel 1 goto :invalid_source_path

powershell -NoProfile -Command "$e=$env:EXTRAS_SUFFIX; if($e -and $e -notmatch '^\[[A-Za-z0-9,_-]+\]$'){exit 1}"
if errorlevel 1 goto :invalid_extras_suffix
rem === End Security Validation ===

for %%I in ("%ARG_SOURCE_DIR%") do set "ARG_SOURCE_DIR=%%~fI"
call :prepare_web_app "%ARG_SOURCE_DIR%"

rem The input has now been verified as safe and can proceed with installation.
uv pip install "%ARG_SOURCE_DIR%%EXTRAS_SUFFIX%" --python "%VENV_PYTHON%"
set "_INST_ERR=%errorlevel%"
call :cleanup_web_app "%ARG_SOURCE_DIR%"
if %_INST_ERR% neq 0 (
    echo [potato] ERROR: Installation from source failed
    exit /b 1
)
goto :install_verify

:invalid_source_path
echo [potato] ERROR: Source path contains unsafe shell characters.
echo [potato] Installation aborted before preparing frontend assets.
exit /b 1

:invalid_extras_suffix
echo [potato] ERROR: Extras must contain only letters, digits, commas, underscores, and hyphens inside brackets.
exit /b 1

:invalid_package_arguments
echo [potato] ERROR: Version or extras contains unsupported characters.
exit /b 1

:install_from_github_potato
where git >nul 2>&1
if errorlevel 1 (
    echo [potato] ERROR: git is required for -FromSource without a local directory.
    echo [potato]        Please install Git from https://git-scm.com/ or pass a local path:
    echo [potato]        install-w-uv.bat -FromSource -SourceDir C:\path\to\Potato
    exit /b 1
)
echo [potato] Installing Potato from source (GitHub)...
set "CLONE_DIR=%TEMP%\potato-install-%RANDOM%"
git clone --depth 1 %POTATO_REPO% "%CLONE_DIR%"
if errorlevel 1 (
    if exist "%CLONE_DIR%" rd /s /q "%CLONE_DIR%"
    echo [potato] ERROR: Failed to clone repository
    exit /b 1
)
call :prepare_web_app "%CLONE_DIR%"
echo [potato] Installing package from source...
uv pip install "%CLONE_DIR%%EXTRAS_SUFFIX%" --python "%VENV_PYTHON%"
set "_INST_ERR=%errorlevel%"
if exist "%CLONE_DIR%" rd /s /q "%CLONE_DIR%"
if %_INST_ERR% neq 0 (
    echo [potato] ERROR: Installation from source failed
    exit /b 1
)
goto :install_verify

:install_from_pypi
set "_PACKAGE=qwenpaw"

rem === Secure Validation for ARG_VERSION and EXTRAS_SUFFIX ===
if not defined ARG_VERSION goto :skip_arg_version
powershell -NoProfile -Command "$v=$env:ARG_VERSION; $allowed='ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789.=<>-~'; $allowed+=[char]33; foreach($ch in $v.ToCharArray()){if(-not $allowed.Contains($ch)){exit 1}}"
if errorlevel 1 goto :invalid_package_arguments
set "_PACKAGE=qwenpaw%ARG_VERSION%"

:skip_arg_version
powershell -NoProfile -Command "$e=$env:EXTRAS_SUFFIX; if($e -and $e -notmatch '^\[[A-Za-z0-9,_-]+\]$'){exit 1}"
if errorlevel 1 goto :invalid_package_arguments
rem === End Version Validation ===

echo [potato] Installing %_PACKAGE%%EXTRAS_SUFFIX% from PyPI...
rem Note: It is also recommended to validate EXTRAS_SUFFIX here. Although it may be undefined in the local scope above,
rem for safety, if ARG_EXTRAS is defined globally, it is best to reuse the validation logic from above or ensure its source is secure.
rem Assume EXTRAS_SUFFIX is generated here based on the previously validated ARG_EXTRAS, or is empty.
rem If ARG_EXTRAS is passed globally, it is recommended to validate it uniformly at the beginning of the script.

set "PRERELEASE_ARG="
if "%ARG_PRERELEASE%"=="1" set "PRERELEASE_ARG=--prerelease=allow"

uv pip install "%_PACKAGE%%EXTRAS_SUFFIX%" --python "%VENV_PYTHON%" --quiet --refresh-package qwenpaw %PRERELEASE_ARG%
if errorlevel 1 (
    echo [potato] ERROR: Installation failed
    exit /b 1
)

:install_verify

REM Verify the CLI entry point exists
if not exist "%VENV_POTATO%" (
    echo [potato] ERROR: Installation failed: potato CLI not found in venv
    exit /b 1
)
echo [potato] Potato installed successfully

REM Check web UI availability (for PyPI installs, probe the installed package)
if "%WEB_UI_AVAILABLE%"=="0" (
    "%VENV_PYTHON%" -c "import importlib.resources, potato; p=importlib.resources.files('potato')/'console'/'index.html'; print('yes' if p.is_file() else 'no')" > "%TEMP%\_potato_console_check.tmp" 2>&1
    set /p WEB_UI_CHECK=<"%TEMP%\_potato_console_check.tmp"
    del "%TEMP%\_potato_console_check.tmp" >nul 2>&1
    if "!WEB_UI_CHECK!"=="yes" set "WEB_UI_AVAILABLE=1"
)

REM ──── Step 4: Create wrapper scripts ────────────────────────────────────────────────────────────────────────────────────────
if not exist "%POTATO_BIN%" mkdir "%POTATO_BIN%"

REM PowerShell wrapper
set "WRAPPER_PS1=%POTATO_BIN%\potato.ps1"
echo # Potato CLI wrapper -- delegates to the uv-managed environment. > "%WRAPPER_PS1%"
echo $ErrorActionPreference = "Stop" >> "%WRAPPER_PS1%"
echo. >> "%WRAPPER_PS1%"
echo $PotatoHome = if ($env:POTATO_HOME) { $env:POTATO_HOME } elseif ($env:QWENPAW_HOME) { $env:QWENPAW_HOME } elseif ((Test-Path (Join-Path $HOME ".qwenpaw")) -and -not (Test-Path (Join-Path $HOME ".potato"))) { Join-Path $HOME ".qwenpaw" } else { Join-Path $HOME ".potato" } >> "%WRAPPER_PS1%"
echo $RealBin = Join-Path $PotatoHome "venv\Scripts\potato.exe" >> "%WRAPPER_PS1%"
echo. >> "%WRAPPER_PS1%"
echo if (-not (Test-Path $RealBin)) { >> "%WRAPPER_PS1%"
echo     Write-Error "Potato environment not found at $PotatoHome\venv" >> "%WRAPPER_PS1%"
echo     Write-Error "Please reinstall: irm ^<install-url^> ^| iex" >> "%WRAPPER_PS1%"
echo     exit 1 >> "%WRAPPER_PS1%"
echo } >> "%WRAPPER_PS1%"
echo. >> "%WRAPPER_PS1%"
echo ^& $RealBin @args >> "%WRAPPER_PS1%"
echo [potato] Wrapper created at %WRAPPER_PS1%

REM CMD wrapper
set "WRAPPER_CMD=%POTATO_BIN%\potato.cmd"
echo @echo off > "%WRAPPER_CMD%"
echo REM Potato CLI wrapper -- delegates to the uv-managed environment. >> "%WRAPPER_CMD%"
echo set "POTATO_HOME=%%POTATO_HOME%%" >> "%WRAPPER_CMD%"
echo if "%%POTATO_HOME%%"=="" if not "%%QWENPAW_HOME%%"=="" set "POTATO_HOME=%%QWENPAW_HOME%%" >> "%WRAPPER_CMD%"
echo if "%%POTATO_HOME%%"=="" if exist "%%USERPROFILE%%\.qwenpaw" if not exist "%%USERPROFILE%%\.potato" set "POTATO_HOME=%%USERPROFILE%%\.qwenpaw" >> "%WRAPPER_CMD%"
echo if "%%POTATO_HOME%%"=="" set "POTATO_HOME=%%USERPROFILE%%\.potato" >> "%WRAPPER_CMD%"
echo set "REAL_BIN=%%POTATO_HOME%%\venv\Scripts\potato.exe" >> "%WRAPPER_CMD%"
echo if not exist "%%REAL_BIN%%" ( >> "%WRAPPER_CMD%"
echo     echo Error: Potato environment not found at %%POTATO_HOME%%\venv ^>^&2 >> "%WRAPPER_CMD%"
echo     echo Please reinstall ^>^&2 >> "%WRAPPER_CMD%"
echo     exit /b 1 >> "%WRAPPER_CMD%"
echo ) >> "%WRAPPER_CMD%"
echo "%%REAL_BIN%%" %%* >> "%WRAPPER_CMD%"
echo [potato] CMD wrapper created at %WRAPPER_CMD%

REM ──── Step 5: Update PATH via user environment variable ──────────────────────────────────────────────────
set "CURRENT_USER_PATH="
for /f "skip=2 tokens=1,2,*" %%a in ('reg query "HKCU\Environment" /v Path 2^>nul') do (
    if /i "%%a"=="Path" set "CURRENT_USER_PATH=%%c"
)

:: === 安全检查PATH是否已存在（关键修复） ===
set "path_check=;%CURRENT_USER_PATH%;"
set "check_str=;%POTATO_BIN%;"
if /i "%path_check%" neq "%path_check:%check_str%=%" (
    echo [potato] %POTATO_BIN% already in PATH
) else (
    :: === 修复1：安全传递参数（解决命令注入） ===
    if defined CURRENT_USER_PATH (
        powershell -NoProfile -Command "$p = $args[0]; $v = $args[1]; [Environment]::SetEnvironmentVariable('Path', $p + ';' + $v, 'User')" "%POTATO_BIN%" "!CURRENT_USER_PATH!"
    ) else (
        powershell -NoProfile -Command "$p = $args[0]; [Environment]::SetEnvironmentVariable('Path', $p, 'User')" "%POTATO_BIN%"
    )

    :: === 修复2：添加关键错误检查（解决失败不报错） ===
    if errorlevel 1 (
        echo [error] Failed to update PATH. POTATO_BIN: "%POTATO_BIN%"
        echo [error] Please verify the path is valid.
        exit /b 1
    )

    :: === 修复3：安全更新当前进程PATH ===
    set "PATH=%POTATO_BIN%;!PATH!"
    echo [potato] Added %POTATO_BIN% to PATH
)

REM ──── Done ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
echo.
echo Potato installed successfully!
echo.
echo   Install location:  %POTATO_HOME%
echo   Python:            %PY_VERSION%
if "%WEB_UI_AVAILABLE%"=="1" (
    echo   Console ^(web UI^):  available
) else (
    echo   Console ^(web UI^):  not available
    echo                      Install Node.js and re-run to enable the web UI.
)
echo.
echo To get started, open a new terminal and run:
echo.
echo   potato init       # first-time setup
echo   potato app        # start Potato
echo.
echo To upgrade later, re-run this installer.
echo To uninstall, run: potato uninstall

exit /b 0
