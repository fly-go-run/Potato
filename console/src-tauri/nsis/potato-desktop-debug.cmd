@echo off
setlocal
chcp 65001 >nul
cd /d "%~dp0"

if not defined POTATO_LOG_LEVEL set "POTATO_LOG_LEVEL=debug"
set "POTATO_DESKTOP_DEBUG=1"
set "RUST_BACKTRACE=1"
if not defined WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS set "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222"

set "POTATO_DEBUG_DIR=%POTATO_WORKING_DIR%"
if not defined POTATO_DEBUG_DIR if defined QWENPAW_WORKING_DIR set "POTATO_DEBUG_DIR=%QWENPAW_WORKING_DIR%"
if not defined POTATO_DEBUG_DIR if defined COPAW_WORKING_DIR set "POTATO_DEBUG_DIR=%COPAW_WORKING_DIR%"
if not defined POTATO_DEBUG_DIR if exist "%USERPROFILE%\.potato" set "POTATO_DEBUG_DIR=%USERPROFILE%\.potato"
if not defined POTATO_DEBUG_DIR if exist "%USERPROFILE%\.qwenpaw" set "POTATO_DEBUG_DIR=%USERPROFILE%\.qwenpaw"
if not defined POTATO_DEBUG_DIR if exist "%USERPROFILE%\.copaw" set "POTATO_DEBUG_DIR=%USERPROFILE%\.copaw"
if not defined POTATO_DEBUG_DIR set "POTATO_DEBUG_DIR=%USERPROFILE%\.potato"
set "POTATO_BACKEND_LOGS=%POTATO_DEBUG_DIR%\desktop.log;%POTATO_DEBUG_DIR%\potato.log;%POTATO_DEBUG_DIR%\qwenpaw.log"
set "POTATO_SHELL_LOGS=%LOCALAPPDATA%\io.agentscope.qwenpaw.desktop\logs\potato-desktop.log;%LOCALAPPDATA%\io.agentscope.qwenpaw.desktop\logs\qwenpaw-desktop.log;%LOCALAPPDATA%\com.qwenpaw.desktop\logs\potato-desktop.log;%LOCALAPPDATA%\com.qwenpaw.desktop\logs\qwenpaw-desktop.log"

echo ====================================
echo Potato Desktop - Debug Mode
echo ====================================
echo Log level: %POTATO_LOG_LEVEL%
echo Working directory: %POTATO_DEBUG_DIR%
echo Press Ctrl+C to stop watching logs.
echo.

powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0potato-desktop-debug.ps1"
