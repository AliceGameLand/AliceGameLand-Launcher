$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$logBase = Join-Path $env:TEMP ('alice-update-preview-' + [guid]::NewGuid().ToString('N'))
Start-Process -FilePath (Join-Path $root 'node_modules/electron/dist/electron.exe') -ArgumentList 'scripts/update-preview/main.cjs' -WorkingDirectory $root -WindowStyle Hidden -RedirectStandardOutput ($logBase + '.out') -RedirectStandardError ($logBase + '.err')
