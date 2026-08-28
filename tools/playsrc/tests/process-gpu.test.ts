import { expect, test } from "bun:test"
import { decodeGpuEngineSamples } from "../profile/process-gpu"
import { execFileSync } from "node:child_process"
import { fileURLToPath } from "node:url"
test("GPU engine percentages retain native status and precise clocks without combining engines", () => {
  const row = { at: "2026-08-28T12:00:00.0000000Z", timestamp100ns: "134324784000000000", instance: "pid_42_luid_0x0_0x123_phys_0_eng_0_engtype_3D", pid: 42, percent: 12.5, status: 0 }
  expect(decodeGpuEngineSamples(JSON.stringify([row, { ...row, status: 1, percent: 15 }]), [{ id: 42, type: "GPU" }])).toHaveLength(2)
  expect(() => decodeGpuEngineSamples(JSON.stringify([{ ...row, pid: 43 }]), [{ id: 42, type: "GPU" }])).toThrow()
  expect(() => decodeGpuEngineSamples(JSON.stringify([{ ...row, percent: -1 }]), [{ id: 42, type: "GPU" }])).toThrow()
})

test.skipIf(process.platform !== "win32")("native GPU reader parses before a headed capture is attempted", () => {
  const file = fileURLToPath(new URL("../profile/process-gpu.ps1", import.meta.url)).replaceAll("'", "''")
  const script = `$tokens=$null;$errors=$null;[System.Management.Automation.Language.Parser]::ParseFile('${file}',[ref]$tokens,[ref]$errors)|Out-Null;if($errors.Count){throw ($errors|Out-String)}`
  expect(() => execFileSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-EncodedCommand", Buffer.from(script, "utf16le").toString("base64")], { windowsHide: true, timeout: 5000 })).not.toThrow()
})
