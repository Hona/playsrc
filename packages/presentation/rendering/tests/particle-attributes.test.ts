import { describe, expect, test } from "bun:test"
import {
  createParticleAttributeUpdates,
  resetParticleAttributeUpdates,
  writeParticleAppearance,
  type ParticleAppearance,
  type ParticleAppearanceArrays,
} from "../src/particle-attributes"

const item: ParticleAppearance = {
  color: 0x8040ff,
  opacity: 0.65,
  primarySheet: {
    current: [[0.125, 0.25, 0.375, 0.5]],
    next: [[0.625, 0.75, 0.875, 1]],
    blend: 0.3,
  },
}

function arrays(particles: number): ParticleAppearanceArrays {
  return {
    uv: new Float32Array(particles * 8),
    uvNext: new Float32Array(particles * 8),
    sheetBlend: new Float32Array(particles * 4),
    colors: new Float32Array(particles * 16),
  }
}

describe("exact retained Source particle appearance attributes", () => {
  test("preserves all four authored sprite corners, animation samples, tint, and binary32 opacity", () => {
    const output = arrays(1)
    const updates = createParticleAttributeUpdates()
    writeParticleAppearance(item, output, 0, updates)
    expect([...output.uv]).toEqual([0.125, 0.25, 0.375, 0.25, 0.375, 0.5, 0.125, 0.5])
    expect([...output.uvNext]).toEqual([0.625, 0.75, 0.875, 0.75, 0.875, 1, 0.625, 1])
    expect([...output.sheetBlend]).toEqual(Array(4).fill(Math.fround(0.3)))
    expect([...output.colors]).toEqual(Array.from({ length: 4 }, () => [Math.fround(128 / 255), Math.fround(64 / 255), 1, Math.fround(0.65)]).flat())
    expect(updates).toEqual([{ start: 0, end: 8 }, { start: 0, end: 8 }, { start: 0, end: 4 }, { start: 0, end: 16 }])
  })

  test("does not publish unchanged retained animation, blend, or tint attributes", () => {
    const output = arrays(2)
    writeParticleAppearance(item, output, 1, createParticleAttributeUpdates())
    const updates = createParticleAttributeUpdates()
    writeParticleAppearance(item, output, 1, updates)
    expect(updates).toEqual(Array.from({ length: 4 }, () => ({ start: Infinity, end: 0 })))
  })

  test("reuses one bounded update ledger across ordered material runs without retaining stale ranges", () => {
    const output = arrays(2)
    const updates = createParticleAttributeUpdates()
    writeParticleAppearance(item, output, 0, updates)
    resetParticleAttributeUpdates(updates)
    writeParticleAppearance(item, output, 1, updates)
    expect(updates).toEqual([{ start: 8, end: 16 }, { start: 8, end: 16 }, { start: 4, end: 8 }, { start: 16, end: 32 }])
  })

  test("bounds one late-particle opacity change to its exact first and last modified scalar", () => {
    const output = arrays(4)
    for (let index = 0; index < 4; index += 1) writeParticleAppearance(item, output, index, createParticleAttributeUpdates())
    const updates = createParticleAttributeUpdates()
    writeParticleAppearance({ ...item, opacity: 0.8 }, output, 3, updates)
    expect(updates[0]).toEqual({ start: Infinity, end: 0 })
    expect(updates[1]).toEqual({ start: Infinity, end: 0 })
    expect(updates[2]).toEqual({ start: Infinity, end: 0 })
    expect(updates[3]).toEqual({ start: 51, end: 64 })
    expect(output.colors[51]).toBe(Math.fround(0.8))
  })

  test("retains signed-zero sheet coordinates instead of collapsing distinct binary32 output", () => {
    const output = arrays(1)
    writeParticleAppearance({ ...item, primarySheet: { ...item.primarySheet, current: [[-0, 0, 1, 1]] } }, output, 0, createParticleAttributeUpdates())
    expect(Object.is(output.uv[0], -0)).toBe(true)
    const updates = createParticleAttributeUpdates()
    writeParticleAppearance({ ...item, primarySheet: { ...item.primarySheet, current: [[0, 0, 1, 1]] } }, output, 0, updates)
    expect(updates[0]).toEqual({ start: 0, end: 7 })
    expect(Object.is(output.uv[0], 0)).toBe(true)
  })
})
