# Meng-generate emojis.json dari dataset gemoji (MIT, github/gemoji).
# Mengunduh sekali, konversi ke format {emoji,name,keywords}, escaping \uXXXX
# oleh ConvertTo-Json (aman untuk karakter di luar BMP).

$ErrorActionPreference = "Stop"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$assets = Join-Path $PSScriptRoot "..\crates\sun-core\assets"
$src    = Join-Path $assets "gemoji-src.json"
$out    = Join-Path $assets "emojis.json"

if (!(Test-Path $src)) {
    Invoke-WebRequest `
        -Uri "https://raw.githubusercontent.com/github/gemoji/master/db/emoji.json" `
        -OutFile $src | Out-Null
}

$raw = [IO.File]::ReadAllText($src, [Text.Encoding]::UTF8) | ConvertFrom-Json

$items = @()
foreach ($e in $raw) {
    if ([string]::IsNullOrEmpty($e.emoji)) { continue }         # entri tanpa glyph (mis. :octocat:)
    $kw = @()
    if ($e.aliases) { $kw += $e.aliases }
    if ($e.tags)    { $kw += $e.tags }
    $kw = $kw | Where-Object { $_ } | Select-Object -Unique
    $items += [pscustomobject]@{
        emoji    = $e.emoji
        name     = $e.description
        keywords = ($kw -join " ")
    }
}

$json = ConvertTo-Json @($items) -Compress -Depth 4
[IO.File]::WriteAllText($out, $json, (New-Object System.Text.UTF8Encoding($false)))
Write-Host "OK: $out ($($items.Count) emoji)"
