param([string]$OutDir = (Join-Path $PSScriptRoot '..\docs'))

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public class WinCap {
  [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out Rect r);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int max);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  public delegate bool EnumProc(IntPtr h, IntPtr l);
}
"@

$targetPids = @{}
foreach ($proc in (Get-Process -Name 'Antigravity IDE' -ErrorAction SilentlyContinue)) { $targetPids[[uint32]$proc.Id] = $true }
if ($targetPids.Count -eq 0) { Write-Output 'Antigravity IDE is not running'; exit 1 }

$handles = New-Object System.Collections.Generic.List[IntPtr]
# the delegate must stay referenced for the duration of EnumWindows or it gets collected
$callback = [WinCap+EnumProc] {
  param($h, $l)
  $ownerPid = [uint32]0
  [void][WinCap]::GetWindowThreadProcessId($h, [ref]$ownerPid)
  if ($targetPids.ContainsKey($ownerPid) -and [WinCap]::IsWindowVisible($h) -and [WinCap]::GetWindowTextLength($h) -gt 0) {
    $handles.Add($h) | Out-Null
  }
  return $true
}
[void][WinCap]::EnumWindows($callback, [IntPtr]::Zero)
$callback = $null
$targetPids = $null

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$index = 0
foreach ($hwnd in $handles) {
  $sb = New-Object System.Text.StringBuilder 512
  [void][WinCap]::GetWindowText($hwnd, $sb, $sb.Capacity)
  $title = $sb.ToString()
  if (-not $title) { Write-Output 'skipped window with empty title'; continue }
  if ($title -notmatch 'Antigravity') { Write-Output "skipped unrelated window '$title'"; continue }

  $rect = New-Object WinCap+Rect
  [void][WinCap]::GetWindowRect($hwnd, [ref]$rect)
  $w = $rect.Right - $rect.Left
  $h = $rect.Bottom - $rect.Top
  if ($w -lt 300 -or $h -lt 300) { Write-Output "skipped minimized/hidden window '$title' (${w}x${h}) — restore it first"; continue }

  $bmp = New-Object System.Drawing.Bitmap $w, $h
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $hdc = $g.GetHdc()
  [void][WinCap]::PrintWindow($hwnd, $hdc, 2)
  $g.ReleaseHdc($hdc)
  $g.Dispose()
  $index++
  $file = Join-Path $OutDir ("screenshot-{0:d2}.png" -f $index)
  $bmp.Save($file, [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
  Write-Output "captured ${w}x${h} '$title' -> $file"
}
Write-Output "windows seen: $($handles.Count), captured: $index"
