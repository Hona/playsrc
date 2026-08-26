import { describe, expect, test } from "bun:test"
import { captureProcessMemory, completeMemoryTotal, decodeProcessMemory, processMemoryCommand } from "../profile/process-memory"

const processes = [{ id: 12, type: "browser" }, { id: 34, type: "renderer" }, { id: 56, type: "GPU" }]

describe("browser-host process memory boundaries", () => {
  test("collects Chrome or Edge by exact CDP PID without enumerating user processes", () => {
    const command = processMemoryCommand("win32", processes)
    expect(command.file).toBe("powershell.exe")
    const script = Buffer.from(command.args.at(-1)!, "base64").toString("utf16le")
    expect(script).toContain("@(12,34,56)")
    expect(script).toContain("Get-Process -Id $processId")
    expect(script).not.toMatch(/msedge|chrome|Stop-Process|Set-/u)
    expect(() => processMemoryCommand("win32", [{ id: NaN, type: "GPU" }])).toThrow("identities")
    expect(() => processMemoryCommand("win32", [processes[0]!, processes[0]!])).toThrow("identities")
  })

  test("retains distinct Windows working set and private commit, including multiple renderer processes", async () => {
    const snapshot = await captureProcessMemory(processes, { platform: "win32", execute: async () => JSON.stringify([
      { id: 56, residentBytes: 300, privateBytes: 600 }, { id: 12, residentBytes: 100, privateBytes: 200 }, { id: 34, residentBytes: 200, privateBytes: 400 },
    ]) })
    expect(snapshot.error).toBeNull()
    expect(snapshot.processes.map(process => process.id)).toEqual([12, 34, 56])
    expect(snapshot.residentBytes).toBe(600)
    expect(snapshot.privateBytes).toBe(1200)
    expect(snapshot.endedAt).toBeGreaterThanOrEqual(snapshot.startedAt)
  })

  test("preserves POSIX RSS units without inventing private memory", () => {
    const measured = decodeProcessMemory("darwin", " 12 100\n 34 200\n 56 300\n", processes)
    expect(completeMemoryTotal(measured, "residentBytes")).toBe(600 * 1024)
    expect(completeMemoryTotal(measured, "privateBytes")).toBeNull()
    expect(processMemoryCommand("linux", processes).args).toEqual(["-o", "pid=,rss=", "-p", "12,34,56"])
  })

  test("does not report absent, exited, denied or malformed process memory as zero", async () => {
    const partial = await captureProcessMemory(processes, { platform: "win32", execute: async () => '[{"id":12,"residentBytes":100,"privateBytes":200}]' })
    expect(partial.residentBytes).toBeNull()
    expect(partial.privateBytes).toBeNull()
    expect(partial.processes[0]!.residentBytes).toBe(100)
    expect(partial.error).toContain("unavailable")
    for (const output of ["[]", "{}", '[{"id":12,"residentBytes":-1,"privateBytes":2}]', '[{"id":999,"residentBytes":1,"privateBytes":2}]']) {
      const snapshot = await captureProcessMemory(processes, { platform: "win32", execute: async () => output })
      expect(snapshot.residentBytes).toBeNull()
      expect(snapshot.error).not.toBeNull()
    }
    const failed = await captureProcessMemory(processes, { execute: async () => { throw new Error("timeout") } })
    expect(failed.error).toBe("timeout")
    expect(failed.residentBytes).toBeNull()
  })

  test("never queries local PIDs for a remotely connected browser", async () => {
    let calls = 0
    const snapshot = await captureProcessMemory(processes, { remote: true, execute: async () => { calls++; return "" } })
    expect(calls).toBe(0)
    expect(snapshot.residentBytes).toBeNull()
    expect(snapshot.error).toContain("browser host")
  })

  test("collects the actual local test process without a browser or process enumeration", async () => {
    if (!["darwin", "linux", "win32"].includes(process.platform)) return
    const snapshot = await captureProcessMemory([{ id: process.pid, type: "test-runner" }])
    expect(snapshot.error).toBeNull()
    expect(snapshot.residentBytes).toBeGreaterThan(0)
    if (process.platform === "win32") expect(snapshot.privateBytes).toBeGreaterThan(0)
    else expect(snapshot.privateBytes).toBeNull()
  })
})
