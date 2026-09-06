; Per-user native GPUI installer. No Python, WebView, service or administrator rights.
Unicode true
!include "MUI2.nsh"
!include "x64.nsh"
!include "LogicLib.nsh"
Name "Potato GPUI"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\Potato GPUI"
InstallDirRegKey HKCU "Software\Potato\GPUI" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
VIProductVersion "${NUMERIC_VERSION}"
VIAddVersionKey /LANG=1033 "ProductName" "Potato GPUI"
VIAddVersionKey /LANG=1033 "FileDescription" "Potato GPUI Installer"
VIAddVersionKey /LANG=1033 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "LegalCopyright" "Potato contributors"
!define MUI_ICON "${PAYLOAD}\icon.ico"
!define MUI_UNICON "${PAYLOAD}\icon.ico"
!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${PAYLOAD}\LICENSE.txt"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "SimpChinese"

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "Potato GPUI requires 64-bit Windows."
    SetErrorLevel 1
    Abort
  ${EndIf}
  SetRegView 64
  SetShellVarContext current
FunctionEnd

Section "Potato GPUI"
  SetOutPath "$INSTDIR"
  ; Fail an unattended upgrade if the executable is in use.
  SetOverwrite try
  ClearErrors
  File "${PAYLOAD}\Potato-GPUI.exe"
  ${If} ${Errors}
    SetErrorLevel 1
    Abort "Close Potato GPUI before installing."
  ${EndIf}
  SetOverwrite on
  File "${PAYLOAD}\icon.ico"
  File "${PAYLOAD}\LICENSE.txt"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateDirectory "$SMPROGRAMS\Potato GPUI"
  CreateShortcut "$SMPROGRAMS\Potato GPUI\Potato GPUI.lnk" "$INSTDIR\Potato-GPUI.exe" "" "$INSTDIR\icon.ico"
  CreateShortcut "$DESKTOP\Potato GPUI.lnk" "$INSTDIR\Potato-GPUI.exe" "" "$INSTDIR\icon.ico"
  WriteRegStr HKCU "Software\Potato\GPUI" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PotatoGPUI" "DisplayName" "Potato GPUI"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PotatoGPUI" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PotatoGPUI" "Publisher" "Potato contributors"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PotatoGPUI" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PotatoGPUI" "DisplayIcon" "$INSTDIR\icon.ico"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PotatoGPUI" "UninstallString" '$"$INSTDIR\Uninstall.exe$"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PotatoGPUI" "QuietUninstallString" '$"$INSTDIR\Uninstall.exe$" /S'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PotatoGPUI" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PotatoGPUI" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetRegView 64
  SetShellVarContext current
  Delete "$INSTDIR\Potato-GPUI.exe"
  Delete "$INSTDIR\icon.ico"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$DESKTOP\Potato GPUI.lnk"
  Delete "$SMPROGRAMS\Potato GPUI\Potato GPUI.lnk"
  RMDir "$SMPROGRAMS\Potato GPUI"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PotatoGPUI"
  DeleteRegKey HKCU "Software\Potato\GPUI"
  ; User data under ~/.potato/native-v1 is deliberately retained.
SectionEnd
