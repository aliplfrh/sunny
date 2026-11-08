# Membuat icons/icon.ico secara prosedural — logo "matahari" Sunny.
# Dipakai oleh tauri-build untuk resource exe Windows.
param(
    [string]$OutFile = (Join-Path $PSScriptRoot "..\apps\sunny\src-tauri\icons\icon.ico")
)

Add-Type -AssemblyName System.Drawing

$size = 256
$bmp = New-Object System.Drawing.Bitmap($size, $size)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
$g.Clear([System.Drawing.Color]::Transparent)

$cx = $size / 2.0
$cy = $size / 2.0

# 1) Halo (glow) — lingkaran lembut di belakang
$haloRect = New-Object System.Drawing.RectangleF(($cx - 105), ($cy - 105), 210, 210)
$haloPath = New-Object System.Drawing.Drawing2D.GraphicsPath
$haloPath.AddEllipse($haloRect)
$halo = New-Object System.Drawing.Drawing2D.PathGradientBrush($haloPath)
$halo.CenterColor = [System.Drawing.Color]::FromArgb(200, 255, 190, 90)
$halo.CenterPoint = New-Object System.Drawing.PointF($cx, $cy)
$halo.SurroundColors = @([System.Drawing.Color]::FromArgb(0, 255, 190, 90))
$g.FillPath($halo, $haloPath)
$haloPath.Dispose(); $halo.Dispose()

# 2) Delapan sinar (rays)
$rayBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 245, 160, 40))
for ($i = 0; $i -lt 8; $i++) {
    $angle = $i * [math]::PI / 4.0
    $g.ResetTransform()
    $g.TranslateTransform($cx, $cy)
    $g.RotateTransform(($i * 45))
    # sinar: kapsul vertikal di atas lingkaran pusat
    $rayW = 18; $rayH = 52
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $path.AddArc(-($rayW / 2), -112, $rayW, $rayW, 180, 180)
    $path.AddArc(-($rayW / 2), -112 + $rayH - $rayW, $rayW, $rayW, 0, 180)
    $path.CloseFigure()
    $g.FillPath($rayBrush, $path)
    $path.Dispose()
}
$g.ResetTransform()
$rayBrush.Dispose()

# 3) Buahnya — lingkaran matahari dengan gradasi vertikal
$ballRect = New-Object System.Drawing.RectangleF(($cx - 62), ($cy - 62), 124, 124)
$ballPath = New-Object System.Drawing.Drawing2D.GraphicsPath
$ballPath.AddEllipse($ballRect)
$ball = New-Object System.Drawing.Drawing2D.PathGradientBrush($ballPath)
$ball.CenterColor = [System.Drawing.Color]::FromArgb(255, 255, 214, 96)
$ball.CenterPoint = New-Object System.Drawing.PointF($cx, ($cy - 18))
$ball.SurroundColors = @([System.Drawing.Color]::FromArgb(255, 233, 120, 20))
$g.FillPath($ball, $ballPath)
$ballPath.Dispose(); $ball.Dispose()

$g.Dispose()

# Simpan PNG ke memori
$ms = New-Object System.IO.MemoryStream
$bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
$png = $ms.ToArray()
$bmp.Dispose()
$ms.Dispose()

# Bungkus PNG dalam ICO (format PNG-in-ICO, sah sejak Windows Vista)
New-Item -ItemType Directory -Force (Split-Path $OutFile) | Out-Null
$fs = [System.IO.File]::Create($OutFile)
$bw = New-Object System.IO.BinaryWriter($fs)
$bw.Write([UInt16]0)      # reserved
$bw.Write([UInt16]1)      # type: icon
$bw.Write([UInt16]1)      # count
$bw.Write([byte]0)        # width  (256 -> 0)
$bw.Write([byte]0)        # height (256 -> 0)
$bw.Write([byte]0)        # colors
$bw.Write([byte]0)        # reserved
$bw.Write([UInt16]1)      # planes
$bw.Write([UInt16]32)     # bpp
$bw.Write([UInt32]$png.Length)
$bw.Write([UInt32]22)     # offset (6 + 16)
$bw.Write($png)
$bw.Close(); $fs.Dispose()

Write-Host "OK: $OutFile ($((Get-Item $OutFile).Length) bytes)"
