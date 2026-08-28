# SSH is the transport. Task Scheduler only bridges SSH's noninteractive
# session to the existing user's physical console; it runs the normal profiler.
param(
  [Parameter(Mandatory=$true)][string]$Job,
  [Parameter(Mandatory=$true)][string]$Profile,
  [switch]$Ready
)
$ErrorActionPreference = 'Stop'
if (!$Ready) { throw 'Pass -Ready only for a freshly approved hands-off window' }
if ($Job -notmatch '^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$') { throw 'Invalid job ID' }
if ($Profile -notmatch '^[a-z0-9-]+$') { throw 'Expected a normal profile name' }
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$bun = (Get-Command bun -CommandType Application).Source
$config = Get-Content -Raw (Join-Path $root 'playsrc.local.json') | ConvertFrom-Json
$directory = Join-Path $config.sourceCacheDir "local-jobs/$Job"
if (!(Test-Path -LiteralPath (Join-Path $directory 'job.json'))) { throw 'Prepare this job first' }
$token = [Guid]::NewGuid().ToString()
$name = "playsrc-local-job-$token"
$log = Join-Path $directory "$token-launch.log"
function Quote([string]$value) { return "'" + $value.Replace("'", "''") + "'" }
$command = "`$ErrorActionPreference='Stop'; try { Set-Location $(Quote $root); & $(Quote $bun) tools/playsrc/src/local-job.ts run $(Quote $Job) --ready profile $(Quote $Profile) *> $(Quote $log); exit `$LASTEXITCODE } finally { Unregister-ScheduledTask -TaskName $(Quote $name) -Confirm:`$false }"
$encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command))
$action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument "-NoProfile -NonInteractive -EncodedCommand $encoded" -WorkingDirectory $root
$principal = New-ScheduledTaskPrincipal -UserId ([Security.Principal.WindowsIdentity]::GetCurrent().Name) -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit (New-TimeSpan -Minutes 3) -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
try {
  Register-ScheduledTask -TaskName $name -Action $action -Principal $principal -Settings $settings | Out-Null
  Start-ScheduledTask -TaskName $name
} catch {
  Unregister-ScheduledTask -TaskName $name -Confirm:$false -ErrorAction SilentlyContinue
  throw
}
@{task=$name;job=$Job;profile=$Profile;log=$log;results=$directory} | ConvertTo-Json -Compress
