!macro customInstall
  DetailPrint "Register scarlet URI Handler"
  DeleteRegKey HKCR "scarlet"
  WriteRegStr HKCR "scarlet" "" "URL:scarlet"
  WriteRegStr HKCR "scarlet" "Scarlet URL Handler" ""
  WriteRegStr HKCR "scarlet\DefaultIcon" "" "$INSTDIR\${APP_EXECUTABLE_FILENAME}"
  WriteRegStr HKCR "scarlet\shell" "" ""
  WriteRegStr HKCR "scarlet\shell\Open" "" ""
  WriteRegStr HKCR "scarlet\shell\Open\command" "" "$INSTDIR\${APP_EXECUTABLE_FILENAME} %1"
!macroend