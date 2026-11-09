!macro customUnInit
  ${GetParameters} $R0
  ${GetOptions} $R0 "--alice-wizard-root=" $R1
  ${IfNot} ${Errors}
    # A retry must never remove another install if the registered location changed.
    ${If} $R1 != "$INSTDIR"
      SetErrorLevel 2
      Quit
    ${EndIf}
    SetOutPath "$EXEDIR"
  ${EndIf}

  ${IfNot} ${Silent}
  ${AndIfNot} ${isDeleteAppData}
    IfFileExists "$INSTDIR\resources\uninstall-wizard\package.json" 0 aliceUninstallFallback

    System::Call 'kernel32::CreateMutexW(p 0, i 1, w "${APP_GUID}-uninstall") p .r0 ?e'
    Pop $1
    ${If} $1 == 183
      Quit
    ${EndIf}

    # The wizard needs Electron outside the installation directory while NSIS removes it.
    InitPluginsDir
    SetOutPath "$PLUGINSDIR\wizard"
    CreateDirectory "$OUTDIR\resources\app"
    CreateDirectory "$OUTDIR\locales"
    ClearErrors
    CopyFiles /SILENT "$INSTDIR\*.dll" "$OUTDIR"
    CopyFiles /SILENT "$INSTDIR\*.pak" "$OUTDIR"
    CopyFiles /SILENT "$INSTDIR\*.bin" "$OUTDIR"
    CopyFiles /SILENT "$INSTDIR\icudtl.dat" "$OUTDIR"
    CopyFiles /SILENT "$INSTDIR\locales\*.pak" "$OUTDIR\locales"
    CopyFiles /SILENT "$INSTDIR\${APP_EXECUTABLE_FILENAME}" "$OUTDIR"
    Rename "$OUTDIR\${APP_EXECUTABLE_FILENAME}" "$OUTDIR\AliceGameLand-Uninstall.exe"
    CopyFiles /SILENT "$INSTDIR\resources\uninstall-wizard\*.*" "$OUTDIR\resources\app"
    CopyFiles /SILENT "$INSTDIR\${UNINSTALL_FILENAME}" "$PLUGINSDIR"
    IfErrors aliceUninstallFallback

    StrCpy $0 "/currentuser"
    ${If} $installMode == "all"
      StrCpy $0 "/allusers"
    ${EndIf}
    ExecWait '"$OUTDIR\AliceGameLand-Uninstall.exe" "$INSTDIR" "$PLUGINSDIR\${UNINSTALL_FILENAME}" $0 "${VERSION}"' $1
    IfErrors aliceUninstallFallback
    ${If} $1 == 0
      SetOutPath "$TEMP"
      Quit
    ${EndIf}

    aliceUninstallFallback:
    SetOutPath "$INSTDIR"
  ${EndIf}
!macroend
