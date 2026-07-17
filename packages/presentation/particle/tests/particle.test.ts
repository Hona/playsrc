import { describe, expect, test } from "bun:test"
import {
  createParticleSystem,
  decodeParticleRenderOutput,
  ParticleAdapterError,
  type ParticleKernel,
} from "../src"

function output(overrides: Readonly<{ count?: number; material?: number; radius?: number }> = {}): Uint8Array {
  const count = overrides.count ?? 1
  const bytes = new Uint8Array(12 + count * 120)
  const view = new DataView(bytes.buffer)
  view.setUint32(0, 0x5250_5350, true)
  view.setUint32(4, 1, true)
  view.setUint32(8, count, true)
  for (let index = 0; index < count; index += 1) {
    const offset = 12 + index * 120
    view.setUint32(offset, index + 1, true)
    view.setUint32(offset + 4, 7, true)
    view.setUint32(offset + 8, 11, true)
    view.setUint16(offset + 12, 2, true)
    bytes[offset + 14] = 1
    bytes.fill(0xab, offset + 16, offset + 32)
    view.setUint32(offset + 32, overrides.material ?? 0, true)
    ;[1, 2, 3, 0, 1, 2].forEach((value, component) => {
      view.setFloat32(offset + 36 + component * 4, value, true)
    })
    view.setFloat32(offset + 60, overrides.radius ?? 4, true)
    view.setFloat32(offset + 64, 0.5, true)
    view.setUint32(offset + 68, 0x12_34_56, true)
    view.setFloat32(offset + 72, 0.75, true)
    view.setInt32(offset + 76, 3, true)
    view.setFloat32(offset + 80, 0.25, true)
    view.setFloat32(offset + 84, 100, true)
    view.setFloat32(offset + 88, 0.5, true)
    view.setFloat32(offset + 92, 2, true)
    view.setFloat32(offset + 96, 0.1, true)
    view.setFloat32(offset + 100, 0, true)
    view.setFloat32(offset + 104, 2_000, true)
    view.setFloat32(offset + 108, 0.2, true)
    view.setInt32(offset + 112, 0, true)
    view.setUint32(offset + 116, 3, true)
  }
  return bytes
}

describe("Rust particle render-data adapter", () => {
  test("decodes bounded renderer-neutral sprite and trail records", () => {
    expect(decodeParticleRenderOutput(output(), ["effects/rocketrailsmoke.vmt"])).toEqual([{
      identity: 1,
      effectIdentity: 7,
      particleIdentity: 11,
      rendererIndex: 2,
      primitive: "trail",
      systemUuid: "ab".repeat(16),
      material: "effects/rocketrailsmoke.vmt",
      position: [1, 2, 3],
      previousPosition: [0, 1, 2],
      radius: 4,
      rollRadians: 0.5,
      color: 0x12_34_56,
      opacity: 0.75,
      sequence: 3,
      trailLength: 0.25,
      sortKey: 100,
      ageSeconds: 0.5,
      lifetimeSeconds: 2,
      animationRate: 0.10000000149011612,
      trailMinLength: 0,
      trailMaxLength: 2_000,
      trailFadeInSeconds: 0.20000000298023224,
      orientationType: 0,
      animationFitLifetime: true,
      animationRateAsFps: true,
    }])
  })

  test("rejects malformed output atomically", () => {
    expect(() => decodeParticleRenderOutput(output({ material: 1 }), ["effects/only.vmt"]))
      .toThrow(ParticleAdapterError)
    expect(() => decodeParticleRenderOutput(output({ radius: Number.NaN }), ["effects/only.vmt"]))
      .toThrow(ParticleAdapterError)
    expect(() => decodeParticleRenderOutput(output({ count: 2 }), ["effects/only.vmt"], {
      maxOutputBytes: 1_024,
      maxRenderItems: 1,
    })).toThrow(ParticleAdapterError)
  })

  test("passes complete PCF and advancement phases to one kernel session", () => {
    const calls: string[] = []
    const kernel: ParticleKernel = Object.freeze({
      load(resources) {
        calls.push(`load:${resources.length}:${resources[0]?.bytes.byteLength}`)
        return Object.freeze({
          materials: Object.freeze(["effects/rocketrailsmoke.vmt"]),
          transact(bytes: Uint8Array) {
            calls.push(`advance:${bytes.byteLength}`)
            return output()
          },
          reset(bytes: Uint8Array) {
            calls.push(`reset:${bytes.byteLength}`)
          },
          dispose() {
            calls.push("dispose")
          },
        })
      },
    })
    const particles = createParticleSystem(kernel, [{
      logicalPath: "particles/rockettrail.pcf",
      bytes: Uint8Array.of(1, 2, 3),
    }])
    expect(particles.advance({ bytes: Uint8Array.of(4, 5) })).toHaveLength(1)
    particles.reset(Uint8Array.of(6))
    particles.dispose()
    particles.dispose()
    expect(calls).toEqual(["load:1:3", "advance:2", "reset:1", "dispose"])
  })
})
