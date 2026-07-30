; One-time migration from the Dislight installation identity to Nitro.
; The Tauri identifier deliberately stays unchanged for updater compatibility,
; but NSIS uses the product name for its install folder and shortcut.

!macro NSIS_HOOK_PREINSTALL
  Delete "$SMPROGRAMS\Dislight.lnk"
  RMDir /r "$LOCALAPPDATA\Dislight"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Dislight"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  Delete "$SMPROGRAMS\Dislight.lnk"
  CreateShortCut "$SMPROGRAMS\Nitro.lnk" "$INSTDIR\nitro.exe"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  Delete "$SMPROGRAMS\Nitro.lnk"
!macroend
