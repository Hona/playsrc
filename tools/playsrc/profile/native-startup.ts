import { spawn } from "node:child_process"
import type { Page } from "@playwright/test"
import { WINDOWS_DESKTOP_QUERY, assertWindowsConsole } from "./windows-desktop"
import { macWindowReader } from "./macos-visible-windows"
import { macPageAdmission, requireMacPageAdmission } from "./macos-page-admission"
import type { StartupNativeAdmission } from "./static-startup-gate"
import os from "node:os"
import path from "node:path"
import { mkdirSync, writeFileSync, readFileSync } from "node:fs"
import { createHash } from "node:crypto"
import { WINDOWS_OWNED_UI, WINDOWS_LOCAL_PERMISSION, ownedDiagnosticWindow, assertOwnedEphemeralBrowser } from "./windows-owned-ui"
import { mkdir, writeFile } from "node:fs/promises"
import { randomUUID } from "node:crypto"

export class NativeStartupProbeError extends Error {
  constructor(message: string, readonly receipt: unknown) { super(message) }
}

export function nativeProbeResponse(result: any): any {
  if (result.error) throw new NativeStartupProbeError(String(result.error), result.receipt ?? null)
  return result
}

export type NativeDesktopPixels = Readonly<{ path: string; bounds: Readonly<{ X: number; Y: number; Width: number; Height: number }>; startedEpoch: number; endedEpoch: number }>

export function requireNativeDesktopPixels(value: NativeDesktopPixels | undefined, expectedPath: string): NativeDesktopPixels {
  if (!value || value.path !== expectedPath || !value.bounds
    || ![value.startedEpoch, value.endedEpoch, value.bounds.X, value.bounds.Y, value.bounds.Width, value.bounds.Height].every(Number.isSafeInteger)
    || value.startedEpoch < 0 || value.endedEpoch < value.startedEpoch || value.endedEpoch - value.startedEpoch > 5000
    || value.bounds.Width <= 0 || value.bounds.Height <= 0 || value.bounds.Width * value.bounds.Height > 33554432) throw new Error("Native desktop pixel receipt is invalid")
  return value
}

export const WINDOWS_INPUT = String.raw`
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class StartupWindow {
 [StructLayout(LayoutKind.Sequential)] public struct Rect {public int Left,Top,Right,Bottom;}
 [StructLayout(LayoutKind.Sequential)] struct Input {public uint Size,Time;}
 [DllImport("user32.dll")] static extern bool GetLastInputInfo(ref Input input);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
 [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr window);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window,out uint pid);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window,out Rect rect);
  [DllImport("user32.dll")] public static extern int GetSystemMetrics(int index);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetClassNameW(IntPtr window,StringBuilder text,int count);
  [DllImport("user32.dll")] static extern IntPtr GetWindow(IntPtr window,uint command);
  [DllImport("user32.dll")] static extern IntPtr GetAncestor(IntPtr window,uint flags);
  public static string WindowClass(IntPtr window) {var text=new StringBuilder(256);return GetClassNameW(window,text,text.Capacity)>0?text.ToString():null;}
 [DllImport("user32.dll")] static extern bool EnumWindows(Callback callback,IntPtr data);
 delegate bool Callback(IntPtr window,IntPtr data);
 public static uint Idle() {var input=new Input{Size=8};if(!GetLastInputInfo(ref input))throw new Exception("Last-input readback unavailable");return unchecked((uint)Environment.TickCount-input.Time);}
  public static IntPtr[] Windows(uint pid) {var result=new System.Collections.Generic.List<IntPtr>();EnumWindows((window,data)=>{uint owner;GetWindowThreadProcessId(window,out owner);if(owner==pid&&IsWindowVisible(window))result.Add(window);return true;},IntPtr.Zero);return result.ToArray();}
  public static object Owner(IntPtr window) {
   uint pid,ownerPid,rootPid;GetWindowThreadProcessId(window,out pid);
   var owner=GetWindow(window,4);var root=GetAncestor(window,3);
   GetWindowThreadProcessId(owner,out ownerPid);GetWindowThreadProcessId(root,out rootPid);
   var name=new StringBuilder(256);GetClassNameW(window,name,name.Capacity);
   string processName=null;try{processName=System.Diagnostics.Process.GetProcessById((int)pid).ProcessName;}catch{}
   return new {windowId=window.ToInt64(),processId=pid,processName=processName,windowClass=name.ToString(),ownerWindowId=owner.ToInt64(),ownerProcessId=ownerPid,rootOwnerWindowId=root.ToInt64(),rootOwnerProcessId=rootPid};
  }
}
'@
`

