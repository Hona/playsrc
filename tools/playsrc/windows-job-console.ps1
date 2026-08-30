# Dot-sourced by the owned scheduled-task process, never by a second console.
# Read-only identity receipt: no titles, arguments, credentials or focus calls.
param([Parameter(Mandatory=$true)][string]$Receipt,[Parameter(Mandatory=$true)][string]$Workload)
$invocation=ConvertFrom-Json -InputObject $Workload
$planText=& bun (Join-Path $PSScriptRoot 'src/local-job.ts') plan @invocation
if($LASTEXITCODE){throw 'Scheduled workload classification failed'}
$plan=$planText|ConvertFrom-Json
$self=[System.Diagnostics.Process]::GetCurrentProcess()
$record=@{pid=$self.Id;sessionId=$self.SessionId;startedEpoch=([DateTimeOffset]$self.StartTime.ToUniversalTime()).ToUnixTimeMilliseconds();interactive=$plan.interactive;invocation=@($invocation)}
if(!$plan.interactive){
 [IO.File]::WriteAllText("$Receipt.tmp",($record|ConvertTo-Json -Depth 6 -Compress))
 [IO.File]::Move("$Receipt.tmp",$Receipt)
 return
}
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class PlaysrcJobConsole {
 [DllImport("kernel32.dll")] public static extern IntPtr GetConsoleWindow();
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window,out uint pid);
 [DllImport("kernel32.dll")] public static extern uint GetConsoleProcessList([Out] uint[] processes,uint count);
}
'@
$self=[System.Diagnostics.Process]::GetCurrentProcess()
$selfInfo=Get-CimInstance Win32_Process -Filter "ProcessId=$($self.Id)"
$parent=Get-CimInstance Win32_Process -Filter "ProcessId=$($selfInfo.ParentProcessId)"
# Zero here means our shell has no console. It is not a foreground admission;
# the browser's separate native foreground guard still rejects a zero HWND.
$handle=[PlaysrcJobConsole]::GetConsoleWindow();[uint32]$consolePid=0
$null=[PlaysrcJobConsole]::GetWindowThreadProcessId($handle,[ref]$consolePid)
$members=New-Object uint32[] 32;$count=[PlaysrcJobConsole]::GetConsoleProcessList($members,32)
$chain=@();$next=[int]$consolePid
for($index=0;$index -lt 4 -and $next -gt 0;$index++) {
 $entry=Get-CimInstance Win32_Process -Filter "ProcessId=$next"
 if(!$entry){break}
 $chain+=@{pid=$entry.ProcessId;parentPid=$entry.ParentProcessId;name=$entry.Name;created=$entry.CreationDate.ToUniversalTime().ToString('o')}
 $next=[int]$entry.ParentProcessId
}
$record+=@{privacy='private-native-owner';at=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds();parentPid=$selfInfo.ParentProcessId;parentName=$parent.Name;consoleWindow=$handle.ToInt64();consolePid=$consolePid;consoleProcessCount=$count;consoleProcesses=@($members|Select-Object -First ([Math]::Min($count,32)));consoleOwnerChain=$chain}
[IO.File]::WriteAllText("$Receipt.tmp",($record|ConvertTo-Json -Depth 6 -Compress))
[IO.File]::Move("$Receipt.tmp",$Receipt)
