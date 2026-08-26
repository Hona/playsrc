import { describe, expect, test } from "bun:test"
import { assertWindowsConsole, parseWindowsDesktopState, type WindowsDesktopState } from "../profile/windows-desktop"

const unlocked: WindowsDesktopState = { consoleSessionId: 2, processSessionId: 2, level: 1, sessionId: 2, state: 0, flags: 1, protocol: 0 }

describe("read-only Windows headed admission", () => {
  test("admits only the active unlocked local console and its own runner session", () => {
    expect(() => assertWindowsConsole(parseWindowsDesktopState(JSON.stringify(unlocked)), "10.0.26200")).not.toThrow()
    for (const change of [
      { flags: 0 }, { flags: -1 }, { flags: 0xffff_ffff }, { state: 4 }, { protocol: 2 },
      { level: 2 }, { sessionId: 3 }, { consoleSessionId: 0 }, { consoleSessionId: 0xffff_ffff }, { processSessionId: 0 },
    ]) expect(() => assertWindowsConsole({ ...unlocked, ...change }, "10.0.26200")).toThrow()
  })

  test("fails closed for missing, string-valued, or legacy inverted lock evidence", () => {
    expect(() => parseWindowsDesktopState("{}")).toThrow("Incomplete")
    expect(() => parseWindowsDesktopState(JSON.stringify({ ...unlocked, flags: "1" }))).toThrow("Incomplete")
    expect(() => assertWindowsConsole(unlocked, "6.1.7601")).toThrow("semantics")
    expect(() => assertWindowsConsole(unlocked, "unknown")).toThrow("semantics")
  })
})
