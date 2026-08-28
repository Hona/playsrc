# SSH is the transport. Task Scheduler only bridges SSH's noninteractive
# session to the existing user's physical console; it runs the normal profiler.
param(
  [ValidateSet('Run','Status','Logs')][string]$Action = 'Run',
  [Parameter(Mandatory=$true)][string]$Job,
  [string]$Profile,
  [switch]$Ready
)
$ErrorActionPreference = 'Stop'
if ($Job -notmatch '^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$') { throw 'Invalid job ID' }
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$bun = (Get-Command bun -CommandType Application).Source
$config = Get-Content -Raw (Join-Path $root 'playsrc.local.json') | ConvertFrom-Json
$directory = Join-Path $config.sourceCacheDir "local-jobs/$Job"
if (!(Test-Path -LiteralPath (Join-Path $directory 'job.json'))) { throw 'Prepare this job first' }
if ($Action -ne 'Run') {
  $latest = Get-ChildItem -Path "$directory/*/command.log" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
  $result = if ($latest -and (Test-Path (Join-Path $latest.DirectoryName 'result.json'))) { Get-Content -Raw (Join-Path $latest.DirectoryName 'result.json') | ConvertFrom-Json } else { $null }
  if ($Action -eq 'Logs') {
    if ($latest) { Get-Content -LiteralPath $latest.FullName -Tail 80 }
    if ($latest) {
      $build = Select-String -LiteralPath $latest.FullName -Pattern '^\[performance\] development build log=(.+)$' | Select-Object -Last 1
      if ($build) { Get-Content -LiteralPath $build.Matches[0].Groups[1].Value -Tail 30 }
    }
  } else {
    $processes = @(Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -and ($_.CommandLine.Replace('/','\').Contains($directory.Replace('/','\')) -or ($_.Name -eq 'bun.exe' -and $_.CommandLine -like "*local-job.ts run $Job*")) } | Select-Object ProcessId,ParentProcessId,Name)
    @{job=$Job;running=(Test-Path (Join-Path $directory 'running'));processes=$processes;log=$latest.FullName;result=$result} | ConvertTo-Json -Depth 8 -Compress
  }
  exit 0
}
if (!$Ready) { throw 'Pass -Ready only for a freshly approved hands-off window' }
if ($Profile -notmatch '^[a-z0-9-]+$') { throw 'Expected a normal profile name' }
$token = [Guid]::NewGuid().ToString()
$name = "playsrc-local-job-$token"
$log = Join-Path $directory "$token-launch.log"
function Quote([string]$value) { return "'" + $value.Replace("'", "''") + "'" }
$command = "`$ErrorActionPreference='Stop'; try { Set-Location $(Quote $root); & $(Quote $bun) tools/playsrc/src/local-job.ts run $(Quote $Job) --ready profile $(Quote $Profile) *> $(Quote $log); exit `$LASTEXITCODE } finally { Unregister-ScheduledTask -TaskName $(Quote $name) -Confirm:`$false }"
$encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command))
$taskAction = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument "-NoProfile -NonInteractive -WindowStyle Hidden -EncodedCommand $encoded" -WorkingDirectory $root
$principal = New-ScheduledTaskPrincipal -UserId ([Security.Principal.WindowsIdentity]::GetCurrent().Name) -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit (New-TimeSpan -Minutes 3) -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
try {
  Register-ScheduledTask -TaskName $name -Action $taskAction -Principal $principal -Settings $settings | Out-Null
  Start-ScheduledTask -TaskName $name
} catch {
  Unregister-ScheduledTask -TaskName $name -Confirm:$false -ErrorAction SilentlyContinue
  throw
}
@{task=$name;job=$Job;profile=$Profile;log=$log;results=$directory} | ConvertTo-Json -Compress
