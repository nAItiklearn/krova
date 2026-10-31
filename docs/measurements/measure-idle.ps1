# Samples Krova's memory and CPU (one sample every -IntervalSec seconds) and prints one line per sample.
#
# Usage (Krova must be running):
#   powershell -NoProfile -ExecutionPolicy Bypass -File measure-idle.ps1
#   powershell -NoProfile -ExecutionPolicy Bypass -File measure-idle.ps1 -ExePath C:\path\to\krova-app.exe -Identifier dev.krova.test -Samples 5 -Label window-closed
#
# Two copies of krova-app.exe can run side by side (an installed one and a test build). Pick yours with
#   -ExePath     the full path of the krova-app.exe to measure (default: the first krova-app.exe found)
#   -Identifier  the bundle identifier of that build; its WebView2 processes carry it in their command line
#                (default dev.krova.desktop; a test build made with TAURI_CONFIG='{"identifier":"dev.krova.test"}'
#                uses dev.krova.test)
#
# Output columns for S rows: S | seconds | krova-app working set MB | all processes working set MB
#   (double-counts pages shared between processes) | all processes private MB | CPU % of ONE core (all processes)
# "All processes" = krova-app.exe + the msedgewebview2.exe processes whose command line names the identifier.
# The result depends on the state you leave Krova in (window open, closed, a drag in progress); note it with -Label.
param(
  [string]$ExePath = '',
  [string]$Identifier = 'dev.krova.desktop',
  [int]$Samples = 30,
  [int]$IntervalSec = 2,
  [string]$Label = ''
)
$ErrorActionPreference = 'Stop'

$candidates = @(Get-Process -Name 'krova-app' -ErrorAction SilentlyContinue)
if ($ExePath -ne '') {
  $want = [System.IO.Path]::GetFullPath($ExePath)
  $candidates = @($candidates | Where-Object { $_.Path -and ([string]::Equals($_.Path, $want, [System.StringComparison]::OrdinalIgnoreCase)) })
}
$app = $candidates | Select-Object -First 1
if (-not $app) { Write-Output 'NOT_RUNNING'; exit 1 }
$appId = $app.Id
$pattern = [regex]::Escape($Identifier)

function Get-KrovaProcs {
  $ids = @($appId)
  Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | ForEach-Object {
    if ($_.CommandLine -match $pattern) { $ids += $_.ProcessId }
  }
  $ids | ForEach-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue }
}

$cores = (Get-CimInstance Win32_ComputerSystem).NumberOfLogicalProcessors
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
$os = Get-CimInstance Win32_OperatingSystem
Write-Output ("HOST|{0}|{1} logical cores|{2:N1} GB RAM|{3}" -f $cpu.Name.Trim(), $cores, ($os.TotalVisibleMemorySize/1MB), $os.Caption)
if ($Label -ne '') { Write-Output ("LABEL|{0}" -f $Label) }
Write-Output ("TARGET|pid {0}|{1}|identifier {2}" -f $appId, $app.Path, $Identifier)
Write-Output ("COUNT|app+webview2 processes: {0}" -f @(Get-KrovaProcs).Count)

$prev = @{}
foreach ($p in Get-KrovaProcs) { $prev[$p.Id] = $p.TotalProcessorTime.TotalSeconds }
$t0 = Get-Date
for ($i = 1; $i -le $Samples; $i++) {
  Start-Sleep -Seconds $IntervalSec
  $procs = @(Get-KrovaProcs)
  $now = Get-Date
  $dt = ($now - $t0).TotalSeconds
  $t0 = $now
  $appP = $procs | Where-Object { $_.Id -eq $appId }
  $ws = ($procs | Measure-Object -Property WorkingSet64 -Sum).Sum
  $pm = ($procs | Measure-Object -Property PrivateMemorySize64 -Sum).Sum
  $cpuDelta = 0.0
  foreach ($p in $procs) {
    $cur = $p.TotalProcessorTime.TotalSeconds
    if ($prev.ContainsKey($p.Id)) { $cpuDelta += ($cur - $prev[$p.Id]) }
    $prev[$p.Id] = $cur
  }
  $cpuPctOfOneCore = 100.0 * $cpuDelta / $dt
  Write-Output ("S|{0}|{1:N1}|{2:N1}|{3:N1}|{4:N2}|procs {5}" -f ($i*$IntervalSec), ($appP.WorkingSet64/1MB), ($ws/1MB), ($pm/1MB), $cpuPctOfOneCore, $procs.Count)
}
Write-Output 'DONE'
