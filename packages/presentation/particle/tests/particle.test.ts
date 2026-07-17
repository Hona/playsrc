import { describe, expect, test } from "bun:test"
import { createParticleSystem, ParticleError } from "../src"

describe("particle sprite output", () => {
  test("advances only supplied definitions and explicit ticks", () => {
    const system = createParticleSystem([{
      identity: "explosion_core",
      lifetimeTicks: 4,
      startRadius: 4,
      endRadius: 12,
      color: 0xff8000,
      startOpacity: 1,
      endOpacity: 0,
    }])
    system.emit({ identity: 7, definition: "EXPLOSION_CORE", tick: 10n, position: [1, 2, 3] })
    expect(system.advance(10n)[0]).toMatchObject({ identity: 7, radius: 4, opacity: 1 })
    expect(system.advance(12n)[0]).toMatchObject({ identity: 7, radius: 8, opacity: 0.5 })
    expect(system.advance(14n)).toEqual([])
  })

  test("refuses missing definitions without a substitute effect", () => {
    const system = createParticleSystem([])
    expect(() => system.emit({ identity: 1, definition: "missing", tick: 0n, position: [0, 0, 0] }))
      .toThrow(ParticleError)
    expect(system.advance(0n)).toEqual([])
  })
})
