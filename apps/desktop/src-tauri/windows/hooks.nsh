; Browsers keep havenkeys-native-host.exe running (it waits for the app to
; come back), and Windows cannot overwrite a running program. End it before
; the files are copied; the extension starts a new one on its next request.
; taskkill only reaches this user's processes without elevation, which is
; where a per-user install's browsers run.
!macro NSIS_HOOK_PREINSTALL
  nsExec::Exec 'taskkill /F /IM havenkeys-native-host.exe'
  Pop $0
!macroend
