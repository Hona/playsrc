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
if ($Job -notmatch '^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$') { throw 'Invalid job ID' }
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$bun = (Get-Command bun -CommandType Application).Source
$config = Get-Content -Raw (Join-Path $root 'playsrc.local.json') | ConvertFrom-Json
$directory = Join-Path $config.sourceCacheDir "local-jobs/$Job"
if (!(Test-Path -LiteralPath (Join-Path $directory 'job.json'))) { throw 'Prepare this job first' }
if ($Action -eq 'Wait') {
  $runner = Get-CimInstance Win32_Process | Where-Object { $_.Name -eq 'bun.exe' -and $_.CommandLine -like "*local-job.ts run $Job*" }
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
  $tasks = if ($Action -eq 'Status') { @(Get-ScheduledTask -TaskName 'playsrc-local-job-*' -ErrorAction SilentlyContinue) } else { @() }
  $taskState = $null
  $launchFile = $null
  $launchText = $null
  if ($Task) {
    if ($Task -notmatch '^playsrc-local-job-([a-f0-9-]{36})$' -or !(Test-Path (Join-Path $directory "$($Matches[1])-launch.log"))) { throw 'Task is not recorded for this job' }
    $launchFile = Join-Path $directory "$($Matches[1])-launch.log"
    $launchText = Get-Content -Raw -LiteralPath $launchFile
    $taskState = $tasks | Where-Object TaskName -eq $Task | Select-Object TaskName,State
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
    if ($launchText) { Write-Output $launchText }
    if ($latest) { Get-Content -LiteralPath $latest.FullName -Tail 80 }
    if ($latest) {
      $build = Select-String -LiteralPath $latest.FullName -Pattern '^\[performance\] development build log=(.+)$' | Select-Object -Last 1
      if ($build) { Get-Content -LiteralPath $build.Matches[0].Groups[1].Value -Tail 30 }
    }
  } else {
    $processes = @(Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -and ($_.CommandLine.Replace('/','\').Contains($directory.Replace('/','\')) -or ($_.Name -eq 'bun.exe' -and $_.CommandLine -like "*local-job.ts run $Job*")) } | Select-Object ProcessId,ParentProcessId,Name)
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
$command = "`$ErrorActionPreference='Stop'; Set-Location $(Quote $root); & $(Quote $bun) tools/playsrc/src/local-job.ts run $(Quote $Job) $arguments *> $(Quote $log); exit `$LASTEXITCODE"
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
@{task=$name;job=$Job;action=$Action;profile=$Profile;target=$Target;log=$log;results=$directory} | ConvertTo-Json -Compress
