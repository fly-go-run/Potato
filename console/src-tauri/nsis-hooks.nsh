!include LogicLib.nsh
!include nsDialogs.nsh

Var PotatoCliPathCheckbox
Var PotatoCliPathState

Page custom POTATO_CLI_PATH_PAGE POTATO_CLI_PATH_PAGE_LEAVE

!macro POTATO_UPDATE_CLI_PATH ACTION
  InitPluginsDir
  File /oname=$PLUGINSDIR\potato-update-path.ps1 "..\..\..\..\nsis\update-potato-path.ps1"
  nsExec::ExecToStack `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$PLUGINSDIR\potato-update-path.ps1" -Action "${ACTION}" -Path "$INSTDIR\binaries\potato-backend"`
  Pop $0
  Pop $1
!macroend

!macro POTATO_ADD_CLI_PATH_IF_SELECTED
  ${If} $PotatoCliPathState == 0
    DetailPrint "$(potatoCliPathSkipped)"
  ${Else}
    IfFileExists "$INSTDIR\binaries\potato-backend\potato.exe" 0 potato_cli_path_missing
    !insertmacro POTATO_UPDATE_CLI_PATH "Add"
    ${If} $0 == 0
      DetailPrint "$(potatoCliPathAdded)"
    ${Else}
      DetailPrint "$(potatoCliPathUpdateFailed)"
      DetailPrint "$1"
    ${EndIf}
    Goto potato_cli_path_done
    potato_cli_path_missing:
      DetailPrint "$(potatoCliPathMissing)"
    potato_cli_path_done:
  ${EndIf}
!macroend

!macro POTATO_REMOVE_CLI_PATH
  !insertmacro POTATO_UPDATE_CLI_PATH "Remove"
  ${If} $0 != 0
    DetailPrint "$(potatoCliPathUpdateFailed)"
    DetailPrint "$1"
  ${EndIf}
!macroend

!macro POTATO_INSTALL_DEBUG_LAUNCHER
  SetOutPath "$INSTDIR"
  File /oname=potato-desktop-debug.cmd "..\..\..\..\nsis\potato-desktop-debug.cmd"
  File /oname=potato-desktop-debug.ps1 "..\..\..\..\nsis\potato-desktop-debug.ps1"
  CreateShortcut "$SMPROGRAMS\Potato Desktop (Debug).lnk" "$INSTDIR\potato-desktop-debug.cmd" "" "$INSTDIR\potato-desktop.exe" 0
!macroend

!macro POTATO_REMOVE_DEBUG_LAUNCHER
  Delete "$SMPROGRAMS\Potato Desktop (Debug).lnk"
  Delete "$INSTDIR\potato-desktop-debug.cmd"
  Delete "$INSTDIR\potato-desktop-debug.ps1"
!macroend

Function POTATO_CLI_PATH_PAGE
  ; Desktop installs no longer put the bundled CLI on PATH.
  ; Uninstall still removes a leftover PATH entry from older builds.
  StrCpy $PotatoCliPathState 0
  Abort

  nsDialogs::Create 1018
  Pop $0
  ${If} $0 == error
    Abort
  ${EndIf}

  !insertmacro MUI_HEADER_TEXT "$(potatoCliPathPageTitle)" "$(potatoCliPathPageSubtitle)"
  ${NSD_CreateLabel} 0 0 100% 28u "$(potatoCliPathPageDescription)"
  Pop $0
  ${NSD_CreateCheckbox} 0 44u 100% 12u "$(potatoCliPathCheckbox)"
  Pop $PotatoCliPathCheckbox

  ${If} $PotatoCliPathState == 0
    SendMessage $PotatoCliPathCheckbox ${BM_SETCHECK} 0 0
  ${Else}
    SendMessage $PotatoCliPathCheckbox ${BM_SETCHECK} 1 0
  ${EndIf}

  nsDialogs::Show
FunctionEnd

Function POTATO_CLI_PATH_PAGE_LEAVE
  ${NSD_GetState} $PotatoCliPathCheckbox $PotatoCliPathState
FunctionEnd

!macro POTATO_STOP_BACKEND_SIDECAR
  ; The Python backend is a Tauri sidecar, not a user-facing window. A leftover
  ; (possibly orphaned, see #5550) backend keeps its PyInstaller ``.pyd`` modules
  ; memory-mapped, which locks them on Windows. The installer then fails to
  ; overwrite those files and shows the cryptic native "can't write file"
  ; abort/retry/ignore dialog.
  ;
  ; The helper stops only backend processes whose executable lives under
  ; $INSTDIR, so a coexisting Potato install is left untouched. It is
  ; ConstrainedLanguage-safe (WDAC/AppLocker): no ``[System.*]`` static calls,
  ; which throw in that mode and made the previous helper give up silently. It
  ; exits non-zero while a scoped backend is still running; if that persists we
  ; surface a friendly retry prompt rather than the raw OS dialog.
  Push $0
  InitPluginsDir
  File /oname=$PLUGINSDIR\potato-stop-backend-sidecar.ps1 "..\..\..\..\nsis\stop-backend-sidecar.ps1"
  ${Do}
    nsExec::Exec `powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$PLUGINSDIR\potato-stop-backend-sidecar.ps1" -InstallDir "$INSTDIR"`
    Pop $0
    ${If} $0 == 0
      ${ExitDo}
    ${EndIf}
    ; Still running (or could not be stopped). Ask the user; default to Cancel
    ; for silent installs.
    MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "$(potatoStopBackendPrompt)" /SD IDCANCEL IDRETRY +2
    Quit
  ${Loop}
  Pop $0
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro POTATO_STOP_BACKEND_SIDECAR
!macroend

; 家人分发预配置:安装器同目录若带 provision.json(由 make_provision.py
; 生成,含供应商 key/默认模型),复制到用户目录,后端首次启动时应用。
; 详见 src/potato/app/provisioning.py 与 scripts/pack-tauri/RELEASE.md。
!macro POTATO_COPY_PROVISION_FILE
  IfFileExists "$EXEDIR\provision.json" 0 potato_provision_done
    CreateDirectory "$PROFILE\.potato"
    CopyFiles /SILENT "$EXEDIR\provision.json" "$PROFILE\.potato\provision.json"
    DetailPrint "Provisioning file staged to $PROFILE\.potato"
  potato_provision_done:
!macroend

!macro NSIS_HOOK_POSTINSTALL
  !insertmacro POTATO_ADD_CLI_PATH_IF_SELECTED
  !insertmacro POTATO_INSTALL_DEBUG_LAUNCHER
  !insertmacro POTATO_COPY_PROVISION_FILE
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro POTATO_STOP_BACKEND_SIDECAR
  !insertmacro POTATO_REMOVE_DEBUG_LAUNCHER
  !insertmacro POTATO_REMOVE_CLI_PATH
!macroend
