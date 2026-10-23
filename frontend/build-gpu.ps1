# Compatibility launcher; no retired ASR GPU features are passed to the app.
$ErrorActionPreference = 'Stop'
if ($args.Count -gt 0) { throw 'Retired ASR GPU flags are not supported. Use tauri-auto.js build.' }
Push-Location $PSScriptRoot
try {
    node scripts/tauri-auto.js build
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally { Pop-Location }
