# Deterministic control-flow tests, NOT native UI evidence. This compilation has
# a different receipt schema and cannot authorize any real local-job/profile.
param([Parameter(Mandatory=$true)][string]$Directory)
$ErrorActionPreference='Stop'
$root=(Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$source=[IO.File]::ReadAllText((Join-Path $root 'windows-job-native.cs'))
$source=$source.Replace('playsrc-native-job-v1','playsrc-native-job-test-only')
$source=[regex]::Replace($source,'(?s)static int ConsoleSession\(\) \{.*?\n  \}', 'static int ConsoleSession() { return TestSession(); }')
$source=[regex]::Replace($source,'(?s)static Dialog Show\(Request request,bool completion,string outcome,Process owner\) \{.*?\n  \}', 'static Dialog Show(Request request,bool completion,string outcome,Process owner) { return TestShow(request,completion); }')
$source=[regex]::Replace($source,'(?s)static void Execute\(Request request,Receipt receipt,Process owner\) \{.*?\n  \}', 'static void Execute(Request request,Receipt receipt,Process owner) { TestExecute(request,receipt); }')
$fixture=@'
public static partial class PlaysrcNativeJob {
 static int prompts, completions, sessions, dispatches;
 static bool closed;
 static string scenario;
 static void Assert(bool value,string message) {if(!value)throw new Exception(scenario+": "+message);}
 static int TestSession() {sessions++;if(scenario=="session-fault")throw new Exception("session-fault");return Process.GetCurrentProcess().SessionId;}
 static Dialog TestShow(Request request,bool completion) {
  if(completion) {Assert(closed && dispatches==1,"completion before prompt closure/dispatch");completions++;}
  else prompts++;
  closed=true;
  return new Dialog{decision=completion?"dismissed-timeout":scenario=="deny"||scenario=="close"||scenario=="escape"||scenario=="race-deny"?"denied":scenario=="display-fault"?"display-failed":scenario=="timeout"||scenario=="race-timeout"?"approved-timeout":"approved",
   error=scenario=="display-fault"?"display-fault":null,displayedAt=Now-3001,decidedAt=Now,dismissedAt=Now,visibleMilliseconds=3001,window=1,sessionId=Process.GetCurrentProcess().SessionId};
 }
 static void TestExecute(Request request,Receipt receipt) {
  Assert(!receipt.interactive || closed,"dispatch before prompt closure");
  Assert(!receipt.interactive || receipt.consent.dismissedAt<=Now,"dispatch before dismissal");
  dispatches++;receipt.commandStartedAt=Now;receipt.childPid=123;receipt.childCreatedAt=Now;
  receipt.outcome=scenario=="failure"?"failed":scenario=="cancel"?"cancelled":"completed";
  receipt.exitCode=receipt.outcome=="completed"?0:1;receipt.treeEmpty=true;receipt.teardownAt=Now;
 }
 public static string TestLifecycle(string directory,string manifest) {
  int cases=0;
  foreach(bool interactive in new[]{false,true}) foreach(string name in new[]{"approve","timeout","deny","close","escape","race-deny","race-timeout","display-fault","session-fault","failure","cancel","preflight","queued-cancel","queue-fault"}) {
   scenario=name;prompts=completions=sessions=dispatches=0;closed=false;
   var run=Path.Combine(directory,Guid.NewGuid().ToString());Directory.CreateDirectory(run);
   var request=new Request{job="test-only",task="test-only",run=run,action=name,manifest=manifest,invocation=new[]{interactive?"profile":"diagnostic"},command=new[]{"NEVER EXECUTED"},ownerPid=Process.GetCurrentProcess().Id,lockPath=Path.Combine(run,"lock.json"),lockToken="test-only",deadline=Now+15000,preflightFailure=name=="preflight"?"missing content":null};
   Save(request.lockPath,new {pid=request.ownerPid,token=name=="queue-fault"?"mismatch":"test-only"});
   if(name=="queued-cancel")File.WriteAllText(Path.Combine(run,"cancel"),"test cancellation");
   string file=Path.Combine(run,"request.json");Save(file,request);
   var output=new StringWriter();var previousOut=Console.Out;var previousIn=Console.In;
   try {Console.SetOut(output);Console.SetIn(new StringReader("{\"error\":null}\n"));Run(file,request.ownerPid,interactive);}
   finally {Console.SetOut(previousOut);Console.SetIn(previousIn);}
   var receipt=Json.Deserialize<Receipt>(File.ReadAllText(Path.Combine(run,"native-result.json")));
   Assert(receipt.schema=="playsrc-native-job-test-only","test schema isolation");
   bool preflight=name=="preflight"||name=="queued-cancel"||name=="queue-fault";
   bool rejected=preflight || interactive && (name=="session-fault"||name=="display-fault"||name=="deny"||name=="close"||name=="escape"||name=="race-deny");
   Assert(dispatches==(rejected?0:1),"dispatch count");
   Assert(prompts==(!interactive||preflight||name=="session-fault"?0:1),"prompt count");
   Assert(completions==(interactive&&!rejected&&name!="failure"&&name!="cancel"?1:0),"completion count");
   Assert(receipt.uiInvocations==prompts+completions,"recorded UI count");
   Assert(interactive || sessions==0 && receipt.consent==null && receipt.completion==null && receipt.uiInvocations==0,"background acquired desktop/UI");
   Assert(receipt.treeEmpty && receipt.teardownAt>=receipt.commandStartedAt,"teardown ordering");
   Assert(receipt.completion==null || receipt.completion.dismissedAt>=receipt.teardownAt,"completion before teardown");
   cases++;
  }
  return Json.Serialize(new {cases=cases,backgroundUiInvocations=0,testOnly=true});
 }
}
'@
Add-Type -TypeDefinition ($source+"`n"+$fixture) -ReferencedAssemblies System.Web.Extensions,System.Drawing
[PlaysrcNativeJob]::TestLifecycle($Directory,(Join-Path $root 'windows-job-native.manifest'))
