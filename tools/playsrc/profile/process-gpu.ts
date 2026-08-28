import { spawn } from "node:child_process"
import { fileURLToPath } from "node:url"
import type { ProfileProcess } from "./process-memory"

export type GpuEngineSample = { at: string; timestamp100ns: string; instance: string; pid: number; percent: number; status: number }

export function decodeGpuEngineSamples(output: string, processes: readonly ProfileProcess[]): GpuEngineSample[] {
  const rows = JSON.parse(output.replace(/^\uFEFF/, ""))
  const ids = new Set(processes.map(process => process.id))
  if (!Array.isArray(rows)) throw new Error("Invalid GPU engine samples")
  for (const row of rows) {
    const pid = /^pid_(\d+)_/.exec(row.instance ?? "")?.[1]
    if (!pid || Number(pid) !== row.pid || !ids.has(row.pid) || !/^\d+$/.test(row.timestamp100ns)
      || !Number.isFinite(Date.parse(row.at)) || !Number.isFinite(row.percent) || row.percent < 0
      || !Number.isSafeInteger(row.status) || row.status < 0) throw new Error("Invalid GPU engine sample identity/value")
  }
  return rows
}

/** Native PDH cooks each per-process engine's utilization percentage. Keep
 * individual engines and invalid statuses; never sum overlapping GPU engines
 * or label GPU-process CPU time as GPU device utilization. No game hooks. */
export async function startGpuEngineCapture(processes: readonly ProfileProcess[], seconds: number) {
  if (process.platform !== "win32") throw new Error("GPU engine capture requires the native Windows host")
  if (!processes.length || processes.length > 256 || processes.some(process => !Number.isSafeInteger(process.id) || process.id < 1)
    || !Number.isInteger(seconds) || seconds < 5 || seconds > 10) throw new Error("Invalid GPU engine capture bounds")
  const ids = processes.filter(process => process.type === "GPU").map(process => process.id)
  if (!ids.length) throw new Error("No authenticated browser GPU process")
  // The first sample establishes PDH's counter baseline before gameplay starts.
  const script = fileURLToPath(new URL("./process-gpu.ps1", import.meta.url))
  const child = spawn("powershell.exe", ["-NoProfile", "-NonInteractive", "-File", script, "-ProcessIds", ids.join(","), "-Seconds", String(seconds)], { windowsHide: true, stdio: ["ignore", "pipe", "pipe"] })
  let output = "", errors = "", readyResolve!: () => void, readyReject!: (error: Error) => void
  const ready = new Promise<void>((resolve, reject) => { readyResolve = resolve; readyReject = reject })
  const deadline = setTimeout(() => child.kill(), (seconds + 6) * 1000)
  const finished = new Promise<{ samples: GpuEngineSample[]; error: string | null; raw?: string }>(resolve => {
    child.stdout.on("data", chunk => {
      output += chunk.toString()
      if (output.startsWith("READY\r\n") || output.startsWith("READY\n")) readyResolve()
      if (output.length > 1024 * 1024) child.kill()
    })
    child.stderr.on("data", chunk => { errors = (errors + chunk.toString()).slice(-4096) })
    child.on("error", error => { readyReject(error); clearTimeout(deadline); resolve({ samples: [], error: String(error) }) })
    child.on("exit", code => {
      clearTimeout(deadline)
      if (!/^READY\r?\n/.test(output)) readyReject(new Error(errors || "GPU reader exited without its baseline sample"))
      if (code !== 0) { readyReject(new Error(errors || `GPU counter reader exited ${code}`)); resolve({ samples: [], error: errors || `GPU counter reader exited ${code}` }); return }
      try { const samples = decodeGpuEngineSamples(output.replace(/^READY\r?\n/, ""), processes); resolve({ samples, error: samples.length ? null : "No owned GPU engine counters" }) }
      catch (error) { readyReject(error as Error); resolve({ samples: [], error: String(error), raw: output }) }
    })
  })
  await ready
  return { finished, scope: "Native PDH per-process engine utilization samples; engine intervals can overlap and are not summed. Raw timestamps/statuses retain boundary and invalid samples." }
}
