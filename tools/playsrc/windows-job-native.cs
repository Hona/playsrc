using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using System.Web.Script.Serialization;

// One scheduled job, one decision, one owned process tree. All UI is native
// TaskDialogIndirect; no game/window focus, input suppression or desktop repair.
// https://learn.microsoft.com/windows/win32/api/commctrl/nf-commctrl-taskdialogindirect
// https://learn.microsoft.com/windows/win32/procthread/job-objects
public static class PlaysrcNativeJob {
 public sealed class Request {
  public string job, task, run, action, executable, cwd, lockPath, lockToken, manifest;
  public string[] arguments;
  public int ownerPid;
  public long deadline;
  public bool diagnostic;
 }
 public sealed class Dialog {
  public string decision, error, pixels, sha256;
  public long displayedAt, decidedAt, dismissedAt, visibleMilliseconds, window, bytes;
  public int sessionId;
 }
 public sealed class Receipt {
  public string schema="playsrc-native-job-v1", job, task, run, action, lockToken, outcome="failed", error;
  public int ownerPid, helperPid, sessionId, childPid, exitCode;
  public long ownerCreatedAt, helperCreatedAt, childCreatedAt, startedAt, finishedAt, commandStartedAt, teardownAt;
  public bool treeEmpty;
  public Dialog consent, completion;
 }
 static readonly JavaScriptSerializer Json=new JavaScriptSerializer();
 static long Now {get{return DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();}}
 static void Save(string file,object value) {
  string temporary=file+"."+Guid.NewGuid()+".tmp";
  try {using(var stream=new FileStream(temporary,FileMode.CreateNew,FileAccess.Write,FileShare.None)) using(var writer=new StreamWriter(stream,new UTF8Encoding(false))) {writer.Write(Json.Serialize(value));writer.Flush();stream.Flush(true);} File.Move(temporary,file);}
  finally {if(File.Exists(temporary))File.Delete(temporary);}
 }
 static Exception Native(string operation) {return new Win32Exception(Marshal.GetLastWin32Error(),operation);}
 static void Check(bool ok,string operation) {if(!ok)throw Native(operation);}

