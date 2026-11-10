param(
    [Parameter(Mandatory)]
    [string]$Installer,
    [ValidateSet('currentuser', 'allusers')]
    [string[]]$Scope = @('currentuser', 'allusers')
)

$ErrorActionPreference = 'Stop'
$installerPath = (Resolve-Path -LiteralPath $Installer).Path
$uninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\69bd89cc-35ff-5408-9b98-972ba4fb446c'

function Find-Preview([string]$Hive) {
    Get-ItemProperty -LiteralPath ("Registry::{0}\{1}" -f $Hive, $uninstallKey) -ErrorAction SilentlyContinue
}
function Invoke-NsisWorker([string]$Executable, [string]$Arguments, [string]$Directory) {
    $start = [System.Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $Executable
    $start.Arguments = $Arguments
    $start.WorkingDirectory = $Directory
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $process = [System.Diagnostics.Process]::Start($start)
    try {
        if (!$process.WaitForExit(60000)) {
            $process.Kill()
            throw 'NSIS worker timed out'
        }
        if ($process.ExitCode -ne 0) {
            throw "NSIS worker failed with exit code $($process.ExitCode)"
        }
    } finally {
        $process.Dispose()
    }
}

if ((Find-Preview 'HKEY_CURRENT_USER') -or (Find-Preview 'HKEY_LOCAL_MACHINE')) {
    throw 'Run this test on a machine without an existing Tauri preview installation.'
}

$legacyCache = Join-Path $env:LOCALAPPDATA 'alice-gameland-launcher-updater/installer.exe'
$legacyBefore = (Get-FileHash -LiteralPath $legacyCache -ErrorAction SilentlyContinue).Hash

foreach ($mode in $Scope) {
    $hive = if ($mode -eq 'allusers') { 'HKEY_LOCAL_MACHINE' } else { 'HKEY_CURRENT_USER' }
    $fixture = Join-Path ([IO.Path]::GetTempPath()) ('Alice Tauri setup test ' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $fixture | Out-Null
    $root = Join-Path $fixture 'alice-gameland-tauri-preview'
    Invoke-NsisWorker $installerPath "/S /$mode /D=$root" $fixture

    $files = @(Get-ChildItem -LiteralPath $root -File)
    $uninstaller = @($files | Where-Object Name -like 'Uninstall*.exe')
    if ($files.Count -ne 2 -or $uninstaller.Count -ne 1 -or $files.Name -notcontains 'AliceGameLand-Tauri.exe') {
        throw "$mode produced unexpected install files"
    }
    $registration = Find-Preview $hive
    if ($registration.UninstallString -ne ('"{0}" /{1}' -f $uninstaller[0].FullName, $mode)) {
        throw "$mode did not register the preview in $hive"
    }
    $userFile = Join-Path $root 'user-settings.txt'
    Set-Content -LiteralPath $userFile -Value 'keep'
    $installedLauncher = Join-Path $root 'AliceGameLand-Tauri.exe'
    $expectedPayload = (Get-FileHash -LiteralPath $installedLauncher).Hash
    Set-Content -LiteralPath $installedLauncher -Value 'previous launcher fixture'
    Invoke-NsisWorker $installerPath "/S --updated /$mode /D=$root" $fixture
    $updated = Find-Preview $hive
    if ((Get-FileHash -LiteralPath $installedLauncher).Hash -ne $expectedPayload -or
        (Get-Content -LiteralPath $userFile) -ne 'keep' -or
        $updated.UninstallString -ne $registration.UninstallString) {
        throw "$mode update did not replace the launcher and preserve the installation state"
    }
    Write-Output "$mode in-place update passed ($hive)"
    $env:ALICE_TAURI_TEST_PROFILE = $fixture
    $env:ALICE_TAURI_VERIFY_REMOVE_WIZARD = '1'
    try {
        Invoke-NsisWorker $uninstaller[0].FullName "/$mode" $fixture
        $ready = Join-Path $fixture 'remove-wizard-ready'
        $wait = [Diagnostics.Stopwatch]::StartNew()
        while (!(Test-Path -LiteralPath $ready) -or (Get-Process -Name 'AliceGameLand-Wizard' -ErrorAction SilentlyContinue)) {
            if ($wait.Elapsed.TotalSeconds -ge 25) { throw "$mode Control Panel removal window did not load and close" }
            Start-Sleep -Milliseconds 100
        }
        if (!(Find-Preview $hive)) { throw "$mode cancellation removed the installation" }
        Remove-Item -LiteralPath $ready
        Write-Output "$mode Control Panel removal window passed ($hive)"
    } finally {
        Remove-Item Env:ALICE_TAURI_TEST_PROFILE, Env:ALICE_TAURI_VERIFY_REMOVE_WIZARD
    }
    if ($mode -eq 'currentuser') {
        Remove-Item -LiteralPath $userFile
    }

    $env:ALICE_TAURI_TEST_PROFILE = $fixture
    $env:ALICE_TAURI_VERIFY_LOGIN = '1'
    $env:ALICE_TAURI_VERIFY_REMOVE_WIZARD = 'remove'
    $env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $fixture 'launcher-webview'
    $launcher = $null
    try {
        $launcher = Start-Process -FilePath $installedLauncher -WorkingDirectory $fixture -WindowStyle Hidden -PassThru
        $wait = [Diagnostics.Stopwatch]::StartNew()
        while (!(Test-Path -LiteralPath (Join-Path $fixture 'login-ready'))) {
            if ($launcher.HasExited -or $wait.Elapsed.TotalSeconds -ge 40) {
                throw "$mode installed launcher did not start"
            }
            Start-Sleep -Milliseconds 100
        }
        if ($launcher.HasExited) { throw "$mode launcher was not running before uninstall" }
        Invoke-NsisWorker $uninstaller[0].FullName "/$mode" $fixture
        if (!$launcher.WaitForExit(5000)) { throw "$mode uninstall did not stop the launcher" }
        if (!(Test-Path -LiteralPath $ready)) { throw "$mode removal window was not loaded" }
    } finally {
        if ($launcher) {
            if (!$launcher.HasExited) { Stop-Process -Id $launcher.Id }
            $launcher.Dispose()
        }
        Remove-Item Env:ALICE_TAURI_TEST_PROFILE, Env:ALICE_TAURI_VERIFY_LOGIN, Env:ALICE_TAURI_VERIFY_REMOVE_WIZARD, Env:WEBVIEW2_USER_DATA_FOLDER
    }
    if (Find-Preview $hive) {
        throw "$mode left an uninstall registration"
    }
    if ($mode -eq 'allusers') {
        if (@(Get-ChildItem -LiteralPath $root).Name -ne 'user-settings.txt' -or (Get-Content -LiteralPath $userFile) -ne 'keep') {
            throw 'Removal did not preserve the user file'
        }
        Remove-Item -LiteralPath $userFile
        Remove-Item -LiteralPath $root
    } elseif (Test-Path -LiteralPath $root) {
        throw 'Removal left an empty installation folder'
    }
    Remove-Item -LiteralPath $fixture
    Write-Output "$mode installation and removal passed ($hive)"
}

if ((Get-FileHash -LiteralPath $legacyCache -ErrorAction SilentlyContinue).Hash -ne $legacyBefore) {
    throw 'The Tauri installer changed the Electron updater cache'
}
