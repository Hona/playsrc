import { expect, test } from "bun:test"
import { WINDOWS_INPUT, windowsForegroundMatches } from "../profile/native-startup"

test("foreground diagnosis adds read-only identity, never focus actions or sensitive window text", () => {
  expect(WINDOWS_INPUT).toContain("GetWindowThreadProcessId")
  expect(WINDOWS_INPUT).toContain("GetClassNameW")
  expect(WINDOWS_INPUT).toContain("GetAncestor")
  expect(WINDOWS_INPUT).not.toMatch(/SetForeground|SetFocus|SendInput|mouse_event|keybd_event|GetWindowText|MainModule|UserName/)
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
