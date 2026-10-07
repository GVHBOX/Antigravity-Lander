param(
  [string]$ExePath,
  [string]$ProcessName = 'Antigravity IDE'
)

$ErrorActionPreference = 'SilentlyContinue'

function Find-Exe {
  if ($ExePath -and (Test-Path $ExePath)) { return $ExePath }

  # A running instance is the most reliable source of truth for the install path.
  $running = Get-Process -Name $ProcessName | Where-Object { $_.Path } | Select-Object -First 1
  if ($running) { return $running.Path }

  $candidates = @(
    'D:\Antigravity IDE\Antigravity IDE.exe'
    (Join-Path $env:LOCALAPPDATA 'Programs\Antigravity IDE\Antigravity IDE.exe')
    'C:\Program Files\Antigravity IDE\Antigravity IDE.exe'
  )
  foreach ($c in $candidates) { if (Test-Path $c) { return $c } }
  return $null
}

$Exe = Find-Exe
if (-not $Exe) {
  Write-Output 'Antigravity IDE executable not found. Pass -ExePath <path> or set it in scripts/restart-ide.ps1'
  exit 1
}

$before = @(Get-Process -Name $ProcessName)
Write-Output "processes before: $($before.Count)"

foreach ($p in ($before | Where-Object { $_.MainWindowHandle -ne 0 })) {
  Write-Output "closing window: [$($p.Id)] $($p.MainWindowTitle)"
  $p.CloseMainWindow() | Out-Null
}

$deadline = (Get-Date).AddSeconds(25)
while ((Get-Date) -lt $deadline) {
  if (@(Get-Process -Name $ProcessName).Count -eq 0) { break }
  Start-Sleep -Milliseconds 500
}

$alive = @(Get-Process -Name $ProcessName)
if ($alive.Count -gt 0) {
  Write-Output "still alive after graceful close: $($alive.Count) -> forcing"
  Stop-Process -Id ($alive | ForEach-Object { $_.Id }) -Force
  Start-Sleep -Seconds 2
}

Write-Output "processes after shutdown: $(@(Get-Process -Name $ProcessName).Count)"
Start-Process -FilePath $Exe
Write-Output "relaunched: $Exe"
