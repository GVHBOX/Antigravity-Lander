param([string]$SourcePath,[int]$X,[int]$Y,[int]$Width,[int]$Height,[string]$OutPath)

Add-Type -AssemblyName System.Drawing
$src = New-Object System.Drawing.Bitmap $SourcePath
$clip = New-Object System.Drawing.Rectangle $X, $Y, $Width, $Height
# Clone() is a pure pixel crop; Graphics.DrawImage rescales by the source DPI and
# leaves the rest of the canvas white.
$cropped = $src.Clone($clip, $src.PixelFormat)
$cropped.Save($OutPath, [System.Drawing.Imaging.ImageFormat]::Png)
$cropped.Dispose(); $src.Dispose()
Write-Output "cropped ${Width}x${Height} -> $OutPath"
