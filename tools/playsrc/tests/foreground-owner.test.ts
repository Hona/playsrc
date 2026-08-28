import { expect, test } from "bun:test"
import { WINDOWS_INPUT, windowsForegroundMatches } from "../profile/native-startup"
import { execFileSync } from "node:child_process"

test("foreground diagnosis adds read-only identity, never focus actions or sensitive window text", () => {
  expect(WINDOWS_INPUT).toContain("GetWindowThreadProcessId")
  expect(WINDOWS_INPUT).toContain("GetClassNameW")
  expect(WINDOWS_INPUT).toContain("GetAncestor")
  expect(WINDOWS_INPUT).not.toMatch(/SetForeground|SetFocus|SendInput|mouse_event|keybd_event|GetWindowText|MainModule|UserName/)
})

test.skipIf(process.platform !== "win32")("the actual native identity helper compiles without requesting focus or reading window text", () => {
  const script = "$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';" + WINDOWS_INPUT + "[StartupWindow]::Owner([IntPtr]::Zero) | ConvertTo-Json -Compress"
  const output = execFileSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-EncodedCommand", Buffer.from(script, "utf16le").toString("base64")],
    { windowsHide: true, timeout: 10_000, encoding: "utf8" })
  expect(JSON.parse(output)).toMatchObject({ windowId: 0, processId: 0, ownerWindowId: 0, rootOwnerWindowId: 0, windowClass: "" })
})

test("a helper or popup remains a guard failure, including ownership changes during readback", () => {
  expect(windowsForegroundMatches({ foreground: 10, foregroundAfter: 10 }, 10, true)).toBe(true)
  expect(windowsForegroundMatches({ foreground: 20, foregroundAfter: 20 }, 10, true)).toBe(false)
  expect(windowsForegroundMatches({ foreground: 10, foregroundAfter: 20 }, 10, true)).toBe(false)
  expect(windowsForegroundMatches({ foreground: 10, foregroundAfter: 10 }, 10, false)).toBe(false)
})

test("map-memory telemetry hides only its non-GUI console helper and retains bounded input observations", async () => {
  const source = await Bun.file(new URL("../profile/map-memory.profile.ts", import.meta.url)).text()
  expect(source).toContain("{ timeout: 2_000, windowsHide: true }")
  expect(source).toContain("helperProcesses.length < 512")
  expect(source).toContain("{ capture: true, passive: true }")
  expect(source).not.toContain("preventDefault()")
})
