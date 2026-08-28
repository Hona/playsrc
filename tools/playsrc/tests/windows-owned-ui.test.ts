import { expect, test } from "bun:test"
import { WINDOWS_OWNED_UI, ownedDiagnosticWindow, assertOwnedEphemeralBrowser, WINDOWS_LOCAL_PERMISSION } from "../profile/windows-owned-ui"
import { windowsForegroundMatches } from "../profile/native-startup"
import { execFileSync } from "node:child_process"

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

test.skipIf(process.platform !== "win32")("native UI Automation empty bounds remain valid JSON in Windows PowerShell", () => {
  const script = "$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue';" + WINDOWS_OWNED_UI
    + "$empty=Read-UIBounds ([System.Windows.Rect]::Empty);$finite=Read-UIBounds ([System.Windows.Rect]::new(1,2,3,4));@{empty=$empty;finite=$finite}|ConvertTo-Json -Compress"
  const output = execFileSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-EncodedCommand", Buffer.from(script, "utf16le").toString("base64")], { encoding: "utf8", timeout: 10000, windowsHide: true })
  expect(JSON.parse(output)).toEqual({ empty: null, finite: { x: 1, y: 2, width: 3, height: 4 } })
})

test("normal permission resolution is restricted to an owned temporary automation profile and exact observed control", () => {
  expect(() => assertOwnedEphemeralBrowser(["--user-data-dir=C:\\Temp\\playwright_chromiumdev_profile-Ab123"], 12, 12)).not.toThrow()
  for (const args of [["--user-data-dir=C:\\Users\\User\\Chrome"], ["--enable-automation"]]) expect(() => assertOwnedEphemeralBrowser(args, 12, 12)).toThrow()
  expect(() => assertOwnedEphemeralBrowser(["--user-data-dir=C:\\Temp\\playwright_chromiumdev_profile-Ab123"], 12, 13)).toThrow()
  expect(WINDOWS_LOCAL_PERMISSION).toContain("Access other apps and services on this device")
  expect(WINDOWS_LOCAL_PERMISSION).toContain("$matches.Count -ne 1")
  expect(WINDOWS_LOCAL_PERMISSION).toContain("GetRuntimeId()")
  expect(WINDOWS_LOCAL_PERMISSION).not.toMatch(/SetForeground|SetFocus|SendInput|CloseWindow/)
})