let windowsProbe: {read(pid:number,capture?:string,captureWindow?:{left:number;top:number;width:number;height:number},expectedWindow?:number,ownedDiagnostic?:boolean,permissionOrigin?:string):Promise<any>;close():void}|undefined

function openWindowsProbe(cacheDir: string) {
  const script = "[Console]::OutputEncoding=New-Object System.Text.UTF8Encoding($false);$ProgressPreference='SilentlyContinue';"+WINDOWS_DESKTOP_QUERY+WINDOWS_INPUT+WINDOWS_OWNED_UI+WINDOWS_LOCAL_PERMISSION+String.raw`
Add-Type -AssemblyName System.Drawing
function ForegroundReceipt {
 $begin=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
 $handle=[StartupWindow]::GetForegroundWindow();[uint32]$owner=0
 $thread=[StartupWindow]::GetWindowThreadProcessId($handle,[ref]$owner)
 $class=[StartupWindow]::WindowClass($handle);$idle=[StartupWindow]::Idle()
 $name=$null;$born=$null;$ownerFault=$null
 if($owner -ne 0) {try {$process=[System.Diagnostics.Process]::GetProcessById([int]$owner);$name=$process.ProcessName;$born=([DateTimeOffset]$process.StartTime.ToUniversalTime()).ToUnixTimeMilliseconds();$process.Dispose()} catch {$ownerFault=$_.Exception.Message}}
 @{startedEpoch=$begin;endedEpoch=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds();handle=$handle.ToInt64();ownerPid=$owner;threadId=$thread;windowClass=$class;idleMilliseconds=$idle;processName=$name;processStartedEpoch=$born;ownerFault=$ownerFault}
}
$self=[System.Diagnostics.Process]::GetCurrentProcess()
$helper=@{pid=$self.Id;sessionId=$self.SessionId;startedEpoch=([DateTimeOffset]$self.StartTime.ToUniversalTime()).ToUnixTimeMilliseconds()}
while (($line=[Console]::ReadLine()) -ne $null) {
 $request=$null;$before=$null;$after=$null;$start=$null;$end=$null;$desktop=$null;$windows=@()
 $received=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
 try {
  $request=$line|ConvertFrom-Json
  $session=[ProfileConsole]::WTSGetActiveConsoleSessionId();$info=[ProfileConsole]::Query($session)
  $desktop=@{consoleSessionId=$session;processSessionId=[System.Diagnostics.Process]::GetCurrentProcess().SessionId;level=$info.Level;sessionId=$info.SessionId;state=$info.State;flags=$info.Flags;protocol=[ProfileConsole]::Protocol($session)}
  $windows=@(foreach($hwnd in [StartupWindow]::Windows([uint32]$request.pid)) {$rect=New-Object StartupWindow+Rect;if(-not [StartupWindow]::GetWindowRect($hwnd,[ref]$rect)){throw 'Window bounds unavailable'};@{id=$hwnd.ToInt64();bounds=$rect;visible=[StartupWindow]::IsWindowVisible($hwnd);minimized=[StartupWindow]::IsIconic($hwnd)}})
   $before=ForegroundReceipt;$foreground=$before.handle
   $foregroundEpoch=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
   $foregroundOwner=$null;$foregroundParent=$null
   if ($request.pid -gt 0) {
    $foregroundOwner=[StartupWindow]::Owner([IntPtr]$foreground)
    if (($request.expectedWindow -and $foreground -ne $request.expectedWindow) -or $foregroundOwner.processId -ne $request.pid) {
     try {$parent=Get-CimInstance Win32_Process -Filter "ProcessId=$($foregroundOwner.processId)" -OperationTimeoutSec 1; $foregroundParent=@{processId=$foregroundOwner.processId;parentProcessId=$parent.ParentProcessId}} catch {$foregroundParent=@{error='foreground process parent unavailable'}}
    }
   }
   $pixels=$null
   if ($request.capture) {
    if ($info.State -ne 0 -or $info.Flags -ne 1 -or $desktop.protocol -ne 0 -or $session -ne $desktop.processSessionId) {throw 'Native pixel console is not active and unlocked'}
    if (@($windows | Where-Object {$_.id -eq $foreground -and $_.visible -and !$_.minimized}).Count -ne 1) {throw 'Native pixel target is not foreground'}
    $x=[StartupWindow]::GetSystemMetrics(76);$y=[StartupWindow]::GetSystemMetrics(77);$w=[StartupWindow]::GetSystemMetrics(78);$h=[StartupWindow]::GetSystemMetrics(79)
    if ($request.captureWindow) {
     $expected=$request.captureWindow
     $matches=@($windows | Where-Object {$_.bounds.Left -eq $expected.left -and $_.bounds.Top -eq $expected.top -and ($_.bounds.Right-$_.bounds.Left) -eq $expected.width -and ($_.bounds.Bottom-$_.bounds.Top) -eq $expected.height})
     if ($matches.Count -ne 1) {throw 'Pixel scope is not the complete measured window'}
     $ownedScope=$request.ownedDiagnostic -and $foregroundOwner.processId -eq $request.pid -and $foregroundOwner.rootOwnerWindowId -eq $matches[0].id
     if ($matches[0].id -ne $foreground -and !$ownedScope) {throw 'Pixel scope is not the complete foreground measured window'}
     $x=$matches[0].bounds.Left;$y=$matches[0].bounds.Top;$w=$matches[0].bounds.Right-$x;$h=$matches[0].bounds.Bottom-$y
    }
    if ($w -le 0 -or $h -le 0 -or $w*$h -gt 33554432) {throw 'Native pixel desktop bounds invalid'}
    $start=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    $bitmap=New-Object System.Drawing.Bitmap($w,$h,[System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $graphics=[System.Drawing.Graphics]::FromImage($bitmap)
    try {$graphics.CopyFromScreen($x,$y,0,0,$bitmap.Size);$bitmap.Save([string]$request.capture,[System.Drawing.Imaging.ImageFormat]::Png)} finally {$graphics.Dispose();$bitmap.Dispose()}
    $end=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
    $after=ForegroundReceipt
    if ($after.handle -ne $foreground) {throw 'Native foreground changed during pixel capture'}
    $pixels=@{path=[string]$request.capture;bounds=@{X=$x;Y=$y;Width=$w;Height=$h};startedEpoch=$start;endedEpoch=$end}
   }
   $ui=$null;$action=$null
   if ($request.ownedDiagnostic) {
    if (!$request.captureWindow -or !$request.capture -or $foregroundOwner.processId -ne $request.pid) {throw 'Owned UI diagnosis requires the linked private capture'}
    $ui=Read-OwnedUI $foreground ([uint32]$request.pid)
     if ($request.permissionOrigin) {
      if ([StartupWindow]::Idle() -lt 2000) {throw 'Owned permission requires genuine two-second idle admission'}
      $action=Allow-OwnedLocalPermission $ui ([string]$request.permissionOrigin) ([uint32]$request.pid) $foreground
     }
   }
   $idle=[StartupWindow]::Idle();$idleEpoch=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
   if($null -eq $after){$after=ForegroundReceipt}
   $result=@{id=$request.id;desktop=$desktop;idleMilliseconds=$idle;idleEpoch=$idleEpoch;foreground=$foreground;foregroundEpoch=$foregroundEpoch;foregroundAfter=[StartupWindow]::GetForegroundWindow().ToInt64();foregroundOwner=$foregroundOwner;foregroundParent=$foregroundParent;probePid=$PID;windows=$windows;pixels=$pixels;ownedUI=$ui;action=$action}
 } catch {
   $failure=$_.Exception.Message
   if($null -eq $after){try {$after=ForegroundReceipt} catch {}}
   $result=@{id=$request.id;error=$failure}
 }
 $result.receipt=@{schema='playsrc-native-capture-receipt-v1';privacy='private-native-owner';receivedEpoch=$received;finishedEpoch=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds();sequence=$request.id;browserPid=$request.pid;helper=$helper;before=$before;after=$after;windows=$windows;captureStartedEpoch=$start;captureEndedEpoch=$end;desktop=$desktop}
 [Console]::WriteLine(($result|ConvertTo-Json -Depth 12 -Compress))
}
`
  // Win32 command lines are bounded. Keep this owned helper in configured
  // cache storage, not a growing base64 command or a security-policy override.
  const scriptPath = path.join(cacheDir, "profile-tools", `native-startup-${createHash("sha256").update(script).digest("hex")}.ps1`)
  const scriptBytes = "\uFEFF" + script
  mkdirSync(path.dirname(scriptPath), { recursive: true })
  try { writeFileSync(scriptPath, scriptBytes, { encoding: "utf8", flag: "wx" }) }
  catch (error) { if ((error as NodeJS.ErrnoException).code !== "EEXIST" || readFileSync(scriptPath, "utf8") !== scriptBytes) throw error }
  const helperSpawnedEpoch=Date.now()
  const child=spawn("powershell.exe",["-NoProfile","-NonInteractive","-File",scriptPath],{windowsHide:true,stdio:["pipe","pipe","pipe"]})
  let next=0,text="",diagnostics=""
  const pending=new Map<number,{resolve:(value:any)=>void;reject:(error:Error)=>void;timer:ReturnType<typeof setTimeout>}>()
  child.stderr.on("data",value=>{diagnostics=(diagnostics+value).slice(-4096)})
  const fail=(error:Error)=>{for(const value of pending.values()){clearTimeout(value.timer);value.reject(error)}pending.clear()}
  child.on("error",fail);child.on("exit",()=>fail(new Error(`Native startup probe exited: ${diagnostics}`)))
  child.stdout.on("data",value=>{
    text+=value
    while(text.includes("\n")){
      const end=text.indexOf("\n"),line=text.slice(0,end).trim();text=text.slice(end+1)
      if(!line)continue
      let result:any
      try{result=JSON.parse(line)}catch{fail(new Error(`Malformed native startup probe response: ${line.slice(0,1024)}; stderr=${diagnostics}`));continue}
      if(!result.id)continue // initial read-only WTS readiness record
      const entry=pending.get(result.id);if(!entry)continue
      pending.delete(result.id);clearTimeout(entry.timer)
      result.receipt={...result.receipt,controller:{pid:process.pid,parentPid:process.ppid,helperPid:child.pid,helperSpawnedEpoch},receivedByControllerEpoch:Date.now()}
      try {entry.resolve(nativeProbeResponse(result))} catch(error){entry.reject(error as Error)}
    }
  })
  return {read:(pid:number,capture?:string,captureWindow?:{left:number;top:number;width:number;height:number},expectedWindow?:number,ownedDiagnostic?:boolean,permissionOrigin?:string)=>new Promise<any>((resolve,reject)=>{const id=++next;const timer=setTimeout(()=>{pending.delete(id);reject(new Error("Native startup readback exceeded ten seconds"))},10_000);pending.set(id,{resolve,reject,timer});child.stdin.write(JSON.stringify({id,pid,capture,captureWindow,expectedWindow,ownedDiagnostic,permissionOrigin})+"\n")}),close(){child.stdin.end();child.kill();fail(new Error("Native startup probe closed"))}}
}

