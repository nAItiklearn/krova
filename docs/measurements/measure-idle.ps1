# Samples Krova's memory and CPU for 60 s (one sample every 2 s) and prints one line per sample.
# Usage (Krova must be running): powershell -NoProfile -ExecutionPolicy Bypass -File measure-idle.ps1
# Output columns for S rows: S | seconds | krova-app working set MB | all processes working set MB
#   (double-counts pages shared between processes) | all processes private MB | CPU % of ONE core (all processes)
# "All processes" = krova-app.exe + the msedgewebview2.exe processes whose command line names dev.krova.desktop.
# The result depends on the state you leave Krova in (window open or hidden, a drag in progress); note it with the data.
$ErrorActionPreference = 'Stop'
$app = Get-Process -Name 'krova-app' -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $app) { Write-Output 'NOT_RUNNING'; exit 1 }

function Get-KrovaProcs {
  $ids = @($app.Id)
  Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | ForEach-Object {
    if ($_.CommandLine -match 'dev\.krova\.desktop') { $ids += $_.ProcessId }
  }
  $ids | ForEach-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue }
}

$cores = (Get-CimInstance Win32_ComputerSystem).NumberOfLogicalProcessors
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
$os = Get-CimInstance Win32_OperatingSystem
Write-Output ("HOST|{0}|{1} logical cores|{2:N1} GB RAM|{3}" -f $cpu.Name.Trim(), $cores, ($os.TotalVisibleMemorySize/1MB), $os.Caption)
Write-Output ("COUNT|app+webview2 processes: {0}" -f @(Get-KrovaProcs).Count)

$prev = @{}
foreach ($p in Get-KrovaProcs) { $prev[$p.Id] = $p.TotalProcessorTime.TotalSeconds }
$t0 = Get-Date
$samples = 30
for ($i = 1; $i -le $samples; $i++) {
  Start-Sleep -Seconds 2
  $procs = @(Get-KrovaProcs)
  $now = Get-Date
  $dt = ($now - $t0).TotalSeconds
  $t0 = $now
  $appP = $procs | Where-Object { $_.Id -eq $app.Id }
  $ws = ($procs | Measure-Object -Property WorkingSet64 -Sum).Sum
  $pm = ($procs | Measure-Object -Property PrivateMemorySize64 -Sum).Sum
  $cpuDelta = 0.0
  foreach ($p in $procs) {
    $cur = $p.TotalProcessorTime.TotalSeconds
    if ($prev.ContainsKey($p.Id)) { $cpuDelta += ($cur - $prev[$p.Id]) }
    $prev[$p.Id] = $cur
  }
  $cpuPctOfOneCore = 100.0 * $cpuDelta / $dt
  Write-Output ("S|{0}|{1:N1}|{2:N1}|{3:N1}|{4:N2}" -f ($i*2), ($appP.WorkingSet64/1MB), ($ws/1MB), ($pm/1MB), $cpuPctOfOneCore)
}
Write-Output 'DONE'
