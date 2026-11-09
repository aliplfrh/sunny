# Build the NSIS installer + updater signature and write `latest.json`.
#
# Usage (from the repo root, PowerShell):
#   powershell -File scripts/release.ps1
#
# Requires the updater private key at %USERPROFILE%\.tauri\sunny.key
# (or set TAURI_SIGNING_PRIVATE_KEY yourself). The public half of that key
# must match `plugins.updater.pubkey` in apps/sunny/src-tauri/tauri.conf.json.

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$confPath = Join-Path $root "apps\sunny\src-tauri\tauri.conf.json"
$conf = Get-Content $confPath -Raw | ConvertFrom-Json
$version = $conf.version
$tag = "v$version"

if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
    $keyPath = Join-Path $env:USERPROFILE ".tauri\sunny.key"
    if (-not (Test-Path $keyPath)) {
        throw "Updater private key not found at $keyPath. Run: npx @tauri-apps/cli@2 signer generate -w `"$keyPath`""
    }
    $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $keyPath -Raw
}

Write-Host "==> Building Sunny $version (signed updater artifacts)" -ForegroundColor Cyan
Push-Location (Join-Path $root "apps\sunny")
try {
    npx --yes @tauri-apps/cli@2 build
} finally {
    Pop-Location
}

$bundleDir = Join-Path $root "target\release\bundle\nsis"
$setup = Get-ChildItem $bundleDir -Filter "*-setup.exe" | Select-Object -First 1
if (-not $setup) { throw "NSIS installer not found in $bundleDir" }
$sig = Get-ChildItem $bundleDir -Filter "*-setup.exe.sig" | Select-Object -First 1
if (-not $sig) { throw "Updater signature not found — createUpdaterArtifacts must be true" }

$signature = (Get-Content $sig.FullName -Raw).Trim()
$manifest = [ordered]@{
    version   = $version
    notes     = "Sunny $tag"
    pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    platforms = [ordered]@{
        "windows-x86_64" = [ordered]@{
            signature = $signature
            url       = "https://github.com/aliplfrh/sunny/releases/download/$tag/$($setup.Name)"
        }
    }
}
$latestPath = Join-Path $root "latest.json"
$manifest | ConvertTo-Json -Depth 6 | Set-Content $latestPath -Encoding utf8

Write-Host ""
Write-Host "Installer : $($setup.FullName)" -ForegroundColor Green
Write-Host "Signature : $($sig.FullName)" -ForegroundColor Green
Write-Host "Manifest  : $latestPath" -ForegroundColor Green
Write-Host ""
Write-Host "Publish with:" -ForegroundColor Cyan
Write-Host "  gh release create $tag `"$($setup.FullName)`" `"$($sig.FullName)`" latest.json --title `"Sunny $tag`" --latest"