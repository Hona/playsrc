# Actual UI control test, not an authorization source. The observer may act only
# on the exact native diagnostic task/run/creation-time identity requested here.
# It never invokes a performance workload or interacts with another application.
param(
 [Parameter(Mandatory=$true)][string]$Job,
 [ValidateSet('approve','deny','close','escape','race','timeout','failure','cancel','queue')][string]$Case='timeout',
 [string]$Observe
)
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
$root=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$config=Get-Content -Raw (Join-Path $root 'playsrc.local.json')|ConvertFrom-Json
if($Job -notmatch '^[a-f0-9-]{36}$'){throw 'Invalid diagnostic job'}
$jobDirectory=Join-Path $config.sourceCacheDir "local-jobs/$Job"
if($Observe) {
 function Step([string]$stage){[IO.File]::WriteAllText((Join-Path $Observe 'observer-stage'),$stage)}
 Step 'entry'
 if((Split-Path $Observe) -ne (Join-Path $config.sourceCacheDir 'evidence/windows-job-ui-tests')){throw 'Invalid observer directory'}
 Add-Type -Path (Join-Path $PSScriptRoot 'windows-readback-guard.cs')
 [PlaysrcReadbackGuard]::Start(45000,536870912,(Join-Path $Observe 'observer-fault.json'))
 Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class DiagnosticDialog {
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window,out uint pid);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
 [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr window,uint message,IntPtr wparam,IntPtr lparam);
 [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr window,int id);
 delegate bool Child(IntPtr window,IntPtr data);
 [DllImport("user32.dll")] static extern bool EnumChildWindows(IntPtr parent,Child callback,IntPtr data);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr window,StringBuilder text,int count);
 public static string Text(IntPtr window){var text=new StringBuilder(128);GetWindowTextW(window,text,text.Capacity);return text.ToString();}
 public static IntPtr Button(IntPtr parent,string name){IntPtr found=IntPtr.Zero;EnumChildWindows(parent,(window,data)=>{if(Text(window).Replace("&","")==name)found=window;return true;},IntPtr.Zero);return found;}
}
'@
 [IO.File]::WriteAllText((Join-Path $Observe 'ready'),"$PID")
 Step 'request-wait'
 $deadline=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()+35000
 $requestFile=Join-Path $Observe 'request.json'
 while(!(Test-Path -LiteralPath $requestFile) -and [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() -lt $deadline){Start-Sleep -Milliseconds 20}
 $request=Get-Content -Raw -LiteralPath $requestFile|ConvertFrom-Json
 Step "run-link: $($request.task)"
 if($request.task -notmatch '^playsrc-local-job-([a-f0-9-]{36})$'){throw 'Invalid exact task'}
 $link=Join-Path $jobDirectory "$($Matches[1])-run.json"
 while(!(Test-Path -LiteralPath $link) -and [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() -lt $deadline){Start-Sleep -Milliseconds 20}
 $identity=Get-Content -Raw -LiteralPath $link|ConvertFrom-Json
 Step 'run-linked'
 if($identity.job -ne $Job -or $identity.task -ne $request.task -or (Split-Path $identity.run) -ne $jobDirectory){throw 'Exact diagnostic run differs'}
 $displayFile=Join-Path $identity.run 'consent-displayed.json'
 while(!(Test-Path -LiteralPath $displayFile) -and [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() -lt $deadline){Start-Sleep -Milliseconds 20}
 $display=Get-Content -Raw -LiteralPath $displayFile|ConvertFrom-Json
 Step 'display-read'
 if($display.job -ne $Job -or $display.task -ne $identity.task -or $display.run -ne $identity.run -or $display.action -notmatch '^diagnostic [0-9]+ [01]$'){throw 'Observer cannot act on a non-diagnostic workload'}
 $helper=[Diagnostics.Process]::GetProcessById([int]$display.helperPid)
 Step 'helper-read'
 if(([DateTimeOffset]$helper.StartTime.ToUniversalTime()).ToUnixTimeMilliseconds() -ne $display.helperCreatedAt -or $helper.SessionId -ne [Diagnostics.Process]::GetCurrentProcess().SessionId){throw 'Native helper creation/session differs'}
 $handle=[IntPtr][long]$display.dialog.window;[uint32]$pidOfWindow=0
 [void][DiagnosticDialog]::GetWindowThreadProcessId($handle,[ref]$pidOfWindow)
 if($pidOfWindow -ne $helper.Id -or ![DiagnosticDialog]::IsWindowVisible($handle)){throw 'Native diagnostic window differs'}
 $approve=[DiagnosticDialog]::Button($handle,'Approve');$deny=[DiagnosticDialog]::Button($handle,'Deny')
 $names=@([DiagnosticDialog]::Text($approve).Replace('&',''),[DiagnosticDialog]::Text($deny).Replace('&',''))
 Step 'controls-read'
 if($names -notcontains 'Approve' -or $names -notcontains 'Deny'){throw 'Native Approve/Deny controls were not found'}
 $began=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
 if($Case -eq 'race') {
   $remaining=3000-($began-$display.dialog.displayedAt)
   if($remaining -gt 0){Start-Sleep -Milliseconds $remaining}
 }
 if($Case -in 'approve','deny','race','cancel','failure') {
   $button=if($Case -in 'deny','race'){$deny}else{$approve}
   if(![DiagnosticDialog]::PostMessageW($button,0xF5,[IntPtr]0,[IntPtr]0) -and $Case -ne 'race'){throw 'Native button delivery failed'}
 } elseif($Case -eq 'close') {
   if(![DiagnosticDialog]::PostMessageW($handle,0x10,[IntPtr]0,[IntPtr]0)){throw 'Native close delivery failed'}
 } elseif($Case -eq 'escape') {
   if(![DiagnosticDialog]::PostMessageW($handle,0x100,[IntPtr]27,[IntPtr]0)){throw 'Escape key delivery failed'}
   [void][DiagnosticDialog]::PostMessageW($handle,0x101,[IntPtr]27,[IntPtr]0)
 }
 if($Case -eq 'cancel') {
   $commandLog=Join-Path $identity.run 'command.log'
   while(!(Test-Path -LiteralPath $commandLog) -and [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() -lt $deadline){Start-Sleep -Milliseconds 20}
   [IO.File]::WriteAllText((Join-Path $identity.run 'cancel'),'native diagnostic cancellation')
 }
 @{case=$Case;job=$Job;task=$identity.task;run=$identity.run;helperPid=$helper.Id;helperCreatedAt=$display.helperCreatedAt;window=$handle.ToInt64();controls=$names;actionStartedAt=$began;actionFinishedAt=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds();authorization='real native control delivery; diagnostic only'}|ConvertTo-Json -Depth 6|Set-Content -Encoding UTF8 (Join-Path $Observe 'interaction.json')
 $helper.Dispose()
 exit 0
}

$directory=Join-Path $config.sourceCacheDir "evidence/windows-job-ui-tests/$([Guid]::NewGuid())"
New-Item -ItemType Directory -Path $directory|Out-Null
function Quote([string]$value){return "'"+$value.Replace("'","''")+"'"}
$observerName="playsrc-dialog-observer-$([Guid]::NewGuid())"
$command="try { & $(Quote $PSCommandPath) -Job $(Quote $Job) -Case $(Quote $Case) -Observe $(Quote $directory) *> $(Quote (Join-Path $directory 'observer.log')) } catch { `$_ | Out-String | Set-Content -LiteralPath $(Quote (Join-Path $directory 'observer-error.log')); exit 1 }"
$encoded=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command))
$action=New-ScheduledTaskAction -Execute 'powershell.exe' -Argument "-NoProfile -NonInteractive -WindowStyle Hidden -EncodedCommand $encoded"
$principal=New-ScheduledTaskPrincipal -UserId ([Security.Principal.WindowsIdentity]::GetCurrent().Name) -LogonType Interactive -RunLevel Limited
$settings=New-ScheduledTaskSettingsSet -Priority 5 -ExecutionTimeLimit (New-TimeSpan -Seconds 45) -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
try {
 Register-ScheduledTask -TaskName $observerName -Action $action -Principal $principal -Settings $settings|Out-Null
 Start-ScheduledTask -TaskName $observerName
 $deadline=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()+10000
 while(!(Test-Path -LiteralPath (Join-Path $directory 'ready')) -and [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() -lt $deadline){Start-Sleep -Milliseconds 20}
 if(!(Test-Path -LiteralPath (Join-Path $directory 'ready'))){throw 'Diagnostic observer failed to start'}
 $duration=if($Case -eq 'cancel'){30000}else{250};$exit=if($Case -eq 'failure'){1}else{0}
 $launch=& powershell.exe -NoProfile -NonInteractive -File (Join-Path $PSScriptRoot 'windows-job.ps1') -Job $Job -Action Diagnostic -Milliseconds $duration -DiagnosticExit $exit|ConvertFrom-Json
 if($LASTEXITCODE){throw 'Diagnostic launch failed'}
 $launch|ConvertTo-Json -Compress|Set-Content -Encoding UTF8 (Join-Path $directory 'request.tmp')
 Move-Item -LiteralPath (Join-Path $directory 'request.tmp') -Destination (Join-Path $directory 'request.json')
 $second=$null
 if($Case -eq 'queue') {
   $second=& powershell.exe -NoProfile -NonInteractive -File (Join-Path $PSScriptRoot 'windows-job.ps1') -Job $Job -Action Diagnostic -Milliseconds 250|ConvertFrom-Json
   if($LASTEXITCODE){throw 'Second queued diagnostic launch failed'}
 }
 $results=@()
 foreach($task in @($launch.task,$second.task)|Where-Object {$_}) {
   & powershell.exe -NoProfile -NonInteractive -File (Join-Path $PSScriptRoot 'windows-job.ps1') -Job $Job -Task $task -Action Wait|Out-Null
   if($LASTEXITCODE){throw 'Diagnostic wait failed'}
   $result=& powershell.exe -NoProfile -NonInteractive -File (Join-Path $PSScriptRoot 'windows-job.ps1') -Job $Job -Task $task -Action Result|ConvertFrom-Json
   $result|ConvertTo-Json -Depth 12|Set-Content -Encoding UTF8 (Join-Path $directory "$task-result.json")
   if(!$result.result.native){throw 'Native diagnostic has no receipt'}
   $native=$result.result.native
   if(!$native.treeEmpty -or !$native.completion.displayedAt -or $native.completion.error){throw 'Owned teardown/completion was not proven'}
   if($native.completion.displayedAt -lt $native.teardownAt){throw 'Completion preceded teardown'}
   $count=0;$log=Join-Path $native.run 'command.log'
   if(Test-Path -LiteralPath $log){$count=@(Select-String -LiteralPath $log -SimpleMatch 'native diagnostic workload').Count}
   if($count -gt 1 -or ($native.commandStartedAt -eq 0 -and $count -ne 0)){throw 'More than one dispatch or a denied dispatch'}
   try {$live=[Diagnostics.Process]::GetProcessById([int]$native.helperPid);if(([DateTimeOffset]$live.StartTime.ToUniversalTime()).ToUnixTimeMilliseconds() -eq $native.helperCreatedAt){throw 'Native helper leaked'}}catch [ArgumentException]{}finally{if($live){$live.Dispose()}}
   $results+=,$native
 }
 $first=$results[0]
 if($Case -in 'deny','close','escape' -and ($first.outcome -ne 'denied' -or $first.commandStartedAt)){throw 'Deny/close/Escape did not deny'}
 if($Case -eq 'approve' -and ($first.consent.decision -ne 'approved' -or $first.commandStartedAt-$first.consent.decidedAt -ge 1000 -or $first.consent.visibleMilliseconds -ge 3000)){throw 'Approve did not dispatch immediately'}
 if($Case -in 'timeout','queue' -and ($first.consent.decision -ne 'approved-timeout' -or $first.consent.visibleMilliseconds -lt 3000)){throw 'Unattended visible timeout was not proven'}
 if($Case -eq 'failure' -and $first.outcome -ne 'failed'){throw 'Workload failure was not preserved'}
 if($Case -eq 'cancel' -and $first.outcome -ne 'cancelled'){throw 'Cancellation was not preserved'}
 if($Case -eq 'race' -and $first.consent.decision -notin 'denied','approved-timeout'){throw 'Unexpected boundary race decision'}
 if($Case -eq 'queue' -and ($results.Count -ne 2 -or $results[1].consent.displayedAt -le $first.completion.dismissedAt)){throw 'Queued dialogs overlapped'}
 @{case=$Case;assertions='real controls, exact task/run/creation identity, at-most-one dispatch, owned teardown and completion';passed=$true}|ConvertTo-Json|Set-Content -Encoding UTF8 (Join-Path $directory 'verification.json')
 @{directory=$directory;case=$Case;tasks=@($launch.task,$second.task)|Where-Object {$_}}|ConvertTo-Json -Compress
} finally {
 $observer=Get-ScheduledTask -TaskName $observerName -ErrorAction SilentlyContinue
 if($observer -and $observer.State -eq 'Running'){Stop-ScheduledTask -TaskName $observerName}
 Unregister-ScheduledTask -TaskName $observerName -Confirm:$false -ErrorAction SilentlyContinue
}
