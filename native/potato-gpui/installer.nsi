; Per-user native GPUI installer. No Python, WebView, service or administrator rights.
Unicode true
!include "MUI2.nsh"
!include "x64.nsh"
!include "LogicLib.nsh"
!include "WinVer.nsh"
Name "Potato GPUI"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\Potato GPUI"
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
  ${IfNot} ${AtLeastWin10}
    IfSilent +2
    MessageBox MB_ICONSTOP "Potato GPUI requires Windows 10 or later."
    SetErrorLevel 1
    Abort
  ${EndIf}
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "Potato GPUI requires 64-bit Windows."
    SetErrorLevel 1
    Abort
  ${EndIf}
  SetRegView 64
  SetShellVarContext current
  ; Read after selecting the 64-bit view; NSIS itself is a 32-bit process.
  ReadRegStr $0 HKCU "Software\Potato\GPUI" "InstallDir"
  ${If} $0 != ""
    StrCpy $INSTDIR $0
  ${EndIf}
FunctionEnd

; Check both executables before changing any payload. A running helper may
; outlive its UI; do not partially upgrade/uninstall that installation.
!macro CheckClosed PREFIX
Function ${PREFIX}CheckClosed
  IfFileExists "$INSTDIR\Potato-GPUI.exe" 0 driver
  System::Call 'kernel32::CreateFileW(w "$INSTDIR\Potato-GPUI.exe", i 0xC0000000, i 0, p 0, i 3, i 0, p 0) p.r0'
  StrCmp $0 -1 busy
  System::Call 'kernel32::CloseHandle(p r0)'
driver:
  IfFileExists "$INSTDIR\computer-driver\cua-driver.exe" 0 done
  System::Call 'kernel32::CreateFileW(w "$INSTDIR\computer-driver\cua-driver.exe", i 0xC0000000, i 0, p 0, i 3, i 0, p 0) p.r0'
  StrCmp $0 -1 busy
  System::Call 'kernel32::CloseHandle(p r0)'
  Goto done
busy:
  SetErrorLevel 1
  IfSilent +2
  MessageBox MB_ICONSTOP "Close Potato GPUI and its computer driver before continuing."
  Abort
done:
FunctionEnd
!macroend
!insertmacro CheckClosed ""
!insertmacro CheckClosed "un."

Section "Potato GPUI"
  Call CheckClosed
  SetOutPath "$INSTDIR"
  ; Fail an unattended upgrade if the executable is in use.
  SetOverwrite try
  ClearErrors
  File "${PAYLOAD}\Potato-GPUI.exe"
  ${If} ${Errors}
    SetErrorLevel 1
    Abort "Close Potato GPUI before installing."
  ${EndIf}
  ClearErrors
  File "${PAYLOAD}\icon.ico"
  File "${PAYLOAD}\LICENSE.txt"
  SetOutPath "$INSTDIR\computer-driver"
  File "${PAYLOAD}\computer-driver\cua-driver.exe"
  File "${PAYLOAD}\computer-driver\VERSION"
  ${If} ${Errors}
    SetErrorLevel 1
    Abort "Unable to install the complete application payload."
  ${EndIf}
  SetOutPath "$INSTDIR"
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
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PotatoGPUI" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PotatoGPUI" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PotatoGPUI" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PotatoGPUI" "NoRepair" 1
  ${If} ${Errors}
    SetErrorLevel 1
    Abort "Unable to register the installation or create shortcuts."
  ${EndIf}
SectionEnd

Section "Uninstall"
  SetRegView 64
  SetShellVarContext current
  Call un.CheckClosed
  ClearErrors
  Delete "$INSTDIR\Potato-GPUI.exe"
  Delete "$INSTDIR\icon.ico"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\computer-driver\cua-driver.exe"
  Delete "$INSTDIR\computer-driver\VERSION"
  ${If} ${Errors}
    SetErrorLevel 1
    Abort "Unable to remove the application. Close Potato GPUI and retry."
  ${EndIf}
  RMDir "$INSTDIR\computer-driver"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$DESKTOP\Potato GPUI.lnk"
  Delete "$SMPROGRAMS\Potato GPUI\Potato GPUI.lnk"
  RMDir "$SMPROGRAMS\Potato GPUI"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PotatoGPUI"
  DeleteRegKey HKCU "Software\Potato\GPUI"
  ; User data under ~/.potato/native-v1 is deliberately retained.
SectionEnd