 [StructLayout(LayoutKind.Explicit,Size=24)] struct SessionInfo {
  [FieldOffset(0)] public uint Level; [FieldOffset(8)] public uint Session;
  [FieldOffset(12)] public int State; [FieldOffset(16)] public int Flags;
 }
 [DllImport("kernel32.dll")] static extern uint WTSGetActiveConsoleSessionId();
 [DllImport("wtsapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool WTSQuerySessionInformationW(IntPtr server,uint session,int kind,out IntPtr data,out int bytes);
 [DllImport("wtsapi32.dll")] static extern void WTSFreeMemory(IntPtr data);
 [DllImport("user32.dll",SetLastError=true)] static extern IntPtr OpenInputDesktop(uint flags,bool inherit,uint access);
 [DllImport("user32.dll")] static extern bool CloseDesktop(IntPtr desktop);
 [DllImport("user32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool GetUserObjectInformationW(IntPtr handle,int index,StringBuilder value,int bytes,out int needed);
 static int ConsoleSession() {
  var version=Environment.OSVersion.Version;
  if(version.Major<6 || (version.Major==6 && version.Minor<2))throw new Exception("Unsupported Windows session flags");
  uint session=WTSGetActiveConsoleSessionId(); int self=Process.GetCurrentProcess().SessionId;
  if(session==0 || session==uint.MaxValue || session!=self)throw new Exception("No matching physical console session");
  IntPtr data;int bytes;
  Check(WTSQuerySessionInformationW(IntPtr.Zero,session,25,out data,out bytes),"WTS session");
  try {if(bytes<24)throw new Exception("Incomplete WTS session");var info=(SessionInfo)Marshal.PtrToStructure(data,typeof(SessionInfo));if(info.Level!=1 || info.Session!=session || info.State!=0 || info.Flags!=1)throw new Exception("Console is not active and unlocked");}finally{WTSFreeMemory(data);}
  Check(WTSQuerySessionInformationW(IntPtr.Zero,session,16,out data,out bytes),"WTS protocol");
  try{if(bytes<2 || Marshal.ReadInt16(data)!=0)throw new Exception("Not a physical console");}finally{WTSFreeMemory(data);}
  var desktop=OpenInputDesktop(0,false,1);
  if(desktop==IntPtr.Zero)throw Native("Input desktop unavailable");
  try{var name=new StringBuilder(256);int needed;Check(GetUserObjectInformationW(desktop,2,name,512,out needed),"Input desktop name");if(name.ToString()!="Default")throw new Exception("Input desktop is not Default");}finally{CloseDesktop(desktop);}
  return self;
 }

 [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] struct Activation {
  public int size;public uint flags;public string source;public ushort architecture,language;
  public string directory,resource,application;public IntPtr module;
 }
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr CreateActCtxW(ref Activation data);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool ActivateActCtx(IntPtr context,out UIntPtr cookie);
 [DllImport("kernel32.dll")] static extern bool DeactivateActCtx(uint flags,UIntPtr cookie);
 [DllImport("kernel32.dll")] static extern void ReleaseActCtx(IntPtr context);
 [StructLayout(LayoutKind.Sequential,Pack=1,CharSet=CharSet.Unicode)] struct Button {public int id;[MarshalAs(UnmanagedType.LPWStr)] public string text;}
 delegate int DialogCallback(IntPtr window,uint notification,IntPtr wparam,IntPtr lparam,IntPtr data);
 [StructLayout(LayoutKind.Sequential,Pack=1,CharSet=CharSet.Unicode)] struct DialogConfig {
  public uint size;public IntPtr parent,instance;public uint flags,commonButtons;
  [MarshalAs(UnmanagedType.LPWStr)] public string title;
  public IntPtr icon;
  [MarshalAs(UnmanagedType.LPWStr)] public string instruction,content;
  public uint buttonCount;public IntPtr buttons;public int defaultButton;public uint radioCount;public IntPtr radios;public int defaultRadio;
  [MarshalAs(UnmanagedType.LPWStr)] public string verification,expanded,expandedControl,collapsedControl;
  public IntPtr footerIcon;[MarshalAs(UnmanagedType.LPWStr)] public string footer;
  public DialogCallback callback;public IntPtr data;public uint width;
 }
 [DllImport("comctl32.dll",CharSet=CharSet.Unicode,PreserveSig=true)] static extern int TaskDialogIndirect(ref DialogConfig config,out int button,out int radio,out bool verification);
 [DllImport("user32.dll")] static extern IntPtr SendMessageW(IntPtr window,uint message,IntPtr wparam,IntPtr lparam);
 [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr window);
 [DllImport("user32.dll")] static extern bool IsIconic(IntPtr window);
 [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr window);
 [DllImport("user32.dll")] static extern bool ShowWindow(IntPtr window,int command);
 [StructLayout(LayoutKind.Sequential)] struct Rect {public int left,top,right,bottom;}
 [StructLayout(LayoutKind.Sequential)] struct Point {public int x,y;}
 [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr window,out Rect rect);
 [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr window,out Rect rect);
 [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr window,ref Point point);
 [DllImport("user32.dll")] static extern bool UpdateWindow(IntPtr window);
 static void Pixels(IntPtr window,string file,Dialog record) {
  if(GetForegroundWindow()!=window)throw new Exception("Diagnostic dialog is not foreground; no pixel claim");
  // Client pixels only: no rounded frame corners, shadows or private desktop.
  Rect rect;Check(GetClientRect(window,out rect),"Dialog bounds");var origin=new Point();Check(ClientToScreen(window,ref origin),"Dialog client origin");
  int width=rect.right-rect.left,height=rect.bottom-rect.top;
  if(width<=0 || height<=0 || (long)width*height>4000000)throw new Exception("Invalid dialog pixel bounds");
  using(var bitmap=new Bitmap(width,height)) {using(var graphics=Graphics.FromImage(bitmap))graphics.CopyFromScreen(origin.x,origin.y,0,0,bitmap.Size);bitmap.Save(file,System.Drawing.Imaging.ImageFormat.Png);}
  if(GetForegroundWindow()!=window)throw new Exception("Foreground changed during diagnostic capture");
  record.pixels=file;record.bytes=new FileInfo(file).Length;
  using(var hash=SHA256.Create())record.sha256=BitConverter.ToString(hash.ComputeHash(File.ReadAllBytes(file))).Replace("-","").ToLowerInvariant();
 }
 static Dialog Show(Request request,bool completion,string outcome,Process owner) {
  var record=new Dialog();var clock=new Stopwatch();bool selected=false;IntPtr buttons=IntPtr.Zero;
  DialogCallback callback=null;
  // Callbacks, button messages and timer run on ONE UI thread. Latch before
  // closing so a nested timer/button cannot decide twice.
  callback=(window,notification,wparam,lparam,data)=>{
   try {
    // One ordinary activation of THIS requested message box, never a browser
    // or another user's window. Do not retry if the user switches away.
    if(notification==0){ShowWindow(window,5);SetForegroundWindow(window);}
    if(notification==2 || notification==4) {
     record.sessionId=ConsoleSession();
     if(owner.HasExited || (!completion && File.Exists(Path.Combine(request.run,"cancel"))))throw new Exception("Job cancelled");
     if(Now>=request.deadline)throw new Exception("Job deadline exceeded");
    }
    if(notification==4 && !selected) {
     if(!IsWindowVisible(window) || IsIconic(window) || GetForegroundWindow()!=window)throw new Exception("Prompt is not displayed in the foreground");
     if(!clock.IsRunning) {
      UpdateWindow(window);record.window=window.ToInt64();record.displayedAt=Now;clock.Start();
      if(request.diagnostic)Pixels(window,Path.Combine(request.run,completion?"completion.png":"consent.png"),record);
      Save(Path.Combine(request.run,completion?"completion-displayed.json":"consent-displayed.json"),new {job=request.job,task=request.task,run=request.run,action=request.action,helperPid=Process.GetCurrentProcess().Id,helperCreatedAt=new DateTimeOffset(Process.GetCurrentProcess().StartTime.ToUniversalTime()).ToUnixTimeMilliseconds(),dialog=record});
     }
     if(clock.ElapsedMilliseconds>=3000) {
      selected=true;record.decision=completion?"dismissed-timeout":"approved-timeout";record.decidedAt=Now;record.visibleMilliseconds=clock.ElapsedMilliseconds;
      SendMessageW(window,0x400+102,new IntPtr(completion?1:100),IntPtr.Zero);
     }
    }
    if(notification==2 && !selected) {
     // Close/Escape is IDCANCEL, never timeout consent. Early genuine clicks
     // establish display before the first ~200ms timer callback.
     if(!IsWindowVisible(window) || IsIconic(window))throw new Exception("Prompt is not displayed");
     if(!clock.IsRunning){record.displayedAt=Now;record.window=window.ToInt64();clock.Start();}
     selected=true;record.decidedAt=Now;record.visibleMilliseconds=clock.ElapsedMilliseconds;
     record.decision=completion?"dismissed":wparam.ToInt32()==100?"approved":"denied";
    }
   } catch(Exception error) {
    record.error=error.Message;record.decision="display-failed";record.decidedAt=Now;selected=true;
    if(notification!=2)SendMessageW(window,0x400+102,new IntPtr(2),IntPtr.Zero);
   }
   return 0;
  };
  try {
   record.sessionId=ConsoleSession();
   int size=Marshal.SizeOf(typeof(Button));buttons=Marshal.AllocHGlobal(size*2);
   Marshal.StructureToPtr(new Button{id=100,text="Approve"},buttons,false);
   Marshal.StructureToPtr(new Button{id=101,text="Deny"},IntPtr.Add(buttons,size),false);
   var config=new DialogConfig {size=(uint)Marshal.SizeOf(typeof(DialogConfig)),flags=0x8|0x800,commonButtons=completion?1u:0u,
    title="playsrc delegated job",instruction=completion?"Job "+outcome:"Approve delegated job?",
    content="Action: "+request.action+"\nJob: "+request.job+"\nTask: "+request.task+"\nRun: "+Path.GetFileName(request.run)+"\n\n"+
     (completion?"This job has stopped. Hands-off is no longer needed for this job.\nThis message closes automatically after 3 seconds.":"Approve starts immediately. Deny, Escape or close will not start the job.\nNo answer for 3 seconds after display means approval (AFK).\nPerformance sampling still requires genuine idle and an unobscured browser."),
    buttonCount=completion?0u:2u,buttons=completion?IntPtr.Zero:buttons,defaultButton=completion?1:101,callback=callback,width=400};
   int button,radio;bool verification;int result=TaskDialogIndirect(ref config,out button,out radio,out verification);
   record.dismissedAt=Now;
   if(result!=0 || !selected || record.displayedAt==0)throw new Exception(record.error??("Native dialog did not produce a displayed decision (HRESULT "+result+")"));
   if(!completion && record.error==null)ConsoleSession();
  } catch(Exception error) {record.error=error.Message;record.decision="display-failed";record.dismissedAt=Now;}
  finally {GC.KeepAlive(callback);if(buttons!=IntPtr.Zero){int size=Marshal.SizeOf(typeof(Button));Marshal.DestroyStructure(buttons,typeof(Button));Marshal.DestroyStructure(IntPtr.Add(buttons,size),typeof(Button));Marshal.FreeHGlobal(buttons);}}
  return record;
 }

 [StructLayout(LayoutKind.Sequential)] struct BasicLimit {public long processTime,jobTime;public uint flags;public UIntPtr minimum,maximum;public uint activeLimit;public UIntPtr affinity;public uint priority,scheduling;}
 [StructLayout(LayoutKind.Sequential)] struct Io {public ulong a,b,c,d,e,f;}
 [StructLayout(LayoutKind.Sequential)] struct ExtendedLimit {public BasicLimit basic;public Io io;public UIntPtr processMemory,jobMemory,peakProcess,peakJob;}
 [StructLayout(LayoutKind.Sequential)] struct Accounting {public long a,b,c,d;public uint faults,total,active,terminated;}
 [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] struct Startup {public int size;public string reserved,desktop,title;public uint x,y,width,height,xChars,yChars,fill,flags;public ushort show,reserved2;public IntPtr bytes,input,output,error;}
 [StructLayout(LayoutKind.Sequential)] struct ProcessInfo {public IntPtr process,thread;public uint pid,tid;}
 [StructLayout(LayoutKind.Sequential)] struct Security {public int length;public IntPtr descriptor;[MarshalAs(UnmanagedType.Bool)] public bool inherit;}
 [DllImport("kernel32.dll",SetLastError=true)] static extern IntPtr CreateJobObjectW(IntPtr security,IntPtr name);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool SetInformationJobObject(IntPtr job,int kind,ref ExtendedLimit info,int size);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool QueryInformationJobObject(IntPtr job,int kind,out Accounting info,int size,IntPtr returned);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool AssignProcessToJobObject(IntPtr job,IntPtr process);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool TerminateJobObject(IntPtr job,uint exit);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool TerminateProcess(IntPtr process,uint exit);
 [DllImport("kernel32.dll",SetLastError=true)] static extern uint ResumeThread(IntPtr thread);
 [DllImport("kernel32.dll",SetLastError=true)] static extern uint WaitForSingleObject(IntPtr handle,uint milliseconds);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetExitCodeProcess(IntPtr process,out uint exit);
 [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr CreateFileW(string name,uint access,uint share,ref Security security,uint creation,uint flags,IntPtr template);
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool CreateProcessW(string application,StringBuilder command,IntPtr processSecurity,IntPtr threadSecurity,bool inherit,uint flags,IntPtr environment,string cwd,ref Startup startup,out ProcessInfo process);
 static string Quote(string value) {
  var output=new StringBuilder("\"");int slashes=0;
  foreach(char c in value){if(c=='\\'){slashes++;continue;}output.Append('\\',c=='\"'?slashes*2+1:slashes);output.Append(c);slashes=0;}
  return output.Append('\\',slashes*2).Append('"').ToString();
 }
 static uint Active(IntPtr job) {Accounting info;Check(QueryInformationJobObject(job,1,out info,Marshal.SizeOf(typeof(Accounting)),IntPtr.Zero),"Query owned tree");return info.active;}
 static void Execute(Request request,Receipt receipt,Process owner) {
  IntPtr job=IntPtr.Zero,log=IntPtr.Zero,input=IntPtr.Zero;var child=new ProcessInfo();bool resumed=false;
  try {
   job=CreateJobObjectW(IntPtr.Zero,IntPtr.Zero);if(job==IntPtr.Zero)throw Native("Create owned job");
   var limits=new ExtendedLimit();limits.basic.flags=0x2000; // KILL_ON_JOB_CLOSE, no breakaway
   Check(SetInformationJobObject(job,9,ref limits,Marshal.SizeOf(typeof(ExtendedLimit))),"Bound owned job");
   var security=new Security{length=Marshal.SizeOf(typeof(Security)),inherit=true};
   log=CreateFileW(Path.Combine(request.run,"command.log"),0x40000000,1,ref security,1,0,IntPtr.Zero);
   if(log==new IntPtr(-1))throw Native("Create command log");
   input=CreateFileW("NUL",0x80000000,3,ref security,3,0,IntPtr.Zero);if(input==new IntPtr(-1))throw Native("Open null input");
   var startup=new Startup{size=Marshal.SizeOf(typeof(Startup)),flags=0x100,input=input,output=log,error=log};
   var command=new StringBuilder(Quote(request.executable));foreach(string argument in request.arguments)command.Append(' ').Append(Quote(argument));
   // Never run even one workload instruction until assignment succeeds.
   Check(CreateProcessW(request.executable,command,IntPtr.Zero,IntPtr.Zero,true,0x08000000|4,IntPtr.Zero,request.cwd,ref startup,out child),"Create suspended workload");
   receipt.childPid=(int)child.pid;
   using(var process=Process.GetProcessById(receipt.childPid))receipt.childCreatedAt=new DateTimeOffset(process.StartTime.ToUniversalTime()).ToUnixTimeMilliseconds();
   Check(AssignProcessToJobObject(job,child.process),"Assign owned workload");
   ConsoleSession();
   if(owner.HasExited || File.Exists(Path.Combine(request.run,"cancel")))throw new OperationCanceledException("Job cancelled before dispatch");
   receipt.commandStartedAt=Now;
   if(ResumeThread(child.thread)==uint.MaxValue)throw Native("Resume workload");resumed=true;
   while(WaitForSingleObject(child.process,50)==258) {
    if(owner.HasExited || File.Exists(Path.Combine(request.run,"cancel")))throw new OperationCanceledException("Job cancelled");
    if(Now>=request.deadline-7000)throw new OperationCanceledException("175-second job budget: command deadline reached");
    using(var self=Process.GetCurrentProcess())if(self.PrivateMemorySize64>536870912)throw new Exception("Native helper memory bound exceeded");
   }
   uint exit;Check(GetExitCodeProcess(child.process,out exit),"Read workload exit");receipt.exitCode=(int)exit;receipt.outcome=exit==0?"completed":"failed";
  } catch(OperationCanceledException error) {receipt.outcome="cancelled";receipt.error=error.Message;}
  catch(Exception error) {receipt.outcome="failed";receipt.error=error.Message;}
  finally {
   // Completion is AFTER all of this invocation's children are gone.
   if(child.process!=IntPtr.Zero && !resumed)TerminateProcess(child.process,1);
   if(job!=IntPtr.Zero){Check(TerminateJobObject(job,1),"Terminate owned descendants");var wait=Stopwatch.StartNew();while(Active(job)>0 && wait.ElapsedMilliseconds<4000)Thread.Sleep(20);receipt.treeEmpty=Active(job)==0;CloseHandle(job);}
   else receipt.treeEmpty=child.process==IntPtr.Zero;
   if(child.thread!=IntPtr.Zero)CloseHandle(child.thread);if(child.process!=IntPtr.Zero)CloseHandle(child.process);
   if(log!=IntPtr.Zero && log!=new IntPtr(-1))CloseHandle(log);if(input!=IntPtr.Zero && input!=new IntPtr(-1))CloseHandle(input);
   receipt.teardownAt=Now;
  }
 }

 public static void Run(string file,int parentPid) {
  var request=Json.Deserialize<Request>(File.ReadAllText(file));
  if(request.ownerPid!=parentPid || request.deadline<=Now || request.deadline-Now>175000)throw new Exception("Invalid native owner/deadline");
  var receipt=new Receipt{job=request.job,task=request.task,run=request.run,action=request.action,lockToken=request.lockToken,ownerPid=parentPid,helperPid=Process.GetCurrentProcess().Id,startedAt=Now};
  IntPtr context=IntPtr.Zero;UIntPtr cookie=UIntPtr.Zero;
  using(var owner=Process.GetProcessById(parentPid)) {
   receipt.ownerCreatedAt=new DateTimeOffset(owner.StartTime.ToUniversalTime()).ToUnixTimeMilliseconds();
   receipt.helperCreatedAt=new DateTimeOffset(Process.GetCurrentProcess().StartTime.ToUniversalTime()).ToUnixTimeMilliseconds();
   var ownerHandle=owner.Handle; // Hold identity, not a recycled PID.
   int faulted=0;
   using(var guard=new Timer(_=>{
    string reason=null;long bytes=0;
    try {using(var self=Process.GetCurrentProcess())bytes=self.PrivateMemorySize64;
     if(owner.HasExited)reason="owner-exited";else if(Now>=request.deadline)reason="deadline";else if(bytes>536870912)reason="private-memory-limit";
    }catch{reason="owner-readback-failed";}
    if(reason==null || Interlocked.Exchange(ref faulted,1)!=0)return;
    try{Save(Path.Combine(request.run,"native-fault.json"),new {reason=reason,pid=receipt.helperPid,createdAt=receipt.helperCreatedAt,at=Now,privateBytes=bytes});}catch{}
    // OS closes the non-inheritable job handle and tears down its owned tree.
    Environment.Exit(124);
   },null,0,100)) {
   try {
    var held=Json.Deserialize<Dictionary<string,object>>(File.ReadAllText(request.lockPath));
    if((string)held["token"]!=request.lockToken || Convert.ToInt32(held["pid"])!=parentPid)throw new Exception("Machine-wide ownership differs");
    receipt.sessionId=ConsoleSession();
    var activation=new Activation{size=Marshal.SizeOf(typeof(Activation)),source=request.manifest};
    context=CreateActCtxW(ref activation);if(context==new IntPtr(-1))throw Native("Native dialog activation context");
    Check(ActivateActCtx(context,out cookie),"Activate native common controls");
    if(File.Exists(Path.Combine(request.run,"cancel")))throw new OperationCanceledException("Job cancelled before prompt");
    receipt.consent=Show(request,false,null,owner);
    Save(Path.Combine(request.run,"consent.json"),receipt);
    if(receipt.consent.error!=null) {receipt.error=receipt.consent.error;receipt.treeEmpty=true;if(File.Exists(Path.Combine(request.run,"cancel")))receipt.outcome="cancelled";}
    else if(receipt.consent.decision=="denied") {receipt.outcome="denied";receipt.treeEmpty=true;}
    else if(receipt.consent.decision=="approved" || receipt.consent.decision=="approved-timeout") {
     Environment.SetEnvironmentVariable("PLAYSRC_LOCAL_JOB_CONSENT",Path.Combine(request.run,"consent.json"));
     Environment.SetEnvironmentVariable("PLAYSRC_LOCAL_JOB_LOCK",request.lockPath);
     Environment.SetEnvironmentVariable("PLAYSRC_LOCAL_JOB_DEADLINE",(request.deadline-7000).ToString());
     Execute(request,receipt,owner);
    } else throw new Exception("Unrecognized native decision");
   } catch(Exception error) {receipt.error=error.Message;receipt.outcome=error is OperationCanceledException?"cancelled":"failed";if(receipt.childPid==0)receipt.treeEmpty=true;}
   finally {
    receipt.teardownAt=Now;
    if(receipt.treeEmpty) {
     Console.WriteLine(Json.Serialize(new {phase="teardown",job=request.job,run=request.run,helperPid=receipt.helperPid,treeEmpty=true}));
     var read=Task.Factory.StartNew(()=>Console.ReadLine());
     while(!read.Wait(50) && !owner.HasExited && Now<request.deadline-3500){}
     try {
      if(!read.IsCompleted || read.Result==null)throw new Exception("Source verification handoff did not finish");
      var verification=Json.Deserialize<Dictionary<string,object>>(read.Result);
      if(!verification.ContainsKey("error"))throw new Exception("Malformed source verification handoff");
      if(verification["error"]!=null)throw new Exception((string)verification["error"]);
     } catch(Exception error) {receipt.outcome="failed";receipt.error=error.Message;}
    }
    if(receipt.treeEmpty && cookie!=UIntPtr.Zero)receipt.completion=Show(request,true,receipt.outcome,owner);
    if(!receipt.treeEmpty){receipt.outcome="failed";receipt.error="Owned child teardown unconfirmed; "+receipt.error;}
    receipt.finishedAt=Now;
    Save(Path.Combine(request.run,"native-result.json"),receipt);
    if(cookie!=UIntPtr.Zero)DeactivateActCtx(0,cookie);
    if(context!=IntPtr.Zero && context!=new IntPtr(-1))ReleaseActCtx(context);
   }
   }
  }
  Console.WriteLine(Json.Serialize(receipt));
 }
}