export function closeStartupNativeProbe(){windowsProbe?.close();windowsProbe=undefined}

export function windowsForegroundMatches(native: { foreground: number; foregroundAfter: number }, windowId: number, focused: boolean): boolean {
  return native.foreground === windowId && native.foregroundAfter === windowId && focused
}

async function windowsSnapshot(cacheDir: string, browserPid = 0, capture?: string, captureWindow?:{left:number;top:number;width:number;height:number}, expectedWindow?: number, ownedDiagnostic?: boolean, permissionOrigin?: string) {
  let result
  try {result=await (windowsProbe??=openWindowsProbe(cacheDir)).read(browserPid,capture,captureWindow,expectedWindow,ownedDiagnostic,permissionOrigin)} catch(error) {
    if(error instanceof NativeStartupProbeError) {
      const directory=path.join(cacheDir,"evidence","tf2-browser-performance","native-startup-failures")
      await mkdir(directory,{recursive:true})
      const file=path.join(directory,`${randomUUID()}.json`)
      await writeFile(file,JSON.stringify({error:error.message,receipt:error.receipt},null,2),{flag:"wx"})
      error.message+=`; private receipt=${file}`
    }
    throw error
  }
  assertWindowsConsole(result.desktop,os.release())
  return result
}

export async function startupConsoleIdle(cacheDir: string) {
  if (process.platform === "win32") return (await windowsSnapshot(cacheDir)).idleMilliseconds as number
  const reader = await macWindowReader(cacheDir)
  if (!reader) throw new Error("Static startup requires a supported physical desktop")
  const snapshot = await reader(), console = snapshot.console
  if (!console?.onConsole || !console.loginDone || console.locked) throw new Error("Static startup physical console is unavailable or locked")
  return console.idleMilliseconds
}

