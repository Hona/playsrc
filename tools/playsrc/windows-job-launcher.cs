using System;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Threading;

// No GUI and no console: the normal local-job CLI owns the checked lock before
// creating its interactive console scope. No shell reparses user arguments.
public static class PlaysrcJobLauncher {
 [DllImport("kernel32.dll")] static extern IntPtr GetConsoleWindow();
 static string Quote(string value) {
  var result=new StringBuilder("\"");int slashes=0;
  foreach(char c in value) {
   if(c=='\\'){slashes++;continue;}
   result.Append('\\',c=='"'?slashes*2+1:slashes);result.Append(c);slashes=0;
  }
  result.Append('\\',slashes*2);return result.Append('"').ToString();
 }
 public static int Main(string[] args) {
  if(args.Length!=2 || !Path.IsPathRooted(args[0])) return 2;
  string[] request;
  try {
   byte[] bytes=File.ReadAllBytes(args[0]);if(bytes.Length>131072)return 2;
   using(var sha=SHA256.Create()){if(BitConverter.ToString(sha.ComputeHash(bytes)).Replace("-","").ToLowerInvariant()!=args[1])return 2;}
   string[] lines=Encoding.UTF8.GetString(bytes).TrimEnd('\r','\n').Split('\n');
   if(lines.Length<7 || lines.Length>32)return 2;
   request=new string[lines.Length];for(int i=0;i<lines.Length;i++)request[i]=Encoding.UTF8.GetString(Convert.FromBase64String(lines[i].TrimEnd('\r')));
   if(!Path.IsPathRooted(request[0])||!Path.IsPathRooted(request[1])||!Path.IsPathRooted(request[2])||request[3]!="tools/playsrc/src/local-job.ts"||request[4]!="run")return 2;
   string directory=Path.GetDirectoryName(args[0]);
   if(!String.Equals(Path.GetFileName(directory),request[5],StringComparison.OrdinalIgnoreCase)||!String.Equals(Path.Combine(directory,"checkout"),request[1],StringComparison.OrdinalIgnoreCase))return 2;
  } catch {return 2;}
  using(var log=new StreamWriter(new FileStream(request[2],FileMode.Create,FileAccess.Write,FileShare.Read))) {
   object gate=new object();bool closed=false;
   var arguments=new StringBuilder();for(int i=3;i<request.Length;i++){if(i>3)arguments.Append(' ');arguments.Append(Quote(request[i]));}
   var start=new ProcessStartInfo(request[0],arguments.ToString());start.WorkingDirectory=request[1];
   start.UseShellExecute=false;start.CreateNoWindow=true;start.RedirectStandardOutput=true;start.RedirectStandardError=true;
   using(var child=new Process()) {
    child.StartInfo=start;var stdoutDone=new ManualResetEventSlim(false);var stderrDone=new ManualResetEventSlim(false);
    DataReceivedEventHandler output=(sender,e)=>{if(e.Data!=null)lock(gate){if(!closed){log.WriteLine(e.Data);log.Flush();}}};
    child.OutputDataReceived+=(sender,e)=>{output(sender,e);if(e.Data==null)stdoutDone.Set();};
    child.ErrorDataReceived+=(sender,e)=>{output(sender,e);if(e.Data==null)stderrDone.Set();};
    try {
     if(!child.Start())return 1;
     var self=Process.GetCurrentProcess();
     File.WriteAllText(Path.ChangeExtension(request[2],"owner.json"),"{\"privacy\":\"private-native-owner\",\"pid\":"+self.Id+",\"childPid\":"+child.Id+",\"sessionId\":"+self.SessionId+",\"consoleWindow\":"+GetConsoleWindow().ToInt64()+",\"createNoWindow\":true,\"at\":"+DateTimeOffset.UtcNow.ToUnixTimeMilliseconds()+"}");
     child.BeginOutputReadLine();child.BeginErrorReadLine();
     if(!child.WaitForExit(175000)) {
      if(!child.HasExited){var stop=new ProcessStartInfo("taskkill.exe","/PID "+child.Id+" /T /F");stop.UseShellExecute=false;stop.CreateNoWindow=true;using(var kill=Process.Start(stop)){kill.WaitForExit(1000);}}
      child.WaitForExit(1000);lock(gate){log.WriteLine("Owned scheduled command exceeded its175second limit");}return 124;
     }
     if(!stdoutDone.Wait(500)||!stderrDone.Wait(500)){lock(gate){log.WriteLine("Owned bootstrap output did not close after process exit");}return 1;}
     return child.ExitCode;
    } catch(Exception error){lock(gate){log.WriteLine(error.Message);}return 1;}
    finally{lock(gate){closed=true;log.Flush();}}
   }
  }
 }
}
