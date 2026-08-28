using System;
using System.Diagnostics;
using System.IO;
using System.Threading;

// GUI-subsystem entry point with no windows. Scheduled console applications can
// allocate a terminal before the profiler acquires its machine-wide lock.
// Only this owned child tree is started, waited for, and (on timeout) retired.
public static class PlaysrcJobLauncher {
 public static int Main(string[] args) {
  if(args.Length!=2 || args[0].Length>131072 || !Path.IsPathRooted(args[1])) return 2;
  try { Convert.FromBase64String(args[0]); } catch { return 2; }
  using(var log=new StreamWriter(new FileStream(args[1],FileMode.CreateNew,FileAccess.Write,FileShare.Read))) {
   object gate=new object(); bool closed=false;
   var start=new ProcessStartInfo("powershell.exe","-NoProfile -NonInteractive -EncodedCommand "+args[0]);
   start.UseShellExecute=false; start.CreateNoWindow=true;
   start.RedirectStandardOutput=true; start.RedirectStandardError=true;
   using(var child=new Process()) {
    child.StartInfo=start;
    var stdoutDone=new ManualResetEventSlim(false);var stderrDone=new ManualResetEventSlim(false);
    DataReceivedEventHandler output=(sender,e)=>{if(e.Data!=null)lock(gate){if(!closed){log.WriteLine(e.Data);log.Flush();}}};
    child.OutputDataReceived+=(sender,e)=>{output(sender,e);if(e.Data==null)stdoutDone.Set();};
    child.ErrorDataReceived+=(sender,e)=>{output(sender,e);if(e.Data==null)stderrDone.Set();};
    try {
     if(!child.Start()) return 1;
     child.BeginOutputReadLine(); child.BeginErrorReadLine();
     if(!child.WaitForExit(175000)) {
      // PID comes from the still-live Process we created, never a name match.
      if(!child.HasExited) {
       var stop=new ProcessStartInfo("taskkill.exe","/PID "+child.Id+" /T /F");
       stop.UseShellExecute=false; stop.CreateNoWindow=true;
       using(var kill=Process.Start(stop)){kill.WaitForExit(1000);}
      }
      child.WaitForExit(1000);
      lock(gate){log.WriteLine("Owned scheduled command exceeded its175second limit");}
      return 124;
     }
     if(!stdoutDone.Wait(500)||!stderrDone.Wait(500)){lock(gate){log.WriteLine("Owned bootstrap output did not close after process exit");}return 1;}
     return child.ExitCode;
    } catch(Exception error) {
     lock(gate){log.WriteLine(error.Message);}
     return 1;
    } finally {lock(gate){closed=true;log.Flush();}}
   }
  }
 }
}
