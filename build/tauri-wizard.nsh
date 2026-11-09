!ifndef BUILD_UNINSTALLER
  !macro preInit
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
      ClearErrors
      ExecWait '"$OUTDIR\AliceGameLand-Wizard.exe" --alice-setup "$EXEPATH"' $0
      IfErrors tauriSetupLaunchFailed
      ${If} $0 == 0
        SetOutPath "$TEMP"
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
  ${nsProcess::FindProcess} "${APP_EXECUTABLE_FILENAME}" $R0
  ${If} $R0 == 0
    SetErrorLevel 2
    Quit
  ${EndIf}
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

  ${IfNot} ${Silent}
    System::Call 'kernel32::CreateMutexW(p 0, i 1, w "${APP_GUID}-remove-wizard") p .r0 ?e'
    Pop $1
    ${If} $1 == 183
      Quit
    ${EndIf}

    InitPluginsDir
    SetOutPath "$PLUGINSDIR\wizard"
    ClearErrors
    CopyFiles /SILENT "$INSTDIR\${APP_EXECUTABLE_FILENAME}" "$OUTDIR"
    Rename "$OUTDIR\${APP_EXECUTABLE_FILENAME}" "$OUTDIR\AliceGameLand-Wizard.exe"
    CopyFiles /SILENT "$INSTDIR\${UNINSTALL_FILENAME}" "$OUTDIR"
    IfErrors tauriRemoveLaunchFailed

    StrCpy $0 "/currentuser"
    ${If} $installMode == "all"
      StrCpy $0 "/allusers"
    ${EndIf}
    ExecWait '"$OUTDIR\AliceGameLand-Wizard.exe" --alice-remove "$OUTDIR\${UNINSTALL_FILENAME}" "$INSTDIR" $0' $1
    IfErrors tauriRemoveLaunchFailed
    ${If} $1 == 0
      SetOutPath "$TEMP"
      Quit
    ${EndIf}

    tauriRemoveLaunchFailed:
    MessageBox MB_OK|MB_ICONSTOP "제거 화면을 열지 못했어요. Microsoft Edge WebView2를 확인해 주세요."
    SetOutPath "$TEMP"
    SetErrorLevel 1
    Quit
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
