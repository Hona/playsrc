import { describe, expect, test } from "bun:test"
import { parseVguiAnimationScript } from "../src"

const limits = Object.freeze({
  maximumSourceBytes: 65_536,
  maximumTokenCodeUnits: 511,
  maximumSequences: 16,
  maximumCommands: 64,
})

describe("Source VGUI animation script parser", () => {
  test("parses manifest-ordered command records without executing them", () => {
    const bytes = new TextEncoder().encode(`
      // fixed parser vector
      event HudPulse [$WIN32]
      {
        Animate Health Position "(center:Root)10 20" Pulse 2 0.1 0.4 [$WIN32]
        RunEvent Child 0.2
        RunEventChild Nested Child 0.3
        StopEvent Child 0.4
        StopAnimation Health alpha 0.5
        StopPanelAnimations Health 0.6
        SetFont Health font Default 0.7
        SetTexture Health textureid "hud/health" 0.8
        SetString Health Caption "ready now" 0.9
        FireCommand 1.0 "status"
        PlaySound 1.1 "UI/buttonclick.wav"
        SetVisible Health 1 1.2
        SetInputEnabled Health 0 1.3
      }
    `)
    const result = parseVguiAnimationScript("scripts/fixed.txt", "fixed-1", bytes, limits)
    expect(result.ok).toBeTrue()
    if (!result.ok) return
    expect(result.script.sequences).toHaveLength(1)
    expect(result.script.sequences[0]?.condition).toBe("[$WIN32]")
    expect(result.script.sequences[0]?.commands.map((command) => command.kind)).toEqual([
      "Animate", "RunEvent", "RunEventChild", "StopEvent", "StopAnimation", "StopPanelAnimations",
      "SetFont", "SetTexture", "SetString", "FireCommand", "PlaySound", "SetVisible", "SetInputEnabled",
    ])
    expect(result.script.sequences[0]?.commands[0]).toMatchObject({
      target: "10 20",
      relative: { panel: "Root", alignment: "center" },
      interpolator: "Pulse",
      parameter: 2,
      condition: "[$WIN32]",
    })
  })

  test("returns typed failures for malformed, unknown, and over-bound input", () => {
    const malformed = parseVguiAnimationScript("scripts/bad.txt", "bad-1", new TextEncoder().encode("event Missing { Animate"), limits)
    expect(malformed.ok).toBeFalse()
    if (!malformed.ok) expect(malformed.diagnostic.code).toBe("MalformedSource")
    const unknown = parseVguiAnimationScript("scripts/bad.txt", "bad-1", new TextEncoder().encode("event Bad { Mystery 0 }"), limits)
    expect(unknown.ok).toBeFalse()
    if (!unknown.ok) expect(unknown.diagnostic.code).toBe("UnknownCommand")
    const bounded = parseVguiAnimationScript("scripts/bad.txt", "bad-1", new TextEncoder().encode("event A { RunEvent B 0 }"), { ...limits, maximumCommands: 1, maximumSequences: 1, maximumTokenCodeUnits: 1 })
    expect(bounded.ok).toBeFalse()
    if (!bounded.ok) expect(bounded.diagnostic.code).toBe("BoundExceeded")
  })
})
