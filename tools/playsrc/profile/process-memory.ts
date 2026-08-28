import { execFile } from "node:child_process"

export type ProfileProcess = Readonly<{ id: number; type: string }>
export type ProcessMemory = ProfileProcess & Readonly<{ residentBytes: number | null; privateBytes: number | null; priorityClass?: string | null; cpuSeconds?: number | null }>
export type MemorySnapshot = Readonly<{
  platform: string; startedAt: number; endedAt: number; error: string | null;
  processes: readonly ProcessMemory[]; residentBytes: number | null; privateBytes: number | null;
}>

type Command = Readonly<{ file: string; args: readonly string[] }>
export const MEMORY_COMMAND_TIMEOUT = 5_000

export function processMemoryCommand(platform: string, processes: readonly ProfileProcess[]): Command {
  if (!processes.length || processes.length > 256 || processes.some(process => !Number.isSafeInteger(process.id) || process.id < 1)
    || new Set(processes.map(process => process.id)).size !== processes.length) throw new Error("Invalid browser process identities")
  const ids = processes.map(process => process.id).join(",")
  if (platform === "win32") {
    // Get-Process uses numeric IDs, not browser names or localized performance counters.
    // WorkingSet64 is resident memory; PrivateMemorySize64 is committed private memory, not private working set.
    const script = `$ErrorActionPreference='Stop'; $rows=@(foreach ($processId in @(${ids})) { try { $p=Get-Process -Id $processId -ErrorAction Stop; $priority=$null;$cpu=$null;try{$priority=[string]$p.PriorityClass;$cpu=$p.TotalProcessorTime.TotalSeconds}catch{}; [pscustomobject]@{id=$p.Id;residentBytes=$p.WorkingSet64;privateBytes=$p.PrivateMemorySize64;priorityClass=$priority;cpuSeconds=$cpu} } catch [Microsoft.PowerShell.Commands.ProcessCommandException] {} }); ConvertTo-Json -InputObject $rows -Compress`
    return { file: "powershell.exe", args: ["-NoProfile", "-NonInteractive", "-EncodedCommand", Buffer.from(script, "utf16le").toString("base64")] }
  }
  if (platform === "darwin" || platform === "linux") return { file: "ps", args: ["-o", "pid=,rss=", "-p", ids] }
  throw new Error(`Process memory is unavailable on ${platform}`)
}

export function decodeProcessMemory(platform: string, output: string, processes: readonly ProfileProcess[]): readonly ProcessMemory[] {
  const rows: Array<{ id: number; residentBytes: number; privateBytes: number | null; priorityClass?: string | null; cpuSeconds?: number | null }> = platform === "win32"
    ? JSON.parse(output.replace(/^\uFEFF/u, ""))
    : output.trim().split(/\r?\n/u).filter(Boolean).map(line => {
      const fields = line.trim().split(/\s+/u)
      if (fields.length !== 2) throw new Error("Malformed process memory row")
      return { id: Number(fields[0]), residentBytes: Number(fields[1]) * 1024, privateBytes: null }
    })
  const ids = new Set(processes.map(process => process.id))
  const seen = new Set<number>()
  if (!Array.isArray(rows)) throw new Error("Malformed process memory response")
  for (const row of rows) {
    if (!row || !ids.has(row.id) || seen.has(row.id) || !Number.isSafeInteger(row.residentBytes) || row.residentBytes < 0
      || (platform === "win32" && (!Number.isSafeInteger(row.privateBytes) || row.privateBytes! < 0))) throw new Error("Malformed process memory response")
    seen.add(row.id)
    if (row.priorityClass != null && typeof row.priorityClass !== "string" || row.cpuSeconds != null && (!Number.isFinite(row.cpuSeconds) || row.cpuSeconds < 0)) throw new Error("Malformed process scheduling observation")
  }
  const byId = new Map(rows.map(row => [row.id, row]))
  return processes.map(process => ({ ...process, residentBytes: byId.get(process.id)?.residentBytes ?? null, privateBytes: byId.get(process.id)?.privateBytes ?? null,
    ...(platform === "win32" ? { priorityClass: byId.get(process.id)?.priorityClass ?? null, cpuSeconds: byId.get(process.id)?.cpuSeconds ?? null } : {}) }))
}

export function completeMemoryTotal(processes: readonly ProcessMemory[], key: "residentBytes" | "privateBytes"): number | null {
  if (!processes.length || processes.some(process => process[key] === null)) return null
  const total = processes.reduce((sum, process) => sum + process[key]!, 0)
  return Number.isSafeInteger(total) ? total : null
}

const execute = (command: Command): Promise<string> => new Promise((resolve, reject) => {
  execFile(command.file, [...command.args], { encoding: "utf8", timeout: MEMORY_COMMAND_TIMEOUT, maxBuffer: 256 * 1024, windowsHide: true }, (error, stdout) => {
    if (error) reject(error)
    else resolve(stdout)
  })
})

/** Boundary snapshots only: no shell/process launches during the marked gameplay interval. */
export async function captureProcessMemory(
  processes: readonly ProfileProcess[] | undefined,
  options: Readonly<{ platform?: string; remote?: boolean; execute?: typeof execute }> = {},
): Promise<MemorySnapshot> {
  const platform = options.platform ?? process.env.PLAYSRC_PROFILE_BROWSER_PLATFORM ?? process.platform
  const startedAt = Date.now()
  let measured: readonly ProcessMemory[] = (processes ?? []).map(process => ({ ...process, residentBytes: null, privateBytes: null }))
  let error: string | null = null
  try {
    // CDP PIDs belong to the browser host, which need not be this runner's machine.
    const hostReader = process.env.PLAYSRC_PROFILE_PROCESS_MEMORY_EXECUTABLE
    if (options.remote && (!hostReader || !process.env.PLAYSRC_PROFILE_BROWSER_PLATFORM)) throw new Error("Remote CDP process memory requires collection on the browser host")
    const localCommand = processMemoryCommand(platform, processes ?? [])
    const command = options.remote
      ? { file: hostReader!, args: [JSON.stringify(processes)] }
      : localCommand
    measured = decodeProcessMemory(platform, await (options.execute ?? execute)(command), processes!)
    if (measured.some(process => process.residentBytes === null)) error = "One or more browser processes exited or were unavailable"
  } catch (failure) { error = failure instanceof Error ? failure.message : String(failure) }
  return { platform, startedAt, endedAt: Date.now(), error, processes: measured,
    residentBytes: completeMemoryTotal(measured, "residentBytes"), privateBytes: completeMemoryTotal(measured, "privateBytes") }
}
