import { describe, expect, test } from "bun:test"
import {
  createParticleSystem,
  decodeParticleRenderOutput,
  ParticleAdapterError,
  type ParticleKernel,
} from "../src"

function output(overrides: Readonly<{ count?: number; material?: number; radius?: number }> = {}): Uint8Array {
  const count = overrides.count ?? 1
  const bytes = new Uint8Array(40 + count * 436)
  const view = new DataView(bytes.buffer)
  view.setUint32(0, 0x5250_5350, true)
  view.setUint32(4, 3, true)
  view.setUint32(8, count, true)
  view.setUint32(12, 1, true)
  ;[-4, -5, -6, 4, 5, 6].forEach((value, component) => {
    view.setFloat32(16 + component * 4, value, true)
  })
  for (let index = 0; index < count; index += 1) {
    const offset = 40 + index * 436
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
    view.setInt32(offset + 120, 4, true)
    view.setUint32(offset + 124, 1, true)
    view.setFloat32(offset + 128, 0.25, true)
    for (let value = 0; value < 16; value += 1) {
      view.setFloat32(offset + 132 + value * 4, value / 16, true)
      view.setFloat32(offset + 196 + value * 4, (value + 1) / 17, true)
    }
    bytes[offset + 392] = 1
    bytes[offset + 393] = 0
    bytes[offset + 394] = 2
    bytes[offset + 395] = 3
    view.setFloat32(offset + 396, 0, true)
    view.setFloat32(offset + 400, 0.05, true)
    ;[4, 5, 6].forEach((value, component) => view.setFloat32(offset + 404 + component * 4, value, true))
    view.setFloat32(offset + 416, 2, true)
    view.setFloat32(offset + 420, 0.25, true)
    view.setBigUint64(offset + 424, 0x0102_0304_0506_0708n, true)
    view.setFloat32(offset + 432, 0.75, true)
  }
  return bytes
}

describe("Rust particle render-data adapter", () => {
  test("decodes bounded renderer-neutral sprite and trail records", () => {
    expect(decodeParticleRenderOutput(output(), ["effects/rocketrailsmoke.vmt"])).toEqual({
      bounds: { minimum: [-4, -5, -6], maximum: [4, 5, 6] },
      items: [{
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
        yawRadians: 0.75,
        color: 0x12_34_56,
        opacity: 0.75,
        sequence: 3,
        secondarySequence: 4,
        trailLength: 0.25,
        trailLengthScale: 0.25,
        trailEndPosition: [4, 5, 6],
        trailWidth: 2,
        sortKey: 100,
        ageSeconds: 0.5,
        lifetimeSeconds: 2,
        animationRate: 0.10000000149011612,
        secondaryAnimationRate: 0,
        stepSeconds: 0.05000000074505806,
        trailMinLength: 0,
        trailMaxLength: 2_000,
        trailFadeInSeconds: 0.20000000298023224,
        orientationType: 0,
        animationFitLifetime: true,
        animationRateAsFps: true,
        materialShader: "mesh-sprite",
        textureColorSpace: "srgb-texture-linear-tint",
        blendSource: "source-alpha",
        blendDestination: "one-minus-source-alpha",
        stableTieIdentity: 0x0102_0304_0506_0708n,
        primarySheet: {
          current: [
            [0, 0.0625, 0.125, 0.1875],
            [0.25, 0.3125, 0.375, 0.4375],
            [0.5, 0.5625, 0.625, 0.6875],
            [0.75, 0.8125, 0.875, 0.9375],
          ],
          next: [
            [0.05882352963089943, 0.11764705926179886, 0.1764705926179886, 0.23529411852359772],
            [0.29411765933036804, 0.3529411852359772, 0.4117647111415863, 0.47058823704719543],
            [0.529411792755127, 0.5882353186607361, 0.6470588445663452, 0.7058823704719543],
            [0.7647058963775635, 0.8235294222831726, 0.8823529481887817, 0.9411764740943909],
          ],
          blend: 0.25,
        },
        secondarySheet: null,
      }],
    })
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
    const bounds = output()
    new DataView(bounds.buffer).setUint32(12, 2, true)
    expect(() => decodeParticleRenderOutput(bounds, ["effects/only.vmt"]))
      .toThrow(ParticleAdapterError)
    const blend = output()
    blend[40 + 394] = 4
    expect(() => decodeParticleRenderOutput(blend, ["effects/only.vmt"]))
      .toThrow(ParticleAdapterError)
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
    expect(particles.advance({ bytes: Uint8Array.of(4, 5) }).items).toHaveLength(1)
    particles.reset(Uint8Array.of(6))
    particles.dispose()
    particles.dispose()
    expect(calls).toEqual(["load:1:3", "advance:2", "reset:1", "dispose"])
  })
})
