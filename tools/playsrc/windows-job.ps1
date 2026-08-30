# SSH is the transport. Only headed profiles bridge to the physical console;
# background tasks use the same bounded scheduler ownership without a desktop.
param(
  [ValidateSet('Run','Build','BuildStage','Test','Diagnostic','Cancel','Status','Result','Logs','Doctor','Wait','Artifacts','Recover')][string]$Action = 'Run',
  [Parameter(Mandatory=$true)][string]$Job,
  [string]$Profile,
  [string]$Grep = '',
  [switch]$FreshBrowser,
  [string]$ProfileArguments = '[]',
  [string]$Target,
  [ValidateSet('wasm','producer','resources')][string]$Stage,
  [string]$Task,
  [switch]$IncludeTrace,
  [string]$JobArguments = '[]',
  [string]$TestArguments = '[]',
  [ValidateRange(0,30000)][int]$Milliseconds = 250,
  [ValidateRange(0,1)][int]$DiagnosticExit = 0
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
. (Join-Path $PSScriptRoot 'windows-readback.ps1')
if ($Job -notmatch '^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$') { throw 'Invalid job ID' }
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$bun = (Get-Command bun -CommandType Application).Source
$config = Get-Content -Raw (Join-Path $root 'playsrc.local.json') | ConvertFrom-Json
$directory = Join-Path $config.sourceCacheDir "local-jobs/$Job"
if (!(Test-Path -LiteralPath (Join-Path $directory 'job.json'))) { throw 'Prepare this job first' }
if($Action -in 'Wait','Status','Result','Logs','Artifacts','Recover','Cancel') {
 $invocation=[Environment]::CommandLine
 if($MyInvocation.InvocationName -eq '.' -or $invocation -notmatch '(?i)\s-File\s' -or $invocation -notmatch '(?i)\s-NonInteractive(?:\s|$)' -or $invocation -match '(?i)\s-NoExit(?:\s|$)'){throw 'Readback requires its own noninteractive -File helper'}
 $heldFile=Join-Path $config.sourceCacheDir 'evidence/tf2-browser-performance/chromium-profile.lock'
 if((Test-Path -LiteralPath $heldFile) -and (Get-Content -Raw -LiteralPath $heldFile|ConvertFrom-Json).pid -eq $PID){throw 'Readback helper must not own the profiler lock'}
 Add-Type -Path (Join-Path $PSScriptRoot 'windows-readback-guard.cs')
 [PlaysrcReadbackGuard]::Start($(if($Action -eq 'Wait'){175000}else{15000}),536870912,(Join-Path $directory "readback-$PID-fault.json"))
}
function OwnedRunner([string]$ownerFile) {
 if(!(Test-Path -LiteralPath $ownerFile)){return $null}
 $identity=Get-Content -Raw -LiteralPath $ownerFile|ConvertFrom-Json
 if(!$identity.pid){return $null}
 try {$process=[System.Diagnostics.Process]::GetProcessById([int]$identity.pid)}catch [ArgumentException]{return $null}
 try {$null=$process.Handle;$created=([DateTimeOffset]$process.StartTime.ToUniversalTime()).ToUnixTimeMilliseconds()}catch [InvalidOperationException]{return $null}
 if(!$identity.startedEpoch -or $created -ne $identity.startedEpoch){throw 'Owned launcher process identity changed; refusing a PID-only wait'}
 return $process
}
if ($Action -eq 'Wait') {
  [PlaysrcReadbackGuard]::Stage='owned-process-wait'
  if($Task -notmatch '^playsrc-local-job-([a-f0-9-]{36})$'){throw 'Wait requires this job current recorded task'}
  $launch=Join-Path $directory "$($Matches[1])-launch.log"
  if(!(Test-Path -LiteralPath $launch)){throw 'Task is not recorded for this job'}
  $deadline=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()+175000
  $ownerFile=[IO.Path]::ChangeExtension($launch,'owner.json')
  while(!(Test-Path -LiteralPath $ownerFile) -and (Get-Item -LiteralPath $launch).Length -eq 0 -and [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() -lt $deadline){Start-Sleep -Milliseconds 100}
  $runner=OwnedRunner $ownerFile
  if($runner -and !$runner.WaitForExit([int][Math]::Max(1,$deadline-[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()))){throw 'Owned job wait exceeded175seconds'}
  if($runner){$runner.Dispose()}
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
if ($Action -notin 'Run','Build','BuildStage','Test','Diagnostic') {
  [PlaysrcReadbackGuard]::Stage='exact-task-query'
  $tasks = if (($Action -in 'Status','Recover') -and $Task) { @(Get-ScheduledTask -TaskName $Task -ErrorAction SilentlyContinue) } else { @() }
  $taskState = $null
  $launchFile = $null
  $launchText = $null
  if ($Task) {
    if ($Task -notmatch '^playsrc-local-job-([a-f0-9-]{36})$' -or !(Test-Path (Join-Path $directory "$($Matches[1])-launch.log"))) { throw 'Task is not recorded for this job' }
    $launchFile = Join-Path $directory "$($Matches[1])-launch.log"
    $runLink=Join-Path $directory "$($Matches[1])-run.json"
    [PlaysrcReadbackGuard]::Stage='launch-log-read'
    $launchText = Read-PlainJobText $launchFile
    $selectedTask = $tasks | Where-Object TaskName -eq $Task
    $taskState = $selectedTask | Select-Object TaskName,State
    $taskInfo = if ($selectedTask) { Get-ScheduledTaskInfo -TaskName $Task | Select-Object LastRunTime,LastTaskResult } else { $null }
  }
  $latestRun = if($Task) {
    if(Test-Path -LiteralPath $runLink) {
      $identity=Get-Content -Raw -LiteralPath $runLink|ConvertFrom-Json
      if($identity.task -ne $Task -or $identity.job -ne $Job -or (Split-Path $identity.run) -ne $directory){throw 'Task run identity differs'}
      Get-Item -LiteralPath $identity.run
    }
  } else { Get-ChildItem -LiteralPath $directory -Directory | Where-Object { $_.Name -match '^[a-f0-9-]{36}$' } | Sort-Object CreationTime -Descending | Select-Object -First 1 }
  if ($launchFile -and $latestRun -and $latestRun.CreationTimeUtc -lt (Get-Item -LiteralPath $launchFile).CreationTimeUtc) { $latestRun = $null }
  $latest = if ($latestRun) { Get-Item (Join-Path $latestRun.FullName 'command.log') -ErrorAction SilentlyContinue } else { $null }
  $result = $null
  if ($Task) {
    # A rejected launch may never create a command.log. Never report the prior
    # build/test's success as this task's outcome.
    [PlaysrcReadbackGuard]::Stage='result-readback'
    $readback = & $bun (Join-Path $root 'tools/playsrc/src/local-job.ts') result $Job $Task
    if ($LASTEXITCODE) { throw 'Cannot read the recorded task result' }
    [PlaysrcReadbackGuard]::Stage='result-decode'
    $result = ($readback | ConvertFrom-Json).result
  } elseif ($latestRun -and (Test-Path (Join-Path $latestRun.FullName 'result.json'))) {
    $result = Get-Content -Raw (Join-Path $latestRun.FullName 'result.json') | ConvertFrom-Json
  }
  if ($Action -eq 'Result') {
    $bootstrap=if($launchFile -and (Test-Path -LiteralPath "$launchFile.bootstrap.log")){Read-PlainJobText "$launchFile.bootstrap.log"}else{$null}
    @{result=$result;launchError=$(if (!$result) { "$launchText$bootstrap" } else { $null })} | ConvertTo-Json -Depth 8 -Compress
    exit 0
  }
  if($Action -eq 'Cancel') {
    if(!$Task -or $result){throw 'Cancel requires this exact unfinished task'}
    $cancelFile=if($latestRun){Join-Path $latestRun.FullName 'cancel'}else{Join-Path $directory "$($Task.Substring('playsrc-local-job-'.Length))-cancel"}
    [IO.File]::WriteAllText($cancelFile,"Cancellation requested for $Task")
    @{task=$Task;run=$latestRun.FullName;cancellationRequested=$true}|ConvertTo-Json -Compress
    exit 0
  }
  if ($Action -eq 'Artifacts') {
    [PlaysrcReadbackGuard]::Stage='artifact-enumeration'
    if (!$result -or $result.schema -ne 'playsrc-local-job-result-v1') { throw 'This task has no completed result to collect' }
    $files = @(@{name='job/result.json';path=(Join-Path $result.run 'result.json')})
    foreach($record in 'identity.json','ownership.json','consent.json','consent-displayed.json','completion-displayed.json','dispatch.json','native-request.json','native-helper.json','native-result.json','native-fault.json') {
      $file=Join-Path $result.run $record
      if(Test-Path -LiteralPath $file){$files+=@{name="job/$record";path=$file}}
    }
    if ($Task) {
      $policyFile = Join-Path $directory "$($Task.Substring('playsrc-local-job-'.Length))-policy.json"
      if (Test-Path -LiteralPath $policyFile) { $files += @{name='job/launch-policy.json';path=$policyFile} }
    }
    if($launchFile){$ownerFile=[IO.Path]::ChangeExtension($launchFile,'owner.json');if(Test-Path -LiteralPath $ownerFile){$files+=@{name='job/launch-owner.json';path=$ownerFile}}}
    if($launchFile -and (Test-Path -LiteralPath "$launchFile.bootstrap.log")){$files+=@{name='job/bootstrap.log';path="$launchFile.bootstrap.log"}}
    if($launchFile -and (Test-Path -LiteralPath "$launchFile.metadata.json")){$files+=@{name='job/launch-metadata.json';path="$launchFile.metadata.json"}}
    $consoleOwner=Join-Path $result.run 'console-owner.json';if(Test-Path -LiteralPath $consoleOwner){$files+=@{name='job/console-owner.json';path=$consoleOwner}}
    $consoleLock=Join-Path $result.run 'console-lock.json';if(Test-Path -LiteralPath $consoleLock){$files+=@{name='job/console-lock.json';path=$consoleLock}}
    $commandLog = Join-Path $result.run 'command.log'
    if (Test-Path $commandLog) {
      $files += @{name='job/command.log';path=$commandLog}
      $report = Select-String -LiteralPath $commandLog -Pattern 'report=(.+command\.json)$' | Select-Object -Last 1
      if ($report) {
        $profileDirectory = Split-Path $report.Matches[0].Groups[1].Value
        if (![IO.Path]::GetFullPath($profileDirectory).StartsWith([IO.Path]::GetFullPath($config.sourceCacheDir), [StringComparison]::OrdinalIgnoreCase)) { throw 'Profiler report is outside the configured cache' }
        foreach ($artifact in Get-ChildItem -LiteralPath $profileDirectory -File | Where-Object { ($_.Extension -in '.json','.png','.cpuprofile' -or $_.Name -match '^[a-f0-9]{64}\.(trace|probes)\.json\.gz$') -and $_.Length -le 16MB } | Sort-Object @{Expression={if ($_.Extension -eq '.png') {1} else {0}}},Name | Select-Object -First 64) {
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
    if($launchFile -and (Test-Path -LiteralPath "$launchFile.bootstrap.log")){Get-Content -LiteralPath "$launchFile.bootstrap.log" -Tail 20}
    if ($launchText) { Write-Output $launchText }
    if ($latest) { Get-Content -LiteralPath $latest.FullName -Tail 80 }
    if ($latest) {
      $build = Select-String -LiteralPath $latest.FullName -Pattern '^\[performance\] development build log=(.+)$' | Select-Object -Last 1
      if ($build) { Get-Content -LiteralPath $build.Matches[0].Groups[1].Value -Tail 30 }
    }
  } else {
    [PlaysrcReadbackGuard]::Stage='owned-process-query'
    $owned=if($launchFile){OwnedRunner ([IO.Path]::ChangeExtension($launchFile,'owner.json'))}else{$null}
    $processes=@(if($owned -and !$owned.HasExited){@{ProcessId=$owned.Id;role='owned-local-job'}})
    if($owned){$owned.Dispose()}
    if($Action -eq 'Recover'){$processes = @(Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -and ($_.CommandLine.Replace('/','\').Contains($directory.Replace('/','\')) -or ($_.Name -eq 'bun.exe' -and $_.CommandLine -like '*local-job.ts*' -and $_.CommandLine.Contains($Job))) } | Select-Object ProcessId,ParentProcessId,Name)}
    $running = Test-Path (Join-Path $directory 'running')
    if ($Action -eq 'Recover') {
      if (!$Task -or !$selectedTask -or $selectedTask.State -ne 'Ready' -or !$taskInfo.LastTaskResult -or $processes.Count -or !$running -or $result) { throw 'Recovery requires this recorded failed task, no live job processes, and no completed result' }
      $record = @{task=$Task;taskInfo=$taskInfo;at=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds();runningRecord=(Read-PlainJobText (Join-Path $directory 'running'));outcome='interrupted-before-result'}
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
    $marker = if ($running) { @{file=(Get-Item (Join-Path $directory 'running') | Select-Object CreationTimeUtc,Length);content=(Read-PlainJobText (Join-Path $directory 'running'))} } else { $null }
    [PlaysrcReadbackGuard]::Stage='status-serialize'
    @{job=$Job;task=$taskState;taskInfo=$taskInfo;running=$running;marker=$marker;latestRun=$latestRun.FullName;processes=$processes;log=$latest.FullName;launchLog=$launchFile;launchError=$(if (!$result) { $launchText } else { $null });result=$summary} | ConvertTo-Json -Depth 8 -Compress
  }
  exit 0
}
function ArgumentArray([string]$text,[int]$maximum) {
 if($text.Length -gt 24576 -or $text.Trim() -notmatch '^\[.*\]$'){throw 'Invalid job argument array'}
 $values=ConvertFrom-Json -InputObject $text
 if($null -eq $values){if($text -notmatch '^\s*\[\s*\]\s*$'){throw 'Invalid job argument array'};$values=@()}elseif($values -isnot [array]){$values=@($values)}
 if($values.Count -gt $maximum){throw 'Job argument count exceeds its bound'}
 foreach($value in $values){if($value -isnot [string] -or $value.Length -gt 1024 -or $value.Contains([char]0)){throw 'Invalid job argument'}}
 # ConvertFrom-Json in Windows PowerShell emits an array as ONE pipeline
 # object. Do not wrap its command in @(), which nests all test files together.
 return ,$values
}
if ($JobArguments -ne '[]') {
  $workload=ArgumentArray $JobArguments 20
  if($workload.Count -lt 1){throw 'Expected a workload'}
} elseif ($Action -eq 'Run') {
  if ($Profile -notmatch '^[a-z0-9-]+$') { throw 'Expected a normal profile name' }
  if ($Grep.Length -gt 512 -or $Grep.Contains([char]0)) {throw 'Profile selection exceeds its bound'}
} elseif ($Action -eq 'BuildStage') {
  if (!$Stage -or ($Stage -eq 'resources' -and $Target -notmatch '^[a-z0-9_]+$') -or ($Stage -ne 'resources' -and $Target)) { throw 'Expected wasm | producer | resources with a local build target' }
} elseif ($Action -eq 'Build' -and $Target -notmatch '^[a-z0-9_]+$') { throw 'Expected a local build target' }
$token = [Guid]::NewGuid().ToString()
$name = "playsrc-local-job-$token"
$log = Join-Path $directory "$token-launch.log"
$policy = Join-Path $directory "$token-policy.json"
function Quote([string]$value) { return "'" + $value.Replace("'", "''") + "'" }
$extra = ArgumentArray $ProfileArguments 16
if ($Grep) { $extra += @('--grep', $Grep) }
if ($FreshBrowser) { $extra += '--fresh-browser' }
if ($extra.Count -gt 16) { throw 'Invalid profiler argument array' }
if($JobArguments -eq '[]') {
  $workload=if($Action -eq 'Build'){@('build',$Target)}elseif($Action -eq 'BuildStage'){@('build-stage',$Stage)+$(if($Target){@($Target)}else{@()})}elseif($Action -eq 'Test'){@('test')+(ArgumentArray $TestArguments 19)}elseif($Action -eq 'Diagnostic'){@('diagnostic',"$Milliseconds","$DiagnosticExit")}else{@('profile',$Profile)+$extra}
}
$arguments = ($workload|ForEach-Object {Quote $_}) -join ' '
$planText = & $bun (Join-Path $root 'tools/playsrc/src/local-job.ts') plan @workload
if($LASTEXITCODE){throw 'Workload classification failed; nothing scheduled'}
$plan = $planText | ConvertFrom-Json
if($plan.interactive -isnot [bool]){throw 'Missing workload classification'}
$ownerLog=[IO.Path]::ChangeExtension($log,'owner.json')
# Windows PowerShell 5 turns redirected native stderr into ErrorRecords. Queue
# progress is not a terminating exception; the actual native exit code owns it.
$command = "`$ErrorActionPreference='Stop'; `$ProgressPreference='SilentlyContinue'; Set-Location $(Quote $root); @{taskPriority=5;processPriority=[string][Diagnostics.Process]::GetCurrentProcess().PriorityClass;pid=`$PID} | ConvertTo-Json -Compress | Set-Content -Encoding UTF8 $(Quote $policy); try { . $(Quote (Join-Path $root 'tools/playsrc/windows-job-console.ps1')) -Receipt $(Quote $ownerLog) -Workload $(Quote (ConvertTo-Json -InputObject @($workload) -Compress)); `$ErrorActionPreference='Continue'; & $(Quote $bun) tools/playsrc/src/local-job.ts run $(Quote $Job) --task $(Quote $name) $arguments > $(Quote $log) 2> $(Quote "$log.bootstrap.log"); `$code=`$LASTEXITCODE; `$ErrorActionPreference='Stop'; exit `$code } catch { `$_ | Out-String | Out-File -LiteralPath $(Quote $log) -Append; exit 1 }"
$encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command))
New-Item -ItemType File -Path $log | Out-Null
$taskAction = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument "-NoProfile -NonInteractive -WindowStyle Hidden -EncodedCommand $encoded" -WorkingDirectory $root
$logonType = if($plan.interactive){'Interactive'}else{'S4U'}
$principal = New-ScheduledTaskPrincipal -UserId ([Security.Principal.WindowsIdentity]::GetCurrent().Name) -LogonType $logonType -RunLevel Limited
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
