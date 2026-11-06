; Installs native Nitro over the Tauri app, in the same place and under the same uninstall
; entry, so there's only ever one Nitro. The old updater runs this silently, with
; /S /R /UPDATE /ARGS, then exits at once. Nothing else gets a turn, so this replaces the
; program and starts it again. The new app brings over settings and sign-in on first launch.
; Built by scripts/build-release.ps1, which passes VERSION, ICON and PAYLOAD.

Unicode true
!include "FileFunc.nsh"
!include "LogicLib.nsh"
!insertmacro GetParameters
!insertmacro GetOptions

; Pass /DAPP_ID=SomethingElse to try the whole install without touching a real Nitro.
!ifndef APP_ID
  !define APP_ID "Nitro"
!endif
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}"

Name "Nitro"
OutFile "${OUTPUT}"
Icon "${ICON}"
UninstallIcon "${ICON}"
InstallDir "$LOCALAPPDATA\${APP_ID}"
RequestExecutionLevel user
ShowInstDetails nevershow
AutoCloseWindow true

Page instfiles
UninstPage instfiles

Var Relaunch

Function .onInit
  ${GetParameters} $0
  ClearErrors
  ${GetOptions} $0 "/R" $1
  ${IfNot} ${Errors}
    StrCpy $Relaunch "1"
  ${EndIf}
  ; Wherever it's really installed, which can differ from the default.
  ReadRegStr $1 HKCU "${UNINSTALL_KEY}" "InstallLocation"
  ${If} $1 != ""
    StrCpy $2 $1 1
    ${If} $2 == '"'
      StrCpy $1 $1 "" 1
      StrCpy $1 $1 -1
    ${EndIf}
    StrCpy $INSTDIR $1
  ${EndIf}
FunctionEnd

; The old app exits as its updater starts this, but can take a moment to let go of its files.
Function CloseRunningApp
  StrCpy $2 0
  retry:
    ClearErrors
    Delete "$INSTDIR\nitro.exe"
    IfErrors 0 done
    IntOp $2 $2 + 1
    ${If} $2 == 3
      nsExec::Exec 'taskkill /IM nitro.exe'
      Pop $3
    ${EndIf}
    ${If} $2 == 12
      nsExec::Exec 'taskkill /F /IM nitro.exe'
      Pop $3
    ${EndIf}
    ${If} $2 > 20
      MessageBox MB_OK|MB_ICONSTOP "Nitro couldn't be updated because it's still running. Close it and try again." /SD IDOK
      SetErrorLevel 2
      Quit
    ${EndIf}
    Sleep 500
    Goto retry
  done:
  ; A call left running can hold the engine. Only this app's engine has this name.
  nsExec::Exec 'taskkill /F /IM nitro-voice-engine.exe'
  Pop $3
FunctionEnd

Section "Nitro"
  SetOutPath "$INSTDIR"
  Call CloseRunningApp
  File /r "${PAYLOAD}\*.*"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; The same entry the Tauri installer made, so Windows sees an update, not a second app.
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "Nitro"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" '"$INSTDIR\nitro.exe"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "MainBinaryName" "nitro.exe"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "github"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" '"$INSTDIR"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "EstimatedSize" "$0"

  Delete "$SMPROGRAMS\Dislight.lnk"
  CreateShortCut "$SMPROGRAMS\${APP_ID}.lnk" "$INSTDIR\nitro.exe"

  ${If} $Relaunch == "1"
    Exec '"$INSTDIR\nitro.exe"'
  ${EndIf}
SectionEnd

; Removes the program only. Chats live on the server, and settings and the old app's
; data stay in case of a reinstall.
Section "Uninstall"
  nsExec::Exec 'taskkill /F /IM nitro-voice-engine.exe'
  Pop $3
  Delete "$INSTDIR\nitro.exe"
  Delete "$INSTDIR\nitro-voice-engine.exe"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\${APP_ID}.lnk"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
SectionEnd
