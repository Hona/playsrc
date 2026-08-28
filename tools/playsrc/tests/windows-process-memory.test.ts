import { expect, test } from "bun:test"
import { readFile } from "node:fs/promises"
import { WINDOWS_PROCESS_MEMORY, windowsProcessMemory } from "../profile/windows-process-memory"
import { loadLocalConfig } from "../src/config"
import { writeFile } from "node:fs/promises"
import path from "node:path"

test("memory telemetry preserves exact counters, numeric PID bounds and a single console-free child", async () => {
  const source = await readFile(new URL("../profile/windows-process-memory.ts", import.meta.url), "utf8")
  expect(WINDOWS_PROCESS_MEMORY).toContain("Get-Process -Id $request.ids")
  expect(WINDOWS_PROCESS_MEMORY).toContain("Select-Object Id,WorkingSet64,PrivateMemorySize64")
  expect(source.match(/spawn\(/gu)).toHaveLength(1)
  expect(source).toContain("windowsHide: true")
  expect(source).toContain("2000")
  expect(source).not.toMatch(/SetPriority|SetForeground|SendInput|ExecutionPolicy/)
})

test.skipIf(process.platform !== "win32")("actual persistent helper returns numeric-PID memory and host pressure without a new process per read", async () => {
  const helper = windowsProcessMemory((await loadLocalConfig()).sourceCacheDir)
  try {
    await expect(helper.read([0])).rejects.toThrow("Invalid numeric-PID")
    const first = await helper.read([process.pid]), second = await helper.read([process.pid])
    await writeFile(path.join((await loadLocalConfig()).sourceCacheDir, "profile-tools/process-memory-test.json"), JSON.stringify({ first, second }, null, 2))
    for (const value of [first, second]) {
      expect(value.processes).toHaveLength(1)
      expect(value.processes[0].Id).toBe(process.pid)
      expect(value.processes[0].WorkingSet64).toBeGreaterThan(0)
      expect(value.processes[0].PrivateMemorySize64).toBeGreaterThan(0)
      expect(value.helper.pid).toBe(helper.receipt.pid)
      expect(value.host.availablePhysicalBytes).toBeGreaterThan(0)
      expect(value.host.cpu100ns.every((count: string) => /^\d+$/u.test(count))).toBe(true)
    }
  } finally { helper.close() }
  expect(helper.receipt.endedEpoch).toBeGreaterThanOrEqual(helper.receipt.startedEpoch)
})
