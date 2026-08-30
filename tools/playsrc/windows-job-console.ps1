# Dot-sourced by the scheduled task before FIFO admission. Publish only the
# creation-bound launcher identity here; no native compilation/desktop probe.
# The owned native stage performs physical-console admission when ready.
param([Parameter(Mandatory=$true)][string]$Receipt,[Parameter(Mandatory=$true)][string]$Workload)
$invocation=ConvertFrom-Json -InputObject $Workload
$planText=& bun (Join-Path $PSScriptRoot 'src/local-job.ts') plan @invocation
if($LASTEXITCODE){throw 'Scheduled workload classification failed'}
$plan=$planText|ConvertFrom-Json
$self=[System.Diagnostics.Process]::GetCurrentProcess()
$record=@{pid=$self.Id;sessionId=$self.SessionId;startedEpoch=([DateTimeOffset]$self.StartTime.ToUniversalTime()).ToUnixTimeMilliseconds();interactive=$plan.interactive;invocation=@($invocation)}
[IO.File]::WriteAllText("$Receipt.tmp",($record|ConvertTo-Json -Depth 6 -Compress))
[IO.File]::Move("$Receipt.tmp",$Receipt)
