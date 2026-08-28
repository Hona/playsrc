import { expect, test } from "bun:test"
import { WINDOWS_OWNED_UI, ownedDiagnosticWindow } from "../profile/windows-owned-ui"
import { windowsForegroundMatches } from "../profile/native-startup"

test("owned diagnostic captures cannot admit a popup or inspect an unrelated app", () => {
  const bounds = { left: 10, top: 10, width: 1296, height: 808 }
  const native = { foreground: 20, foregroundAfter: 20, foregroundOwner: { processId: 100, rootOwnerWindowId: 10 },
    windows: [{ id: 10, visible: true, minimized: false, bounds: { Left: 10, Top: 10, Right: 1306, Bottom: 818 } }] }
  expect(ownedDiagnosticWindow(native, bounds, 100)).toBe(10)
  expect(windowsForegroundMatches(native, 10, true)).toBe(false)
  expect(() => ownedDiagnosticWindow(native, bounds, 200)).toThrow()
  expect(() => ownedDiagnosticWindow({ ...native, foregroundAfter: 30 }, bounds, 100)).toThrow()
  expect(() => ownedDiagnosticWindow({ ...native, foregroundOwner: { processId: 100, rootOwnerWindowId: 30 } }, bounds, 100)).toThrow()
  expect(WINDOWS_OWNED_UI).not.toMatch(/Invoke\(|SetFocus|SendInput|SetForeground|Select\(|Toggle\(/)
  expect(WINDOWS_OWNED_UI).toContain("$rows.Count -lt 48")
  expect(WINDOWS_OWNED_UI).toContain("$clock.ElapsedMilliseconds -lt 1500")
})
