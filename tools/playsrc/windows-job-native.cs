using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using System.Web.Script.Serialization;

// One scheduled job, one validated classification, one owned process tree. UI is
// TaskDialogIndirect; no game/window focus, input suppression or desktop repair.
// https://learn.microsoft.com/windows/win32/api/commctrl/nf-commctrl-taskdialogindirect
// https://learn.microsoft.com/windows/win32/procthread/job-objects
public static partial class PlaysrcNativeJob {
 public sealed class Request {
  public string job, task, run, action, cwd, lockPath, lockToken, manifest;
   public string preflightFailure, dialogDirectory;
  public string[] command, invocation;
  public int ownerPid;
  public long deadline;
 }
 public sealed class ValidatedRequest {public Request request;public bool interactive;}
 public sealed class Dialog {
  public string decision, error;
  public long displayedAt, decidedAt, dismissedAt, visibleMilliseconds, window;
  public int sessionId;
 }
 public sealed class Receipt {
   public string schema="playsrc-native-job-v2", job, task, run, action, lockToken, outcome="failed", error;
   public List<DesktopStage> desktop=new List<DesktopStage>();
  public string[] invocation;
  public int ownerPid, helperPid, sessionId, childPid, uiInvocations;
  public int? exitCode;
  public long ownerCreatedAt, helperCreatedAt, childCreatedAt, startedAt, finishedAt, commandStartedAt, teardownAt;
  public long helperPeakPrivateBytes;
  public bool treeEmpty, interactive;
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
 [DllImport("user32.dll",SetLastError=true)] static extern bool SetWindowPos(IntPtr window,IntPtr after,int x,int y,int width,int height,uint flags);
 [StructLayout(LayoutKind.Sequential)] struct Rect {public int left,top,right,bottom;}
 [StructLayout(LayoutKind.Sequential)] struct Point {public int x,y;}
 [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr window,out Rect rect);
 [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr window,out Rect rect);
 [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr window,ref Point point);
 [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(Point point);
 [DllImport("user32.dll")] static extern IntPtr GetAncestor(IntPtr window,uint flags);
 [DllImport("user32.dll")] static extern bool UpdateWindow(IntPtr window);
 static bool Presented(IntPtr window) {
  if(!IsWindowVisible(window) || IsIconic(window))return false;
  Rect rect;if(!GetClientRect(window,out rect) || rect.right<20 || rect.bottom<20)return false;
  var origin=new Point();if(!ClientToScreen(window,ref origin))return false;
  foreach(var point in new[]{new Point{x=8,y=8},new Point{x=rect.right-8,y=8},new Point{x=8,y=rect.bottom-8},new Point{x=rect.right-8,y=rect.bottom-8},new Point{x=rect.right/2,y=rect.bottom/2}}) {
   if(GetAncestor(WindowFromPoint(new Point{x=origin.x+point.x,y=origin.y+point.y}),2)!=window)return false;
  }
  return true;
 }
 static Dialog Show(Request request,bool completion,string outcome,Process owner) {
  var record=new Dialog();var clock=new Stopwatch();bool selected=false;IntPtr buttons=IntPtr.Zero;
  DialogCallback callback=null;
  // Callbacks, button messages and timer run on ONE UI thread. Latch before
  // closing so a nested timer/button cannot decide twice.
  callback=(window,notification,wparam,lparam,data)=>{
   try {
    // A normal topmost native alert, scoped ONLY to this requested message box.
    // Windows may deny foreground activation while the user types in another
    // app; actual on-screen presentation, not stolen focus, starts the timer.
    if(notification==0){ShowWindow(window,5);Check(SetWindowPos(window,new IntPtr(-1),0,0,0,0,0x13),"Present requested message box");SetForegroundWindow(window);}
    if(notification==2 || notification==4) {
     record.sessionId=ConsoleSession();
     if(owner.HasExited || (!completion && File.Exists(Path.Combine(request.run,"cancel"))))throw new Exception("Job cancelled");
     if(Now>=request.deadline)throw new Exception("Job deadline exceeded");
    }
    if(notification==4 && !selected) {
     if(!IsWindowVisible(window) || IsIconic(window))throw new Exception("Prompt is not displayed");
     if(!clock.IsRunning) {
      if(!Presented(window))throw new Exception("Prompt was not visibly presented");
      UpdateWindow(window);record.window=window.ToInt64();record.displayedAt=Now;clock.Start();
       Save(Path.Combine(request.dialogDirectory,completion?"completion-displayed.json":"consent-displayed.json"),new {job=request.job,task=request.task,run=request.run,action=request.action,helperPid=Process.GetCurrentProcess().Id,helperCreatedAt=new DateTimeOffset(Process.GetCurrentProcess().StartTime.ToUniversalTime()).ToUnixTimeMilliseconds(),dialog=record});
     }
     // After confirmed presentation, switching to another app is non-response,
     // not a display failure or proof of AFK. Sampling has its own idle guard.
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
     title="playsrc delegated job",instruction=completion?"Browser stage "+outcome:"Approve prepared browser stage?",
    content="Action: "+request.action+"\nJob: "+request.job+"\nTask: "+request.task+"\nRun: "+Path.GetFileName(request.run)+"\n\n"+
      (completion?"Browser and input use have ended. Hands-off is no longer needed.\nBackground artifact work may still be finishing.\nThis message closes automatically after 3 seconds.":"Preparation is complete. Approve starts browser admission now.\nDeny, Escape or close will not launch the browser.\nNo answer for 3 seconds after display means approval (AFK).\nPerformance sampling still requires genuine idle and an unobscured browser."),
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
 [StructLayout(LayoutKind.Sequential)] struct ExtendedStartup {public Startup basic;public IntPtr attributes;}
 [StructLayout(LayoutKind.Sequential)] struct ProcessInfo {public IntPtr process,thread;public uint pid,tid;}
 [StructLayout(LayoutKind.Sequential)] struct Security {public int length;public IntPtr descriptor;[MarshalAs(UnmanagedType.Bool)] public bool inherit;}
 [DllImport("kernel32.dll",SetLastError=true)] static extern IntPtr CreateJobObjectW(IntPtr security,IntPtr name);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool SetInformationJobObject(IntPtr job,int kind,ref ExtendedLimit info,int size);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool QueryInformationJobObject(IntPtr job,int kind,out Accounting info,int size,IntPtr returned);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool InitializeProcThreadAttributeList(IntPtr list,int count,uint flags,ref UIntPtr bytes);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool UpdateProcThreadAttribute(IntPtr list,uint flags,UIntPtr attribute,IntPtr value,UIntPtr bytes,IntPtr previous,IntPtr returned);
 [DllImport("kernel32.dll")] static extern void DeleteProcThreadAttributeList(IntPtr list);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool TerminateJobObject(IntPtr job,uint exit);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool TerminateProcess(IntPtr process,uint exit);
 [DllImport("kernel32.dll",SetLastError=true)] static extern uint ResumeThread(IntPtr thread);
 [DllImport("kernel32.dll",SetLastError=true)] static extern uint WaitForSingleObject(IntPtr handle,uint milliseconds);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetExitCodeProcess(IntPtr process,out uint exit);
  [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
  [DllImport("kernel32.dll",SetLastError=true)] static extern IntPtr OpenProcess(uint access,bool inherit,uint pid);
  [DllImport("kernel32.dll",SetLastError=true)] static extern bool IsProcessInJob(IntPtr process,IntPtr job,out bool belongs);
  delegate bool WindowCallback(IntPtr window,IntPtr data);
  [DllImport("user32.dll")] static extern bool EnumWindows(WindowCallback callback,IntPtr data);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window,out uint pid);
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr CreateFileW(string name,uint access,uint share,ref Security security,uint creation,uint flags,IntPtr template);
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool CreateProcessW(string application,StringBuilder command,IntPtr processSecurity,IntPtr threadSecurity,bool inherit,uint flags,IntPtr environment,string cwd,ref ExtendedStartup startup,out ProcessInfo process);
 static string Quote(string value) {
  var output=new StringBuilder("\"");int slashes=0;
  foreach(char c in value){if(c=='\\'){slashes++;continue;}output.Append('\\',c=='\"'?slashes*2+1:slashes);output.Append(c);slashes=0;}
  return output.Append('\\',slashes*2).Append('"').ToString();
 }
  static uint Active(IntPtr job) {Accounting info;Check(QueryInformationJobObject(job,1,out info,Marshal.SizeOf(typeof(Accounting)),IntPtr.Zero),"Query owned tree");return info.active;}
  public class DesktopRequest {
   public string job,task,run,lockToken,stage,preparedIdentity;
   public int childPid,helperPid;
   public long childCreatedAt,helperCreatedAt;
   public bool succeeded;
  }
  public sealed class DesktopStage : DesktopRequest {
   public string schema="playsrc-native-desktop-v1";
   public long preparedAt,desktopStartedAt,desktopReleasedAt;
   public Dialog consent,completion;
  }
  static string DesktopDirectory(Request request,int index) {return Path.Combine(request.run,"desktop",index.ToString("D4"));}
  static DesktopRequest ReadDesktop(Request request,Receipt receipt,string directory,string file,DesktopStage expected) {
   var value=Json.Deserialize<DesktopRequest>(File.ReadAllText(Path.Combine(directory,file)));
   if(value.job!=receipt.job || value.task!=receipt.task || value.run!=receipt.run || value.lockToken!=receipt.lockToken
    || value.childPid!=receipt.childPid || value.childCreatedAt!=receipt.childCreatedAt || value.helperPid!=receipt.helperPid || value.helperCreatedAt!=receipt.helperCreatedAt
    || String.IsNullOrEmpty(value.stage) || value.preparedIdentity==null || value.preparedIdentity.Length!=64
    || expected!=null && (value.stage!=expected.stage || value.preparedIdentity!=expected.preparedIdentity))throw new Exception("Desktop stage identity differs");
   using(var process=Process.GetProcessById(value.childPid))if(process.HasExited || new DateTimeOffset(process.StartTime.ToUniversalTime()).ToUnixTimeMilliseconds()!=value.childCreatedAt)throw new Exception("Desktop stage process changed");
   using(var hash=SHA256.Create())if(BitConverter.ToString(hash.ComputeHash(File.ReadAllBytes(Path.Combine(directory,"prepared.json")))).Replace("-","").ToLowerInvariant()!=value.preparedIdentity)throw new Exception("Desktop prepared identity changed");
   var held=Json.Deserialize<Dictionary<string,object>>(File.ReadAllText(request.lockPath));
   if((string)held["token"]!=receipt.lockToken || Convert.ToInt32(held["pid"])!=receipt.ownerPid)throw new Exception("Desktop stage lost resource ownership");
   return value;
  }
  static Dialog StageDialog(Request request,bool completion,Process owner) {
   IntPtr context=IntPtr.Zero;UIntPtr cookie=UIntPtr.Zero;
   try {
    ConsoleSession();
    var activation=new Activation{size=Marshal.SizeOf(typeof(Activation)),source=request.manifest};
    context=CreateActCtxW(ref activation);if(context==new IntPtr(-1))throw Native("Native dialog activation context");
    Check(ActivateActCtx(context,out cookie),"Activate native common controls");
    return Show(request,completion,"completed",owner);
   } finally {if(cookie!=UIntPtr.Zero)DeactivateActCtx(0,cookie);if(context!=IntPtr.Zero && context!=new IntPtr(-1))ReleaseActCtx(context);}
  }
  static void DesktopTransition(Request request,Receipt receipt,Process owner,IntPtr job) {
   var stage=receipt.desktop.Count==0?null:receipt.desktop[receipt.desktop.Count-1];
   var directory=DesktopDirectory(request,receipt.desktop.Count);
   if((stage==null || stage.desktopReleasedAt!=0) && File.Exists(Path.Combine(directory,"request.json"))) {
    if(!receipt.interactive)throw new Exception("Background command requested desktop ownership");
    var ready=ReadDesktop(request,receipt,directory,"request.json",null);
    foreach(var previous in receipt.desktop)if(previous.stage==ready.stage)throw new Exception("Desktop stage authorization cannot be reused");
    stage=Json.Deserialize<DesktopStage>(Json.Serialize(ready));stage.preparedAt=Now;receipt.desktop.Add(stage);
    request.dialogDirectory=directory;
    if(owner.HasExited || File.Exists(Path.Combine(request.run,"cancel")))throw new OperationCanceledException("Cancelled before desktop consent");
    receipt.sessionId=ConsoleSession();
    using(var requester=Process.GetProcessById(stage.childPid))stage.consent=StageDialog(request,false,requester);
    receipt.uiInvocations++;
    Save(Path.Combine(directory,"consent.json"),stage);
    if(stage.consent.error!=null) {if(File.Exists(Path.Combine(request.run,"cancel")))throw new OperationCanceledException(stage.consent.error);throw new Exception(stage.consent.error);}
    if(stage.consent.decision=="denied") {receipt.outcome="denied";throw new OperationCanceledException("Desktop stage denied");}
    if(stage.consent.decision!="approved" && stage.consent.decision!="approved-timeout")throw new Exception("Unrecognized native decision");
    if(owner.HasExited || File.Exists(Path.Combine(request.run,"cancel")))throw new OperationCanceledException("Cancelled after desktop consent");
    ReadDesktop(request,receipt,directory,"request.json",stage);
    stage.desktopStartedAt=Now;
    Save(Path.Combine(directory,"grant.json"),stage);
   }
   directory=DesktopDirectory(request,receipt.desktop.Count-1);
   if(stage!=null && stage.desktopStartedAt!=0 && stage.desktopReleasedAt==0 && File.Exists(Path.Combine(directory,"release.json"))) {
    var release=ReadDesktop(request,receipt,directory,"release.json",stage);
    RequireDesktopEmpty(job);
    stage.desktopReleasedAt=Now;stage.succeeded=release.succeeded;
    Save(Path.Combine(directory,"released.json"),stage);
    if(release.succeeded && !owner.HasExited && !File.Exists(Path.Combine(request.run,"cancel"))) {
     stage.completion=StageDialog(request,true,owner);receipt.uiInvocations++;
    }
    Save(Path.Combine(directory,"result.json"),stage);
   }
  }
  static void RequireDesktopEmpty(IntPtr job) {
    bool visible=false;
    WindowCallback checkWindow=(window,data)=>{
     if(!IsWindowVisible(window))return true;
     uint pid;GetWindowThreadProcessId(window,out pid);
     var process=OpenProcess(0x1000,false,pid);if(process==IntPtr.Zero)return true;
     try {bool belongs;if(IsProcessInJob(process,job,out belongs) && belongs)visible=true;}finally{CloseHandle(process);}
     return true;
    };
    Check(EnumWindows(checkWindow,IntPtr.Zero),"Confirm owned desktop teardown");GC.KeepAlive(checkWindow);
    if(visible)throw new Exception("Desktop release requested with an owned visible window");
  }
 static void Execute(Request request,Receipt receipt,Process owner) {
  IntPtr job=IntPtr.Zero,log=IntPtr.Zero,input=IntPtr.Zero,attributes=IntPtr.Zero,jobList=IntPtr.Zero;var child=new ProcessInfo();bool resumed=false,attributesInitialized=false;
  try {
   job=CreateJobObjectW(IntPtr.Zero,IntPtr.Zero);if(job==IntPtr.Zero)throw Native("Create owned job");
   var limits=new ExtendedLimit();limits.basic.flags=0x2000; // KILL_ON_JOB_CLOSE, no breakaway
   Check(SetInformationJobObject(job,9,ref limits,Marshal.SizeOf(typeof(ExtendedLimit))),"Bound owned job");
   var security=new Security{length=Marshal.SizeOf(typeof(Security)),inherit=true};
   log=CreateFileW(Path.Combine(request.run,"command.log"),0x40000000,1,ref security,1,0,IntPtr.Zero);
   if(log==new IntPtr(-1))throw Native("Create command log");
   input=CreateFileW("NUL",0x80000000,3,ref security,3,0,IntPtr.Zero);if(input==new IntPtr(-1))throw Native("Open null input");
   // Windows 10+ JOB_LIST assigns ownership atomically at creation. Creating a
   // suspended process and then assigning it leaves a helper-crash gap that
   // can strand an unassigned suspended child. There is no fallback to that gap.
   // https://learn.microsoft.com/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute
   UIntPtr attributeBytes=UIntPtr.Zero;
   InitializeProcThreadAttributeList(IntPtr.Zero,1,0,ref attributeBytes);
   if(attributeBytes.ToUInt64()==0 || attributeBytes.ToUInt64()>65536)throw Native("Size atomic job attributes");
   attributes=Marshal.AllocHGlobal((int)attributeBytes.ToUInt64());
   Check(InitializeProcThreadAttributeList(attributes,1,0,ref attributeBytes),"Initialize atomic job attributes");attributesInitialized=true;
   jobList=Marshal.AllocHGlobal(IntPtr.Size);Marshal.WriteIntPtr(jobList,job);
   Check(UpdateProcThreadAttribute(attributes,0,new UIntPtr(0x2000d),jobList,new UIntPtr((uint)IntPtr.Size),IntPtr.Zero,IntPtr.Zero),"Set atomic owned-job list");
   var startup=new ExtendedStartup{basic=new Startup{size=Marshal.SizeOf(typeof(ExtendedStartup)),flags=0x100,input=input,output=log,error=log},attributes=attributes};
   var command=new StringBuilder(Quote(request.command[0]));for(int index=1;index<request.command.Length;index++)command.Append(' ').Append(Quote(request.command[index]));
   // Never run even one workload instruction outside this owned job.
   Check(CreateProcessW(request.command[0],command,IntPtr.Zero,IntPtr.Zero,true,0x08000000|0x80000|4,IntPtr.Zero,request.cwd,ref startup,out child),"Create atomically owned suspended workload");
   receipt.childPid=(int)child.pid;
   using(var process=Process.GetProcessById(receipt.childPid))receipt.childCreatedAt=new DateTimeOffset(process.StartTime.ToUniversalTime()).ToUnixTimeMilliseconds();
   Save(Path.Combine(request.run,"dispatch.json"),new {pid=receipt.childPid,createdAt=receipt.childCreatedAt,helperPid=receipt.helperPid,helperCreatedAt=receipt.helperCreatedAt,job=request.job,task=request.task,run=request.run});
   if(owner.HasExited || File.Exists(Path.Combine(request.run,"cancel")))throw new OperationCanceledException("Job cancelled before dispatch");
   receipt.commandStartedAt=Now;
   if(ResumeThread(child.thread)==uint.MaxValue)throw Native("Resume workload");resumed=true;
   while(WaitForSingleObject(child.process,50)==258) {
    if(owner.HasExited || File.Exists(Path.Combine(request.run,"cancel")))throw new OperationCanceledException("Job cancelled");
    if(Now>=request.deadline-7000)throw new OperationCanceledException("175-second job budget: command deadline reached");
     using(var self=Process.GetCurrentProcess())if(self.PrivateMemorySize64>536870912)throw new Exception("Native helper memory bound exceeded");
     DesktopTransition(request,receipt,owner,job);
   }
    uint exit;Check(GetExitCodeProcess(child.process,out exit),"Read workload exit");receipt.exitCode=(int)exit;receipt.outcome=exit==0?"completed":"failed";
     if(receipt.interactive && exit==0 && (receipt.desktop.Count==0 || receipt.desktop[receipt.desktop.Count-1].desktopReleasedAt==0))throw new Exception("Interactive profile ended without its scoped desktop lifecycle");
   } catch(OperationCanceledException error) {if(receipt.outcome!="denied")receipt.outcome="cancelled";receipt.error=error.Message;}
  catch(Exception error) {receipt.outcome="failed";receipt.error=error.Message;}
  finally {
   // Completion is AFTER all of this invocation's children are gone.
   if(child.process!=IntPtr.Zero && !resumed)TerminateProcess(child.process,1);
   if(job!=IntPtr.Zero){Check(TerminateJobObject(job,1),"Terminate owned descendants");var wait=Stopwatch.StartNew();while(Active(job)>0 && wait.ElapsedMilliseconds<4000)Thread.Sleep(20);receipt.treeEmpty=Active(job)==0;CloseHandle(job);}
   else receipt.treeEmpty=child.process==IntPtr.Zero;
   if(child.process!=IntPtr.Zero && WaitForSingleObject(child.process,0)==0){uint exit;if(GetExitCodeProcess(child.process,out exit))receipt.exitCode=(int)exit;}
   if(child.thread!=IntPtr.Zero)CloseHandle(child.thread);if(child.process!=IntPtr.Zero)CloseHandle(child.process);
   if(log!=IntPtr.Zero && log!=new IntPtr(-1))CloseHandle(log);if(input!=IntPtr.Zero && input!=new IntPtr(-1))CloseHandle(input);
   if(attributesInitialized)DeleteProcThreadAttributeList(attributes);
   if(attributes!=IntPtr.Zero)Marshal.FreeHGlobal(attributes);if(jobList!=IntPtr.Zero)Marshal.FreeHGlobal(jobList);
   receipt.teardownAt=Now;
  }
 }

 public static void Run(string validatedJson,int parentPid) {
  var validated=Json.Deserialize<ValidatedRequest>(validatedJson);
  var request=validated.request;bool interactive=validated.interactive;
  if(request.ownerPid!=parentPid || request.deadline<=Now || request.deadline-Now>175000)throw new Exception("Invalid native owner/deadline");
  var receipt=new Receipt{job=request.job,task=request.task,run=request.run,action=request.action,invocation=request.invocation,interactive=interactive,lockToken=request.lockToken,ownerPid=parentPid,helperPid=Process.GetCurrentProcess().Id,startedAt=Now};
  using(var owner=Process.GetProcessById(parentPid)) {
   receipt.ownerCreatedAt=new DateTimeOffset(owner.StartTime.ToUniversalTime()).ToUnixTimeMilliseconds();
   receipt.helperCreatedAt=new DateTimeOffset(Process.GetCurrentProcess().StartTime.ToUniversalTime()).ToUnixTimeMilliseconds();
   var ownerHandle=owner.Handle; // Hold identity, not a recycled PID.
   int faulted=0,guardDone=0;long peakPrivateBytes=0;
   using(var guard=new Timer(_=>{
    if(Volatile.Read(ref guardDone)!=0)return;
    string reason=null;long bytes=0;
    try {using(var self=Process.GetCurrentProcess())bytes=self.PrivateMemorySize64;
     if(bytes>Interlocked.Read(ref peakPrivateBytes))Interlocked.Exchange(ref peakPrivateBytes,bytes);
     if(WaitForSingleObject(ownerHandle,0)==0)reason="owner-exited";else if(Now>=request.deadline)reason="deadline";else if(bytes>536870912)reason="private-memory-limit";
    }catch{reason="owner-readback-failed";}
    if(reason==null || Volatile.Read(ref guardDone)!=0 || Interlocked.Exchange(ref faulted,1)!=0)return;
    try{Save(Path.Combine(request.run,"native-fault.json"),new {reason=reason,pid=receipt.helperPid,createdAt=receipt.helperCreatedAt,at=Now,privateBytes=bytes});}catch{}
    // OS closes the non-inheritable job handle and tears down its owned tree.
    Environment.Exit(124);
   },null,0,100)) {
   try {
    var held=Json.Deserialize<Dictionary<string,object>>(File.ReadAllText(request.lockPath));
    if((string)held["token"]!=request.lockToken || Convert.ToInt32(held["pid"])!=parentPid)throw new Exception("Machine-wide ownership differs");
     if(File.Exists(Path.Combine(request.run,"cancel")))throw new OperationCanceledException("Job cancelled before prompt");
     if(request.preflightFailure!=null)throw new Exception(request.preflightFailure);
    receipt.sessionId=Process.GetCurrentProcess().SessionId;
     {
     Save(Path.Combine(request.run,"ownership.json"),receipt);
     Environment.SetEnvironmentVariable("PLAYSRC_LOCAL_JOB_OWNER",Path.Combine(request.run,"ownership.json"));
     Environment.SetEnvironmentVariable("PLAYSRC_LOCAL_JOB_LOCK",request.lockPath);
     Environment.SetEnvironmentVariable("PLAYSRC_LOCAL_JOB_DEADLINE",(request.deadline-7000).ToString());
     Execute(request,receipt,owner);
     }
   } catch(Exception error) {receipt.error=error.Message;receipt.outcome=error is OperationCanceledException?"cancelled":"failed";if(receipt.childPid==0)receipt.treeEmpty=true;}
   finally {
    receipt.teardownAt=Now;
    if(receipt.desktop.Count>0 && receipt.treeEmpty) {
     var stage=receipt.desktop[receipt.desktop.Count-1];
     if(stage.desktopStartedAt!=0 && stage.desktopReleasedAt==0)stage.desktopReleasedAt=receipt.teardownAt;
     var file=Path.Combine(DesktopDirectory(request,receipt.desktop.Count-1),"result.json");
     if(!File.Exists(file))Save(file,stage);
    }
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
    if(!receipt.treeEmpty){receipt.outcome="failed";receipt.error="Owned child teardown unconfirmed; "+receipt.error;}
    receipt.finishedAt=Now;
    // Freeze the receipt before both serializations. A timer tick between the
    // durable save and stdout must not change an otherwise identical outcome.
    receipt.helperPeakPrivateBytes=Interlocked.Read(ref peakPrivateBytes);
    Save(Path.Combine(request.run,"native-result.json"),receipt);
    Interlocked.Exchange(ref guardDone,1);
   }
   }
  }
  Console.WriteLine(Json.Serialize(receipt));
 }
}