export async function startupNativeReader(page: Page, cacheDir: string) {
  const mac = await macPageAdmission(page, cacheDir)
  const pageCdp = await page.context().newCDPSession(page)
  const browserCdp = await page.context().browser()!.newBrowserCDPSession()
  const processes = (await browserCdp.send("SystemInfo.getProcessInfo")).processInfo.filter(p => p.type === "browser")
  if (processes.length !== 1) throw new Error("Static startup browser owner is ambiguous")
  const browserPid = processes[0]!.id
  let established: number | undefined
  let establishedBounds: string | undefined
  const records: any[] = []
  return {
    records,
    async diagnoseOwnedWindow(capture: string) {
      if (process.platform !== "win32") throw new Error("Owned UI diagnosis requires Windows")
      const { targetInfo } = await pageCdp.send("Target.getTargetInfo")
      const before = await browserCdp.send("Browser.getWindowForTarget", { targetId: targetInfo.targetId })
      const native = await windowsSnapshot(cacheDir, browserPid, capture, before.bounds as any, established, true)
      const after = await browserCdp.send("Browser.getWindowForTarget", { targetId: targetInfo.targetId })
      const record = { at: Date.now(), purpose: "private-owned-ui-diagnosis-not-admission", browserPid, targetId: targetInfo.targetId, before, after, native }
      records.push(record)
      if (JSON.stringify(before) !== JSON.stringify(after)) throw new Error("Measured window linkage changed during owned UI diagnosis")
      const mainWindowId = ownedDiagnosticWindow(native, before.bounds as any, browserPid)
      requireNativeDesktopPixels(native.pixels, capture)
      return { ...record, mainWindowId }
    },
    async allowOwnedLocalPermission(captureBefore: string) {
      if (process.platform !== "win32") throw new Error("Owned permission action requires Windows")
      // The launcher owns this receipt. Do not add automation/security flags
      // merely to enable a CDP command that ordinary Chrome may reject.
      const command = JSON.parse(readFileSync(path.join(cacheDir, "evidence/tf2-browser-performance/headed-browser.json"), "utf8"))
      assertOwnedEphemeralBrowser(command.arguments, command.browserPid, browserPid)
      const origin = new URL(page.url()).origin
      if (!/^http:\/\/127\.0\.0\.1:\d+$/u.test(origin)) throw new Error("Permission action is not for the local application")
      const { targetInfo } = await pageCdp.send("Target.getTargetInfo")
      const facts = await browserCdp.send("Browser.getWindowForTarget", { targetId: targetInfo.targetId })
      // Read-only identity checks happen before the explicit action request.
      const before = await windowsSnapshot(cacheDir, browserPid)
      if (before.idleMilliseconds < 2000) throw new Error("Owned permission requires genuine two-second idle admission")
      const mainWindowId = ownedDiagnosticWindow(before, facts.bounds as any, browserPid)
      if (before.foreground === mainWindowId) throw new Error("No owned permission window to resolve")
      const native = await windowsSnapshot(cacheDir, browserPid, captureBefore, facts.bounds as any, mainWindowId, true, origin)
      const record = { at: Date.now(), purpose: "normal-owned-loopback-permission-control", browserPid, targetId: targetInfo.targetId, facts, before, native, ephemeralProfile: command.arguments.find((value: string) => value.startsWith("--user-data-dir=")) }
      records.push(record)
      if (native.action?.action !== "normal-visible-Allow") throw new Error("Permission control was not invoked")
      return record
    },
    async read(desktopScreenshot?: string, pixelScope: "desktop" | "window" = "desktop"): Promise<StartupNativeAdmission & { pixels?: NativeDesktopPixels }> {
      if (mac) {
        const record = await mac.read(desktopScreenshot); records.push(record); requireMacPageAdmission(record)
        const bounds=JSON.stringify(record.page!.bounds)
        if(establishedBounds!==undefined&&establishedBounds!==bounds)throw new Error("Static startup native window geometry changed")
        establishedBounds=bounds
        const console = record.snapshot?.console
        if (!console?.onConsole || !console.loginDone || console.locked) throw new Error("Static startup console changed")
        return { at: record.at, physical: true, unlocked: true, foreground: !record.occluders?.length,
          visible: record.document?.visibility !== "hidden", minimized: false, idleMilliseconds: console.idleMilliseconds,
          browserPid, windowId: record.linkage!.nativeWindowId, targetId: record.linkage!.targetId }
      }
      if (process.platform !== "win32") throw new Error("Static startup native window reader is unavailable")
      const { targetInfo } = await pageCdp.send("Target.getTargetInfo")
      const facts = await browserCdp.send("Browser.getWindowForTarget", { targetId: targetInfo.targetId })
      const boundsIdentity=JSON.stringify(facts.bounds)
      if(establishedBounds!==undefined&&establishedBounds!==boundsIdentity)throw new Error("Static startup native window geometry changed")
      establishedBounds=boundsIdentity
       let native
       try {native = await windowsSnapshot(cacheDir,browserPid,desktopScreenshot,pixelScope === "window" && desktopScreenshot ? facts.bounds as {left:number;top:number;width:number;height:number} : undefined,established)} catch(error) {
         if(error instanceof NativeStartupProbeError) records.push({at:Date.now(),facts,targetId:targetInfo.targetId,failure:error.receipt})
         throw error
       }
       if (desktopScreenshot) requireNativeDesktopPixels(native.pixels, desktopScreenshot)
      const matches = native.windows.filter((w: any) => w.bounds.Left === facts.bounds.left && w.bounds.Top === facts.bounds.top
        && w.bounds.Right - w.bounds.Left === facts.bounds.width && w.bounds.Bottom - w.bounds.Top === facts.bounds.height)
       const record = { at: Date.now(), facts, native, targetId: targetInfo.targetId, documentState: null as { visible: boolean; focused: boolean } | null }
       records.push(record)
      if (matches.length !== 1 || established !== undefined && established !== matches[0].id) throw new Error("Static startup page/native window linkage changed or is ambiguous")
      const window = matches[0]; established = window.id
      const documentState = await page.evaluate(() => ({ visible: document.visibilityState === "visible", focused: document.hasFocus() }))
      record.documentState = documentState
      return { at: Date.now(), physical: true, unlocked: true, foreground: windowsForegroundMatches(native, window.id, documentState.focused),
        visible: window.visible && documentState.visible, minimized: window.minimized, idleMilliseconds: native.idleMilliseconds,
         browserPid, windowId: window.id, targetId: targetInfo.targetId, ...(native.pixels ? { pixels: native.pixels } : {}) }
    },
    async close() { closeStartupNativeProbe();await Promise.all([mac?.close(), pageCdp.detach(), browserCdp.detach()]) },
  }
}

