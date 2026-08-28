# SSH is the transport. Task Scheduler only bridges SSH's noninteractive
# session to the existing user's physical console; it runs the normal profiler.
param(
  [ValidateSet('Run','Build','BuildStage','Status','Result','Logs','Doctor','Wait','Artifacts','Recover')][string]$Action = 'Run',
  [Parameter(Mandatory=$true)][string]$Job,
  [string]$Profile,
  [string]$Grep = '',
  [switch]$FreshBrowser,
  [string]$ProfileArguments = '[]',
  [string]$Target,
  [ValidateSet('wasm','producer','resources')][string]$Stage,
  [string]$Task,
  [switch]$IncludeTrace,
  [switch]$Ready
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
if ($Job -notmatch '^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$') { throw 'Invalid job ID' }
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$bun = (Get-Command bun -CommandType Application).Source
$config = Get-Content -Raw (Join-Path $root 'playsrc.local.json') | ConvertFrom-Json
$directory = Join-Path $config.sourceCacheDir "local-jobs/$Job"
if (!(Test-Path -LiteralPath (Join-Path $directory 'job.json'))) { throw 'Prepare this job first' }
if ($Action -eq 'Wait') {
  $runner = Get-CimInstance Win32_Process | Where-Object { $_.Name -eq 'bun.exe' -and $_.CommandLine -like '*local-job.ts*' -and $_.CommandLine.Contains($Job) }
  foreach ($process in $runner) { Wait-Process -Id $process.ProcessId -Timeout 175 -ErrorAction SilentlyContinue }
  $Action = 'Status'
}
if ($Action -eq 'Doctor') {
  $cargo = Join-Path $config.sourceCacheDir 'toolchains/rust/cargo/bin/cargo.exe'
  $env:CARGO_HOME = Join-Path $config.sourceCacheDir 'toolchains/rust/cargo'
  $env:RUSTUP_HOME = Join-Path $config.sourceCacheDir 'toolchains/rust/rustup'
  $item = Get-Item -LiteralPath $cargo
  @{cargo=$cargo;length=$item.Length;linkType=$item.LinkType;target=$item.Target;pathExt=$env:PATHEXT} | ConvertTo-Json -Compress
  try { [IO.File]::ReadAllBytes($cargo)[0..15] | ConvertTo-Json -Compress } catch { Write-Output $_.Exception.Message }
  try { & $cargo --version } catch { Write-Output $_.Exception.Message }
  $probe = 'const p=process.argv[1]; const fs=require("node:fs"), cp=require("node:child_process"); console.log(JSON.stringify({pathKeys:Object.keys(process.env).filter(k=>k.toLowerCase()==="path"),PATHEXT:process.env.PATHEXT,stat:fs.statSync(p).isFile(),which:Bun.which(p)})); try {console.log(cp.execFileSync(p,["--version"],{encoding:"utf8"}))} catch(e){console.log(String(e))}; try {console.log(Bun.spawnSync([p,"--version"]).stdout.toString())} catch(e){console.log(String(e))}'
  & $bun -e $probe $cargo
  exit $LASTEXITCODE
}
if ($Action -ne 'Run' -and $Action -ne 'Build' -and $Action -ne 'BuildStage') {
  $tasks = if ($Action -in 'Status','Recover') { @(Get-ScheduledTask -TaskName 'playsrc-local-job-*' -ErrorAction SilentlyContinue) } else { @() }
  $taskState = $null
  $launchFile = $null
  $launchText = $null
  if ($Task) {
    if ($Task -notmatch '^playsrc-local-job-([a-f0-9-]{36})$' -or !(Test-Path (Join-Path $directory "$($Matches[1])-launch.log"))) { throw 'Task is not recorded for this job' }
    $launchFile = Join-Path $directory "$($Matches[1])-launch.log"
    $launchText = Get-Content -Raw -LiteralPath $launchFile
    $selectedTask = $tasks | Where-Object TaskName -eq $Task
    $taskState = $selectedTask | Select-Object TaskName,State
    $taskInfo = if ($selectedTask) { Get-ScheduledTaskInfo -TaskName $Task | Select-Object LastRunTime,LastTaskResult } else { $null }
  }
  $latestRun = Get-ChildItem -LiteralPath $directory -Directory | Where-Object { $_.Name -match '^[a-f0-9-]{36}$' } | Sort-Object CreationTime -Descending | Select-Object -First 1
  if ($launchFile -and $latestRun -and $latestRun.CreationTimeUtc -lt (Get-Item -LiteralPath $launchFile).CreationTimeUtc) { $latestRun = $null }
  $latest = if ($latestRun) { Get-Item (Join-Path $latestRun.FullName 'command.log') -ErrorAction SilentlyContinue } else { $null }
  $result = $null
  if ($Task) {
    # A rejected launch may never create a command.log. Never report the prior
    # build/test's success as this task's outcome.
    $readback = & $bun (Join-Path $root 'tools/playsrc/src/local-job.ts') result $Job $Task
    if ($LASTEXITCODE) { throw 'Cannot read the recorded task result' }
    $result = ($readback | ConvertFrom-Json).result
  } elseif ($latestRun -and (Test-Path (Join-Path $latestRun.FullName 'result.json'))) {
    $result = Get-Content -Raw (Join-Path $latestRun.FullName 'result.json') | ConvertFrom-Json
  }
  if ($Action -eq 'Result') {
    @{result=$result;launchError=$(if (!$result) { $launchText } else { $null })} | ConvertTo-Json -Depth 8 -Compress
    exit 0
  }
  if ($Action -eq 'Artifacts') {
    if (!$result -or $result.schema -ne 'playsrc-local-job-result-v1') { throw 'This task has no completed result to collect' }
    $files = @(@{name='job/result.json';path=(Join-Path $result.run 'result.json')})
    if ($Task) {
      $policyFile = Join-Path $directory "$($Task.Substring('playsrc-local-job-'.Length))-policy.json"
      if (Test-Path -LiteralPath $policyFile) { $files += @{name='job/launch-policy.json';path=$policyFile} }
    }
    $commandLog = Join-Path $result.run 'command.log'
    if (Test-Path $commandLog) {
      $files += @{name='job/command.log';path=$commandLog}
      $report = Select-String -LiteralPath $commandLog -Pattern 'report=(.+command\.json)$' | Select-Object -Last 1
      if ($report) {
        $profileDirectory = Split-Path $report.Matches[0].Groups[1].Value
        if (![IO.Path]::GetFullPath($profileDirectory).StartsWith([IO.Path]::GetFullPath($config.sourceCacheDir), [StringComparison]::OrdinalIgnoreCase)) { throw 'Profiler report is outside the configured cache' }
        foreach ($artifact in Get-ChildItem -LiteralPath $profileDirectory -File | Where-Object { $_.Extension -in '.json','.png','.cpuprofile' -and $_.Length -le 16MB } | Sort-Object @{Expression={if ($_.Extension -eq '.png') {1} else {0}}},Name | Select-Object -First 64) {
          $files += @{name="profile/$($artifact.Name)";path=$artifact.FullName}
        }
        if ($IncludeTrace) {
          $reportData = Get-Content -Raw (Join-Path $profileDirectory 'latest.json') | ConvertFrom-Json
          $manifestName = $reportData.compositorEvidence.file
          if ($manifestName -notmatch '^[a-f0-9]{64}\.manifest\.json$') { throw 'Invalid retained trace manifest' }
          $kind = if ($reportData.entry -eq 'create-server') { '2fort-startup' } else { 'upward-training-bots' }
          $evidence = Join-Path $config.sourceCacheDir "profiles/$kind/compositor-evidence"
          $manifestPath = Join-Path $evidence $manifestName
          $manifest = Get-Content -Raw $manifestPath | ConvertFrom-Json
          $files += @{name="evidence/$manifestName";path=$manifestPath}
          foreach ($blob in @($manifest.trace,$manifest.probes)) {
            if ($blob.file -notmatch '^[a-f0-9]{64}\.(trace|probes)\.json\.gz$' -or $blob.bytes -gt 64MB) { throw 'Invalid retained trace blob' }
            $files += @{name="evidence/$($blob.file)";path=(Join-Path $evidence $blob.file)}
          }
        }
      }
    }
    @{run=(Split-Path $result.run -Leaf);files=$files} | ConvertTo-Json -Depth 4 -Compress
    exit 0
  }
  if ($Action -eq 'Logs') {
    if ($launchText) { Write-Output $launchText }
    if ($latest) { Get-Content -LiteralPath $latest.FullName -Tail 80 }
    if ($latest) {
      $build = Select-String -LiteralPath $latest.FullName -Pattern '^\[performance\] development build log=(.+)$' | Select-Object -Last 1
      if ($build) { Get-Content -LiteralPath $build.Matches[0].Groups[1].Value -Tail 30 }
    }
  } else {
    $processes = @(Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -and ($_.CommandLine.Replace('/','\').Contains($directory.Replace('/','\')) -or ($_.Name -eq 'bun.exe' -and $_.CommandLine -like '*local-job.ts*' -and $_.CommandLine.Contains($Job))) } | Select-Object ProcessId,ParentProcessId,Name)
    $running = Test-Path (Join-Path $directory 'running')
    if ($Action -eq 'Recover') {
      if (!$Task -or !$selectedTask -or $selectedTask.State -ne 'Ready' -or !$taskInfo.LastTaskResult -or $processes.Count -or !$running -or $result) { throw 'Recovery requires this recorded failed task, no live job processes, and no completed result' }
      $record = @{task=$Task;taskInfo=$taskInfo;at=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds();runningRecord=(Get-Content -Raw (Join-Path $directory 'running'));outcome='interrupted-before-result'}
      $record | ConvertTo-Json -Depth 6 | Set-Content -Encoding UTF8 (Join-Path $directory "$Task-recovery.json")
      Remove-Item -LiteralPath (Join-Path $directory 'running')
      $running = $false
    }
    # Retire from the launching account, not the deliberately unelevated task.
    # Never remove a queued/running task or another job's recorded task name.
    if (!$running -and !$processes.Count) {
      foreach ($launch in Get-ChildItem -Path "$directory/*-launch.log") {
        if ($launch.BaseName -match '^([a-f0-9-]{36})-launch$') {
          $completed = $tasks | Where-Object TaskName -eq "playsrc-local-job-$($Matches[1])"
          if ($completed -and $completed.State -eq 'Ready') { Unregister-ScheduledTask -TaskName $completed.TaskName -Confirm:$false }
        }
      }
    }
    $summary = if ($result) { $result | Select-Object commit,outcome,startedAt,finishedAt,run } else { $null }
    $marker = if ($running) { @{file=(Get-Item (Join-Path $directory 'running') | Select-Object CreationTimeUtc,Length);content=(Get-Content -Raw (Join-Path $directory 'running'))} } else { $null }
    @{job=$Job;task=$taskState;taskInfo=$taskInfo;running=$running;marker=$marker;latestRun=$latestRun.FullName;processes=$processes;log=$latest.FullName;launchLog=$launchFile;launchError=$(if (!$result) { $launchText } else { $null });result=$summary} | ConvertTo-Json -Depth 8 -Compress
  }
  exit 0
}
if ($Action -eq 'Run') {
  if (!$Ready) { throw 'Pass -Ready only for a freshly approved hands-off window' }
  if ($Profile -notmatch '^[a-z0-9-]+$') { throw 'Expected a normal profile name' }
  if ($Grep.Length -gt 512 -or $Grep.Contains([char]0)) {throw 'Profile selection exceeds its bound'}
} elseif ($Action -eq 'BuildStage') {
  if (!$Stage -or ($Stage -eq 'resources' -and $Target -notmatch '^[a-z0-9_]+$') -or ($Stage -ne 'resources' -and $Target)) { throw 'Expected wasm | producer | resources with a local build target' }
} elseif ($Target -notmatch '^[a-z0-9_]+$') { throw 'Expected a local build target' }
$token = [Guid]::NewGuid().ToString()
$name = "playsrc-local-job-$token"
$log = Join-Path $directory "$token-launch.log"
$policy = Join-Path $directory "$token-policy.json"
function Quote([string]$value) { return "'" + $value.Replace("'", "''") + "'" }
$extra = ConvertFrom-Json -InputObject $ProfileArguments
if ($ProfileArguments.Trim() -notmatch '^\[.*\]$' -or $extra.Count -gt 16 -or ($null -eq $extra -and $ProfileArguments -notmatch '^\s*\[\s*\]\s*$')) { throw 'Invalid profiler argument array' }
foreach ($value in $extra) { if ($value -isnot [string]) { throw 'Invalid profiler argument array' } }
if ($null -eq $extra) { $extra = @() } elseif ($extra -isnot [array]) { $extra = @($extra) }
if ($Grep) { $extra += @('--grep', $Grep) }
if ($FreshBrowser) { $extra += '--fresh-browser' }
if ($extra.Count -gt 16) { throw 'Invalid profiler argument array' }
$arguments = if ($Action -eq 'Build') { "build $(Quote $Target)" } elseif ($Action -eq 'BuildStage') { "build-stage $(Quote $Stage)" + $(if ($Stage -eq 'resources') { " $(Quote $Target)" } else { '' }) } else { "--ready profile $(Quote $Profile) " + (($extra | ForEach-Object { Quote $_ }) -join ' ') }
$command = "`$ErrorActionPreference='Stop'; Set-Location $(Quote $root); @{taskPriority=5;processPriority=[string][Diagnostics.Process]::GetCurrentProcess().PriorityClass;pid=`$PID} | ConvertTo-Json -Compress | Set-Content -Encoding UTF8 $(Quote $policy); & $(Quote $bun) tools/playsrc/src/local-job.ts run $(Quote $Job) $arguments *> $(Quote $log); exit `$LASTEXITCODE"
$encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command))
$taskAction = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument "-NoProfile -NonInteractive -WindowStyle Hidden -EncodedCommand $encoded" -WorkingDirectory $root
$principal = New-ScheduledTaskPrincipal -UserId ([Security.Principal.WindowsIdentity]::GetCurrent().Name) -LogonType Interactive -RunLevel Limited
# Task Scheduler defaults to 7 (BELOW_NORMAL/background), unlike an ordinary
# interactive launch. 5 is NORMAL, not an above-normal/realtime benchmark boost.
# https://learn.microsoft.com/windows/win32/taskschd/tasksettings-priority
$settings = New-ScheduledTaskSettingsSet -Priority 5 -ExecutionTimeLimit (New-TimeSpan -Minutes 3) -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
try {
  Register-ScheduledTask -TaskName $name -Action $taskAction -Principal $principal -Settings $settings | Out-Null
  Start-ScheduledTask -TaskName $name
} catch {
  Unregister-ScheduledTask -TaskName $name -Confirm:$false -ErrorAction SilentlyContinue
  throw
}
@{task=$name;job=$Job;action=$Action;profile=$Profile;target=$Target;log=$log;results=$directory} | ConvertTo-Json -Compress
