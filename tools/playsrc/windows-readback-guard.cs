using System;
using System.Diagnostics;
using System.IO;
using System.Threading;

// Runs only inside a verified noninteractive readback helper. It never signals
// another PID, a process tree, the scheduled command, or the profiler lockholder.
public static class PlaysrcReadbackGuard {
 public static volatile string Stage="start";
 static Timer timer;
 public static void Start(int milliseconds,long maximumPrivateBytes,string receipt) {
  var self=Process.GetCurrentProcess();int pid=self.Id;
  long birth=new DateTimeOffset(self.StartTime.ToUniversalTime()).ToUnixTimeMilliseconds();self.Dispose();
  long age=Math.Max(0,DateTimeOffset.UtcNow.ToUnixTimeMilliseconds()-birth);
  var clock=Stopwatch.StartNew();int finishing=0;
  timer=new Timer(_=>{
   long bytes=0;try {using(var process=Process.GetCurrentProcess()){bytes=process.PrivateMemorySize64;}}catch{}
   long elapsed=age+clock.ElapsedMilliseconds;
   string reason=elapsed>=milliseconds?"deadline":bytes>maximumPrivateBytes?"private-memory-limit":null;
   if(reason==null||Interlocked.Exchange(ref finishing,1)!=0)return;
   try {File.WriteAllText(receipt,"{\"privacy\":\"private-helper-fault\",\"pid\":"+pid+",\"createdEpoch\":"+birth+",\"elapsedMilliseconds\":"+elapsed+",\"privateBytes\":"+bytes+",\"stage\":\""+Stage+"\",\"reason\":\""+reason+"\"}");}catch{}
   Environment.Exit(124);
  },null,0,100);
 }
}
