; Browsers keep havenkeys-native-host.exe running (it waits for the app to
; come back), and Windows cannot overwrite a running program. End it before
; the files are copied; the extension starts a new one on its next request.
; taskkill only reaches this user's processes without elevation, which is
; where a per-user install's browsers run. The full System32 path keeps a
; taskkill.exe planted next to the installer (e.g. in Downloads) from running.
; Its exit code is ignored: "no such process" is the usual, harmless case.
!macro NSIS_HOOK_PREINSTALL
  nsExec::Exec '"$SYSDIR\taskkill.exe" /F /IM havenkeys-native-host.exe'
  Pop $0
!macroend
