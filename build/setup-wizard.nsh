!include "${BUILD_RESOURCES_DIR}\uninstaller.nsh"

!ifndef BUILD_UNINSTALLER
  !macro customHeader
    SetDatablockOptimize on
    CRCCheck force
  !macroend

  !macro preInit
    ${IfNot} ${Silent}
      !insertmacro check64BitAndSetRegView
      System::Call 'kernel32::CreateMutexW(p 0, i 1, w "${APP_GUID}-setup-wizard") p .r0 ?e'
      Pop $1
      ${If} $1 == 183
        SetErrorLevel 0
        Quit
      ${EndIf}

      InitPluginsDir
      SetOutPath "$PLUGINSDIR\wizard"
      # Store the same bytes as the stock silent install section. NSIS deduplicates the data block.
      !ifdef COMPRESS
        SetCompress off
      !endif
      File /oname=$PLUGINSDIR\app-64.${COMPRESSION_METHOD} "${APP_64}"
      !ifdef COMPRESS
        SetCompress "${COMPRESS}"
      !endif
      Nsis7z::Extract "$PLUGINSDIR\app-64.7z"
      Pop $0
      Delete "$PLUGINSDIR\app-64.7z"

      ClearErrors
      Rename "$OUTDIR\resources\app.asar" "$PLUGINSDIR\launcher.asar"
      Rename "$OUTDIR\resources\setup-wizard" "$OUTDIR\resources\app"
      Rename "$OUTDIR\${APP_EXECUTABLE_FILENAME}" "$OUTDIR\AliceGameLand-Setup.exe"
      IfErrors aliceSetupLaunchFailed
      StrCpy $R3 ""
      ${If} ${isUpdated}
        StrCpy $R3 "--updated"
      ${EndIf}
      StrCpy $R2 "$OUTDIR\AliceGameLand-Setup.exe"
      # Do not hold the registered install directory open during an upgrade.
      SetOutPath "$TEMP"
      ClearErrors
      ExecWait '"$R2" --alice-installer "$EXEPATH" $R3' $0
      IfErrors aliceSetupLaunchFailed
      ${If} $0 == 0
        SetErrorLevel 0
        Quit
      ${EndIf}

      aliceSetupLaunchFailed:
      MessageBox MB_OK|MB_ICONSTOP "설치 화면을 열지 못했습니다. 설치파일을 다시 받아 주세요."
      SetOutPath "$TEMP"
      SetErrorLevel 1
      Quit
    ${EndIf}
  !macroend
!endif
