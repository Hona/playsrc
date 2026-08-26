import { spawnSync } from "node:child_process"

type BrowserProcess = Readonly<{ id: number; type: string }>
export function validateResidentMemory(processes: readonly BrowserProcess[], value: unknown) {
  if (!Array.isArray(value) || value.length !== processes.length) throw new Error("Browser-host memory response has a different process inventory")
  return processes.map(process => {
    const matches = value.filter(row => row?.id === process.id)
    const row = matches[0]
    if (matches.length !== 1 || row.type !== process.type || (row.residentBytes !== null && (!Number.isSafeInteger(row.residentBytes) || row.residentBytes < 0))) {
      throw new Error("Browser-host memory response has an invalid PID, type or resident byte count")
    }
    return { id: process.id, type: process.type, residentBytes: row.residentBytes as number | null }
  })
}

export function processResidentMemory(processes: readonly BrowserProcess[] | undefined) {
  if (!processes?.length) return null
  const hostReader = process.env.PLAYSRC_PROFILE_PROCESS_MEMORY_EXECUTABLE
  if (hostReader) {
    const result = spawnSync(hostReader, [JSON.stringify(processes)], { encoding: "utf8", timeout: 5000 })
    if (result.status !== 0) throw new Error(`Browser-host memory reader failed: ${result.stderr || result.error}`)
    return validateResidentMemory(processes, JSON.parse(result.stdout))
  }
  // Remote CDP IDs never identify processes on this controller.
  if (process.env.PLAYSRC_PROFILE_CDP_ENDPOINT) return null
  if (process.platform === "win32") {
    const result = spawnSync("powershell", ["-NoProfile", "-Command", `@(Get-Process -Id ${processes.map(process => process.id).join(",")} -ErrorAction SilentlyContinue | Select-Object Id,WorkingSet64) | ConvertTo-Json -Compress`], { encoding: "utf8", timeout: 5000 })
    if (result.status !== 0 || !result.stdout.trim()) return null
    const rows = [JSON.parse(result.stdout)].flat()
    return processes.map(process => ({ ...process, residentBytes: rows.find(row => row.Id === process.id)?.WorkingSet64 ?? null }))
  }
  const result = spawnSync("ps", ["-o", "pid=,rss=", "-p", processes.map(process => process.id).join(",")], { encoding: "utf8", timeout: 5000 })
  if (result.status !== 0) return null
  const resident = new Map(result.stdout.trim().split("\n").map(line => line.trim().split(/\s+/).map(Number) as [number, number]))
  return processes.map(process => ({ ...process, residentBytes: resident.has(process.id) ? resident.get(process.id)! * 1024 : null }))
}
