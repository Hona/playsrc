# Deterministic control-flow tests, NOT native UI evidence. This compilation has
# a different receipt schema and cannot authorize any real local-job/profile.
param([Parameter(Mandatory=$true)][string]$Directory)
$ErrorActionPreference='Stop'
$root=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$source=[IO.File]::ReadAllText((Join-Path $root 'windows-job-native.cs'))
$source=$source.Replace('playsrc-native-job-v2','playsrc-native-job-test-only')
$source=$source.Replace('playsrc-native-desktop-v1','playsrc-native-desktop-test-only')
$source=[regex]::Replace($source,'(?s)static int ConsoleSession\(\) \{.*?\n \}', 'static int ConsoleSession() { return TestSession(); }')
$source=[regex]::Replace($source,'(?s)static Dialog Show\(Request request,bool completion,string outcome,Process owner\) \{.*?\n \}', 'static Dialog Show(Request request,bool completion,string outcome,Process owner) { return TestShow(request,completion); }')
$source=[regex]::Replace($source,'(?s)static void Execute\(Request request,Receipt receipt,Process owner\) \{.*?\n \}', 'static void Execute(Request request,Receipt receipt,Process owner) { TestExecute(request,receipt,owner); }')
$source=[regex]::Replace($source,'(?s)static void RequireDesktopEmpty\(IntPtr job\) \{.*?\n  \}', 'static void RequireDesktopEmpty(IntPtr job) { Assert(closed,"teardown before consent closure"); }')
if($source.Contains('TaskDialogIndirect(ref config') -or $source.Contains('Check(CreateProcessW(') -or !$source.Contains('return TestSession();')){throw 'Test isolation substitution failed; refusing UI or workload dispatch'}
$fixture=@'
public static partial class PlaysrcNativeJob {
  static int prompts, completions, sessions, dispatches, browsers;
 static bool closed;
 static string scenario;
 static void Assert(bool value,string message) {if(!value)throw new Exception(scenario+": "+message);}
 static int TestSession() {sessions++;if(scenario=="session-fault")throw new Exception("session-fault");return Process.GetCurrentProcess().SessionId;}
 static Dialog TestShow(Request request,bool completion) {
  if(completion) {Assert(closed && dispatches==1,"completion before prompt closure/dispatch");completions++;}
   else prompts++;
   if(!completion && scenario=="changed-source")File.WriteAllText(Path.Combine(request.dialogDirectory,"prepared.json"),"changed during prompt");
  closed=true;
  return new Dialog{decision=completion?"dismissed-timeout":scenario=="deny"||scenario=="close"||scenario=="escape"||scenario=="race-deny"?"denied":scenario=="display-fault"?"display-failed":scenario=="timeout"||scenario=="race-timeout"?"approved-timeout":"approved",
   error=scenario=="display-fault"?"display-fault":null,displayedAt=Now-3001,decidedAt=Now,dismissedAt=Now,visibleMilliseconds=3001,window=1,sessionId=Process.GetCurrentProcess().SessionId};
 }
  static void TestExecute(Request request,Receipt receipt,Process owner) {
   Assert(prompts==0 && sessions==0,"UI before preparation");
   dispatches++;receipt.commandStartedAt=Now;receipt.childPid=Process.GetCurrentProcess().Id;receipt.childCreatedAt=receipt.helperCreatedAt;
   try {
    Thread.Sleep(20);Assert(prompts==0 && receipt.desktop.Count==0,"preparation acquired desktop");
    if(scenario=="preparation-failure")throw new Exception("preparation failed");
    if(scenario=="preparation-cancel")throw new OperationCanceledException("preparation cancelled");
    if(receipt.interactive) for(int index=0;index<(scenario=="reacquire"?2:1);index++) {
     var directory=DesktopDirectory(request,index);Directory.CreateDirectory(directory);
     var bytes=Encoding.UTF8.GetBytes("{\"testOnly\":true}");File.WriteAllBytes(Path.Combine(directory,"prepared.json"),bytes);
     string prepared;using(var hash=SHA256.Create())prepared=BitConverter.ToString(hash.ComputeHash(bytes)).Replace("-","").ToLowerInvariant();
     var stage=new DesktopRequest{job=request.job,task=request.task,run=request.run,lockToken=request.lockToken,childPid=receipt.childPid,childCreatedAt=receipt.childCreatedAt,helperPid=receipt.helperPid,helperCreatedAt=receipt.helperCreatedAt,stage=Guid.NewGuid().ToString(),preparedIdentity=prepared};
     if(scenario=="stale-request")stage.childCreatedAt--;
     Save(Path.Combine(directory,"request.json"),stage);
     DesktopTransition(request,receipt,owner,IntPtr.Zero);
     var scope=receipt.desktop[index];
     Assert(closed && scope.consent.dismissedAt<=scope.desktopStartedAt,"browser before dismissed consent");
     browsers++;
     if(scenario=="failure")throw new Exception("browser failed");
     if(scenario=="cancel")throw new OperationCanceledException("browser cancelled");
     stage.succeeded=true;Save(Path.Combine(directory,"release.json"),stage);
     DesktopTransition(request,receipt,owner,IntPtr.Zero);
     Assert(scope.desktopReleasedAt>=scope.desktopStartedAt,"artifact work holds desktop");
     Thread.Sleep(20);Assert(prompts==index+1,"authorization reused or repeated");
    }
    receipt.outcome=scenario=="failure"?"failed":scenario=="cancel"?"cancelled":"completed";
   } catch(Exception error) {receipt.error=error.Message;if(receipt.outcome!="denied")receipt.outcome=error is OperationCanceledException?"cancelled":"failed";}
   receipt.exitCode=receipt.outcome=="completed"?0:1;receipt.treeEmpty=true;receipt.teardownAt=Now;
   foreach(var stage in receipt.desktop)if(stage.desktopStartedAt!=0 && stage.desktopReleasedAt==0)stage.desktopReleasedAt=Now;
 }
 public static string TestLifecycle(string directory,string manifest) {
  int cases=0;
   foreach(bool interactive in new[]{false,true}) foreach(string name in new[]{"approve","timeout","deny","close","escape","race-deny","race-timeout","display-fault","session-fault","failure","cancel","preflight","queued-cancel","queue-fault","preparation-failure","preparation-cancel","changed-source","stale-request","reacquire"}) {
    scenario=name;prompts=completions=sessions=dispatches=browsers=0;closed=false;
   var run=Path.Combine(directory,Guid.NewGuid().ToString());Directory.CreateDirectory(run);
   var request=new Request{job="test-only",task="test-only",run=run,action=name,manifest=manifest,invocation=new[]{interactive?"profile":"diagnostic"},command=new[]{"NEVER EXECUTED"},ownerPid=Process.GetCurrentProcess().Id,lockPath=Path.Combine(run,"lock.json"),lockToken="test-only",deadline=Now+15000,preflightFailure=name=="preflight"?"missing content":null};
   Save(request.lockPath,new {pid=request.ownerPid,token=name=="queue-fault"?"mismatch":"test-only"});
   if(name=="queued-cancel")File.WriteAllText(Path.Combine(run,"cancel"),"test cancellation");
   string file=Path.Combine(run,"request.json");Save(file,request);
   var output=new StringWriter();var previousOut=Console.Out;var previousIn=Console.In;
   try {Console.SetOut(output);Console.SetIn(new StringReader("{\"error\":null}\n"));Run(Json.Serialize(new {request=request,interactive=interactive}),request.ownerPid);}
   finally {Console.SetOut(previousOut);Console.SetIn(previousIn);}
   var receipt=Json.Deserialize<Receipt>(File.ReadAllText(Path.Combine(run,"native-result.json")));
   Assert(receipt.schema=="playsrc-native-job-test-only","test schema isolation");
   bool preflight=name=="preflight"||name=="queued-cancel"||name=="queue-fault";
    bool prepared=!preflight && name!="preparation-failure" && name!="preparation-cancel";
    bool rejected=!prepared || interactive && (name=="session-fault"||name=="display-fault"||name=="deny"||name=="close"||name=="escape"||name=="race-deny"||name=="changed-source"||name=="stale-request");
    Assert(dispatches==(preflight?0:1),"preparation dispatch count");
    int count=name=="reacquire"?2:1;
    Assert(prompts==(!interactive||!prepared||name=="session-fault"||name=="stale-request"?0:count),"prompt count");
    Assert(browsers==(interactive&&!rejected?count:0),"browser dispatch count");
   Assert(completions==(interactive&&!rejected&&name!="failure"&&name!="cancel"?count:0),"completion count");
   Assert(receipt.uiInvocations==prompts+completions,"recorded UI count");
   Assert(interactive || sessions==0 && receipt.desktop.Count==0 && receipt.uiInvocations==0,"background acquired desktop/UI");
   Assert(receipt.treeEmpty && receipt.teardownAt>=receipt.commandStartedAt,"teardown ordering");
    foreach(var stage in receipt.desktop)Assert(stage.completion==null || stage.completion.dismissedAt>=stage.desktopReleasedAt,"completion before desktop teardown");
   cases++;
  }
  return Json.Serialize(new {cases=cases,backgroundUiInvocations=0,testOnly=true});
 }
}
'@
Add-Type -TypeDefinition ($source+"`n"+$fixture) -ReferencedAssemblies System.Web.Extensions,System.Drawing
[PlaysrcNativeJob]::TestLifecycle($Directory,(Join-Path $root 'windows-job-native.manifest'))
