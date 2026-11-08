# Membuat icon.ico (multi-ukuran) + icon.png dari "Sunny Logo Packs\Sunny Icon.png".
# Dipakai oleh tauri-build (resource exe Windows) dan tray icon aplikasi.
param(
    [string]$Source = (Join-Path $PSScriptRoot "..\Sunny Logo Packs\Sunny Icon.png"),
    [string]$OutDir = (Join-Path $PSScriptRoot "..\apps\sunny\src-tauri\icons")
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

if (!(Test-Path $Source)) { throw "Sumber logo tidak ditemukan: $Source" }
New-Item -ItemType Directory -Force $OutDir | Out-Null

$src = [System.Drawing.Image]::FromFile((Resolve-Path $Source))

# Resize dengan latar transparan (logo putih-transparan milik brand).
# Untuk ukuran kecil tray, glyph putih diberi "plate" gelap tipis agar tetap
# terlihat bila taskbar Windows bertema terang.
function New-IconBitmap([int]$size, [bool]$plate) {
    $bmp = New-Object System.Drawing.Bitmap($size, $size)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.Clear([System.Drawing.Color]::Transparent)
    if ($plate) {
        $pad = [math]::Max(1, [int]($size * 0.06))
        $d = $size - 2 * $pad
        $r = [int]($size * 0.28)
        $g.FillEllipse((New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(235, 24, 24, 28))),
            $pad, $pad, $d, $d)
    }
    # Pertahankan rasio asli lalu pusatkan
    $ratio = [math]::Min($size / $src.Width, $size / $src.Height)
    $w = [int]($src.Width * $ratio); $h = [int]($src.Height * $ratio)
    $x = [int](($size - $w) / 2); $y = [int](($size - $h) / 2)
    $g.DrawImage($src, $x, $y, $w, $h)
    $g.Dispose()
    return $bmp
}

function Get-PngBytes($bmp) {
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $bytes = $ms.ToArray()
    $ms.Dispose()
    return , $bytes
}

# Entri DIB klasik (BITMAPINFOHEADER + piksel BGRA bottom-up + AND mask).
# Dipakai untuk ukuran kecil — jauh lebih kompatibel daripada PNG-in-ICO
# (beberapa parser lawas/shell tidak menangani PNG-in-ICO dengan baik).
function Get-DibBytes($bmp) {
    $size = $bmp.Width
    $rect = New-Object System.Drawing.Rectangle(0, 0, $size, $size)
    $data = $bmp.LockBits($rect,
        [System.Drawing.Imaging.ImageLockMode]::ReadOnly,
        [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $stride = $data.Stride
    $raw = New-Object byte[] ($stride * $size)
    [System.Runtime.InteropServices.Marshal]::Copy($data.Scan0, $raw, 0, $raw.Length)
    $bmp.UnlockBits($data)

    $xorLen = $size * $size * 4
    $maskRow = [int]([math]::Ceiling($size / 32.0) * 4)   # baris AND mask 32-bit aligned
    $out = New-Object byte[] (40 + $xorLen + $maskRow * $size)

    # BITMAPINFOHEADER
    [BitConverter]::GetBytes([UInt32]40).CopyTo($out, 0)
    [BitConverter]::GetBytes([Int32]$size).CopyTo($out, 4)
    [BitConverter]::GetBytes([Int32]($size * 2)).CopyTo($out, 8)   # tinggi = XOR + AND
    [BitConverter]::GetBytes([UInt16]1).CopyTo($out, 12)
    [BitConverter]::GetBytes([UInt16]32).CopyTo($out, 14)
    [BitConverter]::GetBytes([UInt32]$xorLen).CopyTo($out, 20)

    # Piksel BGRA, baris bawah ke atas (konvensi DIB)
    for ($y = 0; $y -lt $size; $y++) {
        $so = ($size - 1 - $y) * $stride
        $do = 40 + $y * $size * 4
        for ($x = 0; $x -lt $size; $x++) {
            $i = $so + $x * 4; $j = $do + $x * 4
            $out[$j] = $raw[$i + 2]      # B
            $out[$j + 1] = $raw[$i + 1]  # G
            $out[$j + 2] = $raw[$i]      # R
            $out[$j + 3] = $raw[$i + 3]  # A
        }
    }
    # AND mask dibiarkan nol → transparansi sepenuhnya dari kanal alpha
    return , $out
}

# ── 1) icon.png (256px, transparan) untuk default window/tray icon ──
$png256 = New-IconBitmap 256 $false
[System.IO.File]::WriteAllBytes((Join-Path $OutDir "icon.png"), (Get-PngBytes $png256))
$png256.Dispose()

# ── 2) icon.ico multi-ukuran (16..256) ──
#    ≤128 px → DIB klasik (kompatibel luas), 256 px → PNG (hemat ruang).
$sizes = @(16, 24, 32, 48, 64, 128, 256)
$entries = @()
foreach ($s in $sizes) {
    # plate khusus ukuran tray kecil (<=32px) agar terlihat di taskbar terang
    $bmp = New-IconBitmap $s ($s -le 32)
    $payload = if ($s -ge 256) { Get-PngBytes $bmp } else { Get-DibBytes $bmp }
    $entries += [pscustomobject]@{ Size = $s; Data = $payload }
    $bmp.Dispose()
}

$icoPath = Join-Path $OutDir "icon.ico"
$fs = [System.IO.File]::Create($icoPath)
$bw = New-Object System.IO.BinaryWriter($fs)
$bw.Write([UInt16]0)                 # reserved
$bw.Write([UInt16]1)                 # type: icon
$bw.Write([UInt16]$entries.Count)    # count
$offset = 6 + 16 * $entries.Count
foreach ($e in $entries) {
    $dim = [byte]0                            # 256 direpresentasikan sebagai 0
    if ($e.Size -lt 256) { $dim = [byte]$e.Size }
    $bw.Write($dim)                  # width
    $bw.Write($dim)                  # height
    $bw.Write([byte]0)               # colors
    $bw.Write([byte]0)               # reserved
    $bw.Write([UInt16]1)             # planes
    $bw.Write([UInt16]32)            # bpp
    $bw.Write([UInt32]$e.Data.Length) # bytes in resource
    $bw.Write([UInt32]$offset)       # offset
    $offset += $e.Data.Length
}
foreach ($e in $entries) { $bw.Write($e.Data) }
$bw.Close(); $fs.Dispose()
$src.Dispose()

Write-Host "OK: $icoPath ($([math]::Round((Get-Item $icoPath).Length/1KB,1)) KB, $($entries.Count) ukuran)"
Write-Host "OK: $(Join-Path $OutDir 'icon.png')"
