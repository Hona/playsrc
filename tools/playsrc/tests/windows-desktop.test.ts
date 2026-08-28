import { describe, expect, test } from "bun:test"
import { assertWindowsConsole, assertWindowsIdle, parseWindowsDesktopState, type WindowsDesktopState } from "../profile/windows-desktop"
import { requireNativeDesktopPixels } from "../profile/native-startup"

const unlocked: WindowsDesktopState = { consoleSessionId: 2, processSessionId: 2, level: 1, sessionId: 2, state: 0, flags: 1, protocol: 0, idleMilliseconds: 2_000 }

describe("read-only Windows headed admission", () => {
  test("native PNG receipts retain real capture bounds and reject substituted files or censored clocks", () => {
    const value = { path: "C:\\cache\\selection-001.desktop.png", bounds: { X: -1920, Y: 0, Width: 3840, Height: 1080 }, startedEpoch: 1000, endedEpoch: 1010 }
    expect(requireNativeDesktopPixels(value, value.path)).toBe(value)
    for (const changed of [undefined, { ...value, path: "other.png" }, { ...value, startedEpoch: 1011 },
      { ...value, endedEpoch: 7000 }, { ...value, bounds: { ...value.bounds, Width: 0 } },
      { ...value, bounds: { ...value.bounds, Height: 65536 } }]) expect(() => requireNativeDesktopPixels(changed, value.path)).toThrow("receipt")
  })
  test("admits only the active unlocked local console and its own runner session", () => {
    expect(() => assertWindowsConsole(parseWindowsDesktopState(JSON.stringify(unlocked)), "10.0.26200")).not.toThrow()
    for (const change of [
      { flags: 0 }, { flags: -1 }, { flags: 0xffff_ffff }, { state: 4 }, { protocol: 2 },
      { level: 2 }, { sessionId: 3 }, { consoleSessionId: 0 }, { consoleSessionId: 0xffff_ffff }, { processSessionId: 0 },
    ]) expect(() => assertWindowsConsole({ ...unlocked, ...change }, "10.0.26200")).toThrow()
  })

  test("requires real idle before starting but does not apply it to active scripted input", () => {
    expect(() => assertWindowsIdle(unlocked)).not.toThrow()
    for (const idleMilliseconds of [1_999, -1, Number.NaN, 0x1_0000_0000]) expect(() => assertWindowsIdle({ ...unlocked, idleMilliseconds })).toThrow("genuine idle")
    expect(() => assertWindowsConsole({ ...unlocked, idleMilliseconds: 0 }, "10.0.26200")).not.toThrow()
  })

  test("fails closed for missing, string-valued, or legacy inverted lock evidence", () => {
    expect(() => parseWindowsDesktopState("{}")).toThrow("Incomplete")
    expect(() => parseWindowsDesktopState(JSON.stringify({ ...unlocked, flags: "1" }))).toThrow("Incomplete")
    expect(() => assertWindowsConsole(unlocked, "6.1.7601")).toThrow("semantics")
    expect(() => assertWindowsConsole(unlocked, "unknown")).toThrow("semantics")
  })
})
