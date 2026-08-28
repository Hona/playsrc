import { spawn } from "node:child_process"
import type { Page } from "@playwright/test"
import { WINDOWS_DESKTOP_QUERY, assertWindowsConsole } from "./windows-desktop"
import { macWindowReader } from "./macos-visible-windows"
import { macPageAdmission, requireMacPageAdmission } from "./macos-page-admission"
import type { StartupNativeAdmission } from "./static-startup-gate"
import os from "node:os"

const WINDOWS_INPUT = String.raw`
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class StartupWindow {
 [StructLayout(LayoutKind.Sequential)] public struct Rect {public int Left,Top,Right,Bottom;}
 [StructLayout(LayoutKind.Sequential)] struct Input {public uint Size,Time;}
 [DllImport("user32.dll")] static extern bool GetLastInputInfo(ref Input input);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
 [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr window);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window,out uint pid);
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window,out Rect rect);
 [DllImport("user32.dll")] static extern bool EnumWindows(Callback callback,IntPtr data);
 delegate bool Callback(IntPtr window,IntPtr data);
 public static uint Idle() {var input=new Input{Size=8};if(!GetLastInputInfo(ref input))throw new Exception("Last-input readback unavailable");return unchecked((uint)Environment.TickCount-input.Time);}
 public static IntPtr[] Windows(uint pid) {var result=new System.Collections.Generic.List<IntPtr>();EnumWindows((window,data)=>{uint owner;GetWindowThreadProcessId(window,out owner);if(owner==pid&&IsWindowVisible(window))result.Add(window);return true;},IntPtr.Zero);return result.ToArray();}
}
'@
`

let windowsProbe: {read(pid:number):Promise<any>;close():void}|undefined

function openWindowsProbe() {
  const script = "$ProgressPreference='SilentlyContinue';"+WINDOWS_DESKTOP_QUERY+WINDOWS_INPUT+String.raw`
while (($line=[Console]::ReadLine()) -ne $null) {
 try {
  $request=$line|ConvertFrom-Json
  $session=[ProfileConsole]::WTSGetActiveConsoleSessionId();$info=[ProfileConsole]::Query($session)
  $desktop=@{consoleSessionId=$session;processSessionId=[System.Diagnostics.Process]::GetCurrentProcess().SessionId;level=$info.Level;sessionId=$info.SessionId;state=$info.State;flags=$info.Flags;protocol=[ProfileConsole]::Protocol($session)}
  $windows=@(foreach($hwnd in [StartupWindow]::Windows([uint32]$request.pid)) {$rect=New-Object StartupWindow+Rect;if(-not [StartupWindow]::GetWindowRect($hwnd,[ref]$rect)){throw 'Window bounds unavailable'};@{id=$hwnd.ToInt64();bounds=$rect;visible=[StartupWindow]::IsWindowVisible($hwnd);minimized=[StartupWindow]::IsIconic($hwnd)}})
  $result=@{id=$request.id;desktop=$desktop;idleMilliseconds=[StartupWindow]::Idle();foreground=[StartupWindow]::GetForegroundWindow().ToInt64();windows=$windows}
 } catch {$result=@{id=$request.id;error=($_|Out-String)}}
 [Console]::WriteLine(($result|ConvertTo-Json -Depth 6 -Compress))
}
`
  const child=spawn("powershell.exe",["-NoProfile","-NonInteractive","-EncodedCommand",Buffer.from(script,"utf16le").toString("base64")],{windowsHide:true,stdio:["pipe","pipe","pipe"]})
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
      try{result=JSON.parse(line)}catch{fail(new Error("Malformed native startup probe response"));continue}
      if(!result.id)continue // initial read-only WTS readiness record
      const entry=pending.get(result.id);if(!entry)continue
      pending.delete(result.id);clearTimeout(entry.timer)
      result.error?entry.reject(new Error(result.error)):entry.resolve(result)
    }
  })
  return {read:(pid:number)=>new Promise<any>((resolve,reject)=>{const id=++next;const timer=setTimeout(()=>{pending.delete(id);reject(new Error("Native startup readback exceeded ten seconds"))},10_000);pending.set(id,{resolve,reject,timer});child.stdin.write(JSON.stringify({id,pid})+"\n")}),close(){child.stdin.end();child.kill();fail(new Error("Native startup probe closed"))}}
}

export function closeStartupNativeProbe(){windowsProbe?.close();windowsProbe=undefined}

async function windowsSnapshot(browserPid = 0) {
  const result=await (windowsProbe??=openWindowsProbe()).read(browserPid)
  assertWindowsConsole(result.desktop,os.release())
  return result
}

export async function startupConsoleIdle(cacheDir: string) {
  if (process.platform === "win32") return (await windowsSnapshot()).idleMilliseconds as number
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
    async read(desktopScreenshot?: string): Promise<StartupNativeAdmission> {
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
      const native = await windowsSnapshot(browserPid)
      const matches = native.windows.filter((w: any) => w.bounds.Left === facts.bounds.left && w.bounds.Top === facts.bounds.top
        && w.bounds.Right - w.bounds.Left === facts.bounds.width && w.bounds.Bottom - w.bounds.Top === facts.bounds.height)
      records.push({ at: Date.now(), facts, native, targetId: targetInfo.targetId })
      if (matches.length !== 1 || established !== undefined && established !== matches[0].id) throw new Error("Static startup page/native window linkage changed or is ambiguous")
      const window = matches[0]; established = window.id
      const documentState = await page.evaluate(() => ({ visible: document.visibilityState === "visible", focused: document.hasFocus() }))
      return { at: Date.now(), physical: true, unlocked: true, foreground: native.foreground === window.id && documentState.focused,
        visible: window.visible && documentState.visible, minimized: window.minimized, idleMilliseconds: native.idleMilliseconds,
        browserPid, windowId: window.id, targetId: targetInfo.targetId }
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
