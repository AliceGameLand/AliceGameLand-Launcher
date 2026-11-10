!ifndef BUILD_UNINSTALLER
  !define ALICE_WEBVIEW2_KEY "Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"

  !macro preInit
    # Prefer the machine runtime. A machine install also needs it for other accounts.
    SetRegView 32
    ReadRegStr $R2 HKLM "${ALICE_WEBVIEW2_KEY}" "pv"
    ${If} $R2 == ""
    ${OrIf} $R2 == "0.0.0.0"
      ${GetParameters} $R0
      ClearErrors
      ${GetOptions} $R0 "/allusers" $R1
      ${If} ${Errors}
        ReadRegStr $R2 HKCU "${ALICE_WEBVIEW2_KEY}" "pv"
      ${EndIf}
    ${EndIf}
    SetRegView 64
    ${If} $R2 == ""
    ${OrIf} $R2 == "0.0.0.0"
      InitPluginsDir
      File /oname=$PLUGINSDIR\MicrosoftEdgeWebView2Setup.exe "$%ALICE_WEBVIEW2_SETUP%"
      ClearErrors
      ExecWait '"$PLUGINSDIR\MicrosoftEdgeWebView2Setup.exe" /silent /install' $R2
      ${If} ${Errors}
      ${OrIf} $R2 != 0
        ${IfNot} ${Silent}
          MessageBox MB_OK|MB_ICONSTOP "WebView2를 설치하지 못했어요. 인터넷 연결을 확인하고 다시 실행해 주세요."
        ${EndIf}
        SetErrorLevel 1
        Quit
      ${EndIf}
    ${EndIf}
    ${IfNot} ${Silent}
      System::Call 'kernel32::CreateMutexW(p 0, i 1, w "${APP_GUID}-setup-wizard") p .r0 ?e'
      Pop $1
      ${If} $1 == 183
        Quit
      ${EndIf}

      InitPluginsDir
      SetOutPath "$PLUGINSDIR\wizard"
      # Reuse the same compressed app archive consumed by the silent install section.
      !ifdef COMPRESS
        SetCompress off
      !endif
      File /oname=$PLUGINSDIR\app-64.7z "${APP_64}"
      !ifdef COMPRESS
        SetCompress "${COMPRESS}"
      !endif
      Nsis7z::Extract "$PLUGINSDIR\app-64.7z"
      Pop $0
      Rename "$OUTDIR\${APP_EXECUTABLE_FILENAME}" "$OUTDIR\AliceGameLand-Wizard.exe"
      StrCpy $R6 ""
      ${GetParameters} $R0
      ClearErrors
      ${GetOptions} $R0 "--alice-update-currentuser" $R1
      ${IfNot} ${Errors}
        StrCpy $R6 "--alice-update-currentuser"
      ${Else}
        ClearErrors
        ${GetOptions} $R0 "--alice-update-allusers" $R1
        ${IfNot} ${Errors}
          StrCpy $R6 "--alice-update-allusers"
        ${EndIf}
      ${EndIf}
      ClearErrors
      ExecWait '"$OUTDIR\AliceGameLand-Wizard.exe" --alice-setup "$EXEPATH" $R6' $0
      IfErrors tauriSetupLaunchFailed
      ${If} $0 == 0
        SetOutPath "$TEMP"
        SetErrorLevel 0
        Quit
      ${EndIf}
      tauriSetupLaunchFailed:
      MessageBox MB_OK|MB_ICONSTOP "설치 화면을 열지 못했어요. Microsoft Edge WebView2를 확인해 주세요."
      SetOutPath "$TEMP"
      SetErrorLevel 1
      Quit
    ${EndIf}
  !macroend
!endif