/** Remote native launchers keep their machine's checked lock and physical
 * console process. This narrow read-only broker joins the measured CDP Page to
 * that process; it never launches, focuses or admits another browser window. */
export async function externalStartupNativeReader(page: Page, endpoint: string, lockToken: string) {
  if (!/^http:\/\/127\.0\.0\.1:\d+$/.test(endpoint) || !lockToken) throw new Error("External native startup broker is malformed")
  const pageCdp=await page.context().newCDPSession(page),browserCdp=await page.context().browser()!.newBrowserCDPSession()
  const processes=(await browserCdp.send("SystemInfo.getProcessInfo")).processInfo.filter(p=>p.type==="browser")
  if(processes.length!==1)throw new Error("External startup browser owner is ambiguous")
  const browserPid=processes[0]!.id,records:any[]=[]
  let sequence=0
  let establishedBounds:string|undefined
  const facts=async()=>{const {targetInfo}=await pageCdp.send("Target.getTargetInfo");return {targetId:targetInfo.targetId,...await browserCdp.send("Browser.getWindowForTarget",{targetId:targetInfo.targetId})}}
  return {
    records,
    async read(_desktopScreenshot?:string):Promise<StartupNativeAdmission> {
      const before=await facts()
      const bounds=JSON.stringify(before.bounds)
      if(establishedBounds!==undefined&&establishedBounds!==bounds)throw new Error("Static startup external window geometry changed")
      establishedBounds=bounds
      const request={sequence:++sequence,browserPid,...before,lockToken}
      const response=await fetch(`${endpoint}/snapshot`,{method:"POST",headers:{"content-type":"application/json"},body:JSON.stringify(request),signal:AbortSignal.timeout(12_000)})
      if(!response.ok)throw new Error(`External native startup broker rejected readback: ${await response.text()}`)
      const proof=await response.json(),after=await facts()
      records.push({before,after,proof})
      const admission=proof.admission as StartupNativeAdmission
      if(JSON.stringify(before)!==JSON.stringify(after)||proof.lockToken!==lockToken||proof.lockAlive!==true
        ||admission?.browserPid!==browserPid||admission.targetId!==before.targetId||Date.now()-admission.at>15_000)throw new Error("External startup native ownership/readback changed")
      const documentState=await page.evaluate(()=>({visible:document.visibilityState==="visible",focused:document.hasFocus()}))
      return {...admission,visible:admission.visible&&documentState.visible,foreground:admission.foreground&&documentState.focused}
    },
    async close(){await Promise.all([pageCdp.detach(),browserCdp.detach()])},
  }
}
