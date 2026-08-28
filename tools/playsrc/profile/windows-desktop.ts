import { execFileSync } from "node:child_process"
import os from "node:os"

// Read-only WTSInfoEx prefix. Its union is eight-byte aligned because level 1
// contains LARGE_INTEGER timestamps. No user/domain names are collected.
// https://learn.microsoft.com/windows/win32/api/wtsapi32/ns-wtsapi32-wtsinfoexw
export const WINDOWS_DESKTOP_QUERY = String.raw`$ErrorActionPreference='Stop'
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class ProfileConsole {
 [StructLayout(LayoutKind.Sequential)] public struct LastInput { public uint cbSize; public uint dwTime; }
 [DllImport("user32.dll", SetLastError=true)] public static extern bool GetLastInputInfo(ref LastInput info);
 public static uint IdleMilliseconds() {
  var input = new LastInput { cbSize = (uint)Marshal.SizeOf(typeof(LastInput)) };
  if (!GetLastInputInfo(ref input)) throw new System.ComponentModel.Win32Exception();
  return unchecked((uint)Environment.TickCount - input.dwTime);
 }
 [StructLayout(LayoutKind.Explicit, Size=24)]
 public struct InfoPrefix {
  [FieldOffset(0)] public uint Level;
  [FieldOffset(8)] public uint SessionId;
  [FieldOffset(12)] public int State;
  [FieldOffset(16)] public int Flags;
 }
 [DllImport("kernel32.dll")] public static extern uint WTSGetActiveConsoleSessionId();
 [DllImport("wtsapi32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
 public static extern bool WTSQuerySessionInformationW(IntPtr server, uint session, int kind, out IntPtr data, out int bytes);
 [DllImport("wtsapi32.dll")] public static extern void WTSFreeMemory(IntPtr data);
 public static InfoPrefix Query(uint session) {
  IntPtr data; int bytes;
  if (!WTSQuerySessionInformationW(IntPtr.Zero, session, 25, out data, out bytes)) throw new System.ComponentModel.Win32Exception();
  try {
   if (bytes < Marshal.SizeOf(typeof(InfoPrefix))) throw new Exception("Incomplete WTSInfoEx response");
   return (InfoPrefix)Marshal.PtrToStructure(data, typeof(InfoPrefix));
  } finally { WTSFreeMemory(data); }
 }
 public static short Protocol(uint session) {
  IntPtr data; int bytes;
  if (!WTSQuerySessionInformationW(IntPtr.Zero, session, 16, out data, out bytes)) throw new System.ComponentModel.Win32Exception();
  try { if (bytes < 2) throw new Exception("Incomplete WTS protocol response"); return Marshal.ReadInt16(data); }
  finally { WTSFreeMemory(data); }
 }
}
"@
$session=[ProfileConsole]::WTSGetActiveConsoleSessionId()
if ($session -eq [uint32]::MaxValue) { throw 'No active physical console session' }
$info=[ProfileConsole]::Query($session)
[pscustomobject]@{consoleSessionId=$session;processSessionId=[System.Diagnostics.Process]::GetCurrentProcess().SessionId;level=$info.Level;sessionId=$info.SessionId;state=$info.State;flags=$info.Flags;protocol=[ProfileConsole]::Protocol($session);idleMilliseconds=[ProfileConsole]::IdleMilliseconds()} | ConvertTo-Json -Compress
`

export type WindowsDesktopState = Readonly<{
  consoleSessionId: number; processSessionId: number; level: number; sessionId: number; state: number; flags: number; protocol: number;
  idleMilliseconds: number;
}>

export function parseWindowsDesktopState(output: string): WindowsDesktopState {
  const value = JSON.parse(output.replace(/^\uFEFF/u, "")) as WindowsDesktopState
  if (!value || [value.consoleSessionId, value.processSessionId, value.level, value.sessionId, value.state, value.flags, value.protocol, value.idleMilliseconds]
    .some(field => !Number.isSafeInteger(field))) throw new Error("Incomplete Windows desktop evidence")
  return value
}

export function assertWindowsConsole(state: WindowsDesktopState, release: string): void {
  // Windows 7 / Server 2008 R2 invert the lock flags. Do not guess on older hosts.
  const version = /^(\d+)\.(\d+)(?:\.|$)/u.exec(release)
  if (!version || Number(version[1]) < 6 || (Number(version[1]) === 6 && Number(version[2]) < 2)) throw new Error("Unsupported Windows session flag semantics")
  if (state.level !== 1 || state.consoleSessionId < 1 || state.consoleSessionId >= 0xffff_ffff || state.sessionId !== state.consoleSessionId
    || state.state !== 0 || state.flags !== 1 || state.protocol !== 0) throw new Error("Windows physical console is not confirmed active and unlocked")
  if (state.processSessionId !== state.consoleSessionId) throw new Error("Profiler is not running in the unlocked physical console session; no browser will be launched")
}

export function assertWindowsIdle(state: WindowsDesktopState): void {
  if (!Number.isSafeInteger(state.idleMilliseconds) || state.idleMilliseconds < 2_000 || state.idleMilliseconds > 0xffff_ffff) throw new Error("Windows physical console requires two seconds of genuine idle before profiling")
}

export function queryWindowsDesktop(timeout = 10_000): WindowsDesktopState {
  if (process.platform !== "win32") throw new Error("Windows desktop evidence must be collected on the Windows host")
  if (!Number.isSafeInteger(timeout) || timeout < 1 || timeout > 10_000) throw new Error("Windows desktop query deadline is invalid")
  return parseWindowsDesktopState(execFileSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-EncodedCommand",
    Buffer.from(WINDOWS_DESKTOP_QUERY, "utf16le").toString("base64")], { encoding: "utf8", timeout, maxBuffer: 64 * 1024, windowsHide: true }))
}

export function requireWindowsProfileConsole(remaining = 10_000): WindowsDesktopState | null {
  if (process.platform !== "win32") return null
  const deadline = Date.now() + Math.min(10_000, remaining)
  let state: WindowsDesktopState
  do {
    state = queryWindowsDesktop(Math.max(1, deadline - Date.now()))
    assertWindowsConsole(state, os.release())
    if (state.idleMilliseconds >= 2_000 || Date.now() + 100 >= deadline) break
    // Only wait for actual GetLastInputInfo aging; never send input, reset its
    // clock, or suppress the user's events to manufacture admission.
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 100)
  } while (Date.now() < deadline)
  assertWindowsIdle(state)
  return state
}

if (import.meta.main) {
  try {
    const state = queryWindowsDesktop()
    let blocker: string | null = null
    try { assertWindowsConsole(state, os.release()) } catch (error) { blocker = String(error) }
    console.log(JSON.stringify({ state, release: os.release(), blocker }))
    if (blocker) process.exitCode = 1
  } catch (error) { console.error(String(error)); process.exitCode = 1 }
}
