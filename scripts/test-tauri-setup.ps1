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
    if ($mode -eq 'currentuser') {
        Invoke-NsisWorker $installerPath "/S --updated /$mode /D=$root" $fixture
        if ((Get-Content -LiteralPath $userFile) -ne 'keep' -or !(Find-Preview $hive)) {
            throw 'Updating the existing installation did not preserve its state'
        }
        Remove-Item -LiteralPath $userFile
    }

    # The UI also runs a temporary copy, so the registered uninstaller can be deleted.
    $worker = Join-Path $fixture 'remove-worker.exe'
    Copy-Item -LiteralPath $uninstaller[0].FullName -Destination $worker
    Invoke-NsisWorker $worker ('/S /{0} --alice-wizard-root="{1}" _?={1}' -f $mode, $root) $fixture
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
    Remove-Item -LiteralPath $worker
    Remove-Item -LiteralPath $fixture
    Write-Output "$mode installation and removal passed ($hive)"
}

if ((Get-FileHash -LiteralPath $legacyCache -ErrorAction SilentlyContinue).Hash -ne $legacyBefore) {
    throw 'The Tauri installer changed the Electron updater cache'
}