!macro customCheckAppRunning
  !ifdef BUILD_UNINSTALLER
    # The removal wizard already stops the launcher at the registered path.
    ${GetParameters} $R0
    ClearErrors
    ${GetOptions} $R0 "--alice-wizard-root=" $R1
    ${If} ${Errors}
      ${nsProcess::FindProcess} "${APP_EXECUTABLE_FILENAME}" $R0
      ${If} $R0 == 0
        SetErrorLevel 2
        Quit
      ${EndIf}
    ${EndIf}
  !else
    ${nsProcess::FindProcess} "${APP_EXECUTABLE_FILENAME}" $R0
    ${If} $R0 == 0
      SetErrorLevel 2
      Quit
    ${EndIf}
  !endif
!macroend

!macro preUnInit
  # Show WebView2 before the stock uninstaller can request elevation.
  ReadRegStr $R2 HKCU "${INSTALL_REGISTRY_KEY}" InstallLocation
  ReadRegStr $R3 HKLM "${INSTALL_REGISTRY_KEY}" InstallLocation
  ${If} $R2 == "$INSTDIR"
    StrCpy $R4 "/currentuser"
  ${ElseIf} $R3 == "$INSTDIR"
    StrCpy $R4 "/allusers"
  ${Else}
    SetErrorLevel 1
    Quit
  ${EndIf}
  ${If} $INSTDIR == ""
    SetErrorLevel 1
    Quit
  ${EndIf}

  System::Call 'kernel32::CreateMutexW(p 0, i 1, w "${APP_GUID}-remove-wizard") p .r0 ?e'
  Pop $1
  ${If} $1 == 183
    SetErrorLevel 0
    Quit
  ${EndIf}

  InitPluginsDir
  SetOutPath "$PLUGINSDIR\wizard"
  ClearErrors
  CopyFiles /SILENT "$INSTDIR\${APP_EXECUTABLE_FILENAME}" "$OUTDIR"
  Rename "$OUTDIR\${APP_EXECUTABLE_FILENAME}" "$OUTDIR\AliceGameLand-Wizard.exe"
  StrCpy $R5 "$INSTDIR\${UNINSTALL_FILENAME}"
  ${If} $R4 == "/currentuser"
    CopyFiles /SILENT "$R5" "$OUTDIR"
    StrCpy $R5 "$OUTDIR\${UNINSTALL_FILENAME}"
  ${EndIf}
  IfErrors tauriRemoveLaunchFailed

  ExecWait '"$OUTDIR\AliceGameLand-Wizard.exe" --alice-remove "$R5" "$INSTDIR" $R4' $1
  IfErrors tauriRemoveLaunchFailed
  ${If} $1 == 0
    SetOutPath "$TEMP"
    SetErrorLevel 0
    Quit
  ${EndIf}

  tauriRemoveLaunchFailed:
  SetOutPath "$TEMP"
  MessageBox MB_YESNO|MB_ICONQUESTION "제거 화면을 열지 못했어요. Windows 제거 화면을 열까요?" IDYES tauriRemoveNative
  SetErrorLevel 1
  Quit
  tauriRemoveNative:
!macroend

!macro customUnInit
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "--alice-wizard-root=" $R1
  ${IfNot} ${Errors}
    ${If} $R1 != "$INSTDIR"
      SetErrorLevel 1
      Quit
    ${EndIf}
    SetOutPath "$EXEDIR"
  ${EndIf}
!macroend

!macro customRemoveFiles
  SetOutPath "$TEMP"
  Delete "$INSTDIR\${APP_EXECUTABLE_FILENAME}"
  IfFileExists "$INSTDIR\${APP_EXECUTABLE_FILENAME}" tauriRemoveBusy
  Delete "$INSTDIR\${UNINSTALL_FILENAME}"
  IfFileExists "$INSTDIR\${UNINSTALL_FILENAME}" tauriRemoveBusy
  # Leave any files added by the user and remove the install directory when empty.
  RMDir "$INSTDIR"
  Goto tauriRemoveDone
  tauriRemoveBusy:
  SetErrorLevel 2
  Quit
  tauriRemoveDone:
!macroend
