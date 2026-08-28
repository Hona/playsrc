# SSH is the transport. Task Scheduler only bridges SSH's noninteractive
# session to the existing user's physical console; it runs the normal profiler.
param(
  [ValidateSet('Run','Build','BuildStage','Status','Result','Logs','Doctor','Wait','Artifacts')][string]$Action = 'Run',
  [Parameter(Mandatory=$true)][string]$Job,
  [string]$Profile,
  [string]$Grep = '',
  [switch]$FreshBrowser,
  [string]$Target,
  [ValidateSet('wasm','producer','resources')][string]$Stage,
  [string]$Task,
  [switch]$Ready
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
if($Action -in 'Wait','Status','Result','Logs','Artifacts') {
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
if ($Action -ne 'Run' -and $Action -ne 'Build' -and $Action -ne 'BuildStage') {
  [PlaysrcReadbackGuard]::Stage='exact-task-query'
  $tasks = if ($Action -eq 'Status' -and $Task) { @(Get-ScheduledTask -TaskName $Task -ErrorAction SilentlyContinue) } else { @() }
  $taskState = $null
  $launchFile = $null
  $launchText = $null
  if ($Task) {
    if ($Task -notmatch '^playsrc-local-job-([a-f0-9-]{36})$' -or !(Test-Path (Join-Path $directory "$($Matches[1])-launch.log"))) { throw 'Task is not recorded for this job' }
    $launchFile = Join-Path $directory "$($Matches[1])-launch.log"
    [PlaysrcReadbackGuard]::Stage='launch-log-read'
    $launchText = Read-PlainJobText $launchFile
    $taskState = $tasks | Where-Object TaskName -eq $Task | Select-Object TaskName,State
  }
  $latestRun = Get-ChildItem -LiteralPath $directory -Directory | Where-Object { $_.Name -match '^[a-f0-9-]{36}$' } | Sort-Object CreationTime -Descending | Select-Object -First 1
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
  if ($Action -eq 'Artifacts') {
    [PlaysrcReadbackGuard]::Stage='artifact-enumeration'
    if (!$result -or $result.schema -ne 'playsrc-local-job-result-v1') { throw 'This task has no completed result to collect' }
    $files = @(@{name='job/result.json';path=(Join-Path $result.run 'result.json')})
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
        foreach ($artifact in Get-ChildItem -LiteralPath $profileDirectory -File | Where-Object { $_.Extension -in '.json','.png','.cpuprofile' -and $_.Length -le 16MB } | Sort-Object @{Expression={if ($_.Extension -eq '.png') {1} else {0}}},Name | Select-Object -First 64) {
          $files += @{name="profile/$($artifact.Name)";path=$artifact.FullName}
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
    $running = Test-Path (Join-Path $directory 'running')
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
    [PlaysrcReadbackGuard]::Stage='status-serialize'
    @{job=$Job;task=$taskState;running=$running;processes=$processes;log=$latest.FullName;launchLog=$launchFile;launchError=$(if (!$result) { $launchText } else { $null });result=$summary} | ConvertTo-Json -Depth 8 -Compress
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
function Quote([string]$value) { return "'" + $value.Replace("'", "''") + "'" }
$arguments = if ($Action -eq 'Build') { "build $(Quote $Target)" } elseif ($Action -eq 'BuildStage') { "build-stage $(Quote $Stage)" + $(if ($Stage -eq 'resources') { " $(Quote $Target)" } else { '' }) } else { "--ready profile $(Quote $Profile)" + $(if ($Grep) { " --grep $(Quote $Grep)" } else { '' }) + $(if ($FreshBrowser) { ' --fresh-browser' } else { '' }) }
$ownerLog=[IO.Path]::ChangeExtension($log,'owner.json')
$command = "`$ErrorActionPreference='Stop'; `$ProgressPreference='SilentlyContinue'; Set-Location $(Quote $root); try { . $(Quote (Join-Path $root 'tools/playsrc/windows-job-console.ps1')) -Receipt $(Quote $ownerLog); & $(Quote $bun) tools/playsrc/src/local-job.ts run $(Quote $Job) $arguments *> $(Quote $log); exit `$LASTEXITCODE } catch { `$_ | Out-String | Out-File -LiteralPath $(Quote $log) -Append; exit 1 }"
$encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command))
New-Item -ItemType File -Path $log | Out-Null
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
@{task=$name;job=$Job;action=$Action;profile=$Profile;target=$Target;log=$log;results=$directory} | ConvertTo-Json -Compress
