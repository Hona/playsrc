import { spawn } from "node:child_process"
import { createHash } from "node:crypto"
import { mkdirSync, readFileSync, writeFileSync } from "node:fs"
import path from "node:path"

export const WINDOWS_PROCESS_MEMORY = String.raw`
[Console]::OutputEncoding=New-Object System.Text.UTF8Encoding($false)
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class ProcessMemoryHost {
 [StructLayout(LayoutKind.Sequential)] public struct Memory { public uint Length,Load; public ulong Total,Available,PageFile,AvailablePageFile,Virtual,AvailableVirtual,Extended; }
 [DllImport("kernel32.dll")] static extern bool GlobalMemoryStatusEx(ref Memory value);
 [DllImport("kernel32.dll")] static extern bool GetSystemTimes(out ulong idle,out ulong kernel,out ulong user);
 public static Memory ReadMemory() { Memory m=new Memory();m.Length=(uint)Marshal.SizeOf(m);if(!GlobalMemoryStatusEx(ref m))throw new Exception("Memory pressure unavailable");return m; }
 public static string[] ReadCpu() { ulong i,k,u;if(!GetSystemTimes(out i,out k,out u))throw new Exception("CPU counters unavailable");return new string[]{i.ToString(),k.ToString(),u.ToString()}; }
}
'@
while ($line=[Console]::ReadLine()) {
 try {
  $request=$line|ConvertFrom-Json
  if ($request.ids.Count -lt 1 -or $request.ids.Count -gt 64) {throw 'Invalid process count'}
  foreach($id in $request.ids) {if ($id -le 0 -or $id -gt [int]::MaxValue -or [long]$id -ne $id) {throw 'Invalid numeric PID'}}
  $rows=@(Get-Process -Id $request.ids -ErrorAction SilentlyContinue | Select-Object Id,WorkingSet64,PrivateMemorySize64)
  $memory=[ProcessMemoryHost]::ReadMemory();$cpu=[ProcessMemoryHost]::ReadCpu()
  $result=@{id=$request.id;processes=$rows;host=@{epoch=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds();cpu100ns=$cpu;memoryLoad=$memory.Load;totalPhysicalBytes=$memory.Total;availablePhysicalBytes=$memory.Available;commitLimitBytes=$memory.PageFile;availableCommitBytes=$memory.AvailablePageFile};helper=@{pid=$PID;cpuSeconds=(Get-Process -Id $PID).CPU}}
 } catch {$result=@{id=$request.id;error=($_|Out-String)}}
 [Console]::WriteLine(($result|ConvertTo-Json -Depth 5 -Compress))
}
`

export function windowsProcessMemory(cacheDir: string) {
  if (process.platform !== "win32") throw new Error("Windows process telemetry requires Windows")
  const filename = path.join(cacheDir, "profile-tools", `process-memory-${createHash("sha256").update(WINDOWS_PROCESS_MEMORY).digest("hex")}.ps1`)
  mkdirSync(path.dirname(filename), { recursive: true })
  const bytes = "\uFEFF" + WINDOWS_PROCESS_MEMORY
  try { writeFileSync(filename, bytes, { flag: "wx" }) }
  catch (error) { if ((error as NodeJS.ErrnoException).code !== "EEXIST" || readFileSync(filename, "utf8") !== bytes) throw error }
  const startedEpoch = Date.now()
  const child = spawn("powershell.exe", ["-NoProfile", "-NonInteractive", "-File", filename], { windowsHide: true, stdio: ["pipe", "pipe", "pipe"] })
  let next = 0, text = "", stderr = "", closed = false
  const pending = new Map<number, { resolve(value: any): void; reject(error: Error): void; timer: ReturnType<typeof setTimeout> }>()
  const fail = (error: Error) => { for (const entry of pending.values()) { clearTimeout(entry.timer); entry.reject(error) }; pending.clear() }
  child.stderr.on("data", value => { stderr = (stderr + value).slice(-2048) })
  child.on("error", fail)
  child.on("exit", () => { closed = true; fail(new Error(`Numeric-PID telemetry exited: ${stderr}`)) })
  child.stdout.on("data", value => {
    text += value
    while (text.includes("\n")) {
      const end = text.indexOf("\n"), line = text.slice(0, end).trim(); text = text.slice(end + 1)
      if (!line) continue
      let value: any
      try { value = JSON.parse(line) } catch { fail(new Error("Invalid numeric-PID telemetry JSON")); continue }
      const entry = pending.get(value.id); if (!entry) continue
      pending.delete(value.id); clearTimeout(entry.timer)
      value.error ? entry.reject(new Error(value.error)) : entry.resolve(value)
    }
  })
  const receipt = { pid: child.pid, startedEpoch, endedEpoch: undefined as number | undefined, command: "persistent-process-memory-sampler" }
  return {
    receipt,
    read(ids: readonly number[]): Promise<any> {
      if (closed || ids.length < 1 || ids.length > 64 || ids.some(id => !Number.isSafeInteger(id) || id < 1 || id > 0x7fffffff)) return Promise.reject(new Error("Invalid numeric-PID telemetry request"))
      return new Promise((resolve, reject) => {
        const id = ++next, timer = setTimeout(() => { pending.delete(id); reject(new Error("Numeric-PID telemetry exceeded two seconds")); child.kill() }, 2000)
        pending.set(id, { resolve, reject, timer }); child.stdin.write(JSON.stringify({ id, ids }) + "\n")
      })
    },
    close() { closed = true; receipt.endedEpoch = Date.now(); child.stdin.end(); child.kill(); fail(new Error("Numeric-PID telemetry closed")) },
  }
}
