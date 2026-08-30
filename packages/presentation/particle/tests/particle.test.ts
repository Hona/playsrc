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
  view.setUint32(4, 5, true)
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

test("particle pass ownership distinguishes real sky particles and rejects unknown ownership", () => {
  const bytes = output(); bytes[55] = 1
  expect(decodeParticleRenderOutput(bytes, ["smoke"]).items[0]!.sky).toBe(true)
  bytes[55] = 2
  expect(() => decodeParticleRenderOutput(bytes, ["smoke"])).toThrow()
  bytes[55] = 0; new DataView(bytes.buffer).setUint32(4, 3, true)
  expect(() => decodeParticleRenderOutput(bytes, ["smoke"])).toThrow("identity is invalid")
})

test("retains authored reversed rain trail bounds instead of rejecting Source's upper-bound clamp", () => {
  const bytes = output(), view = new DataView(bytes.buffer)
  view.setFloat32(40 + 100, 22, true); view.setFloat32(40 + 104, 20, true)
  const item = decodeParticleRenderOutput(bytes, ["rain"]).items[0]!
  expect([item.trailMinLength, item.trailMaxLength]).toEqual([22, 20])
})

test("repeated sheet rectangles share frozen values without borrowing packet bytes or merging signed zero", () => {
  const bytes = output({ count: 3 }), view = new DataView(bytes.buffer)
  view.setUint32(40 + 2 * 436 + 132, 0x80000000, true)
  const decoded = decodeParticleRenderOutput(bytes, ["smoke"])
  const [first, second, third] = decoded.items.map(item => item.primarySheet!)
  expect(first!.current).toBe(second!.current)
  expect(first!.next).toBe(second!.next)
  expect(third!.current).not.toBe(first!.current)
  expect(Object.is(first!.current[0]![0], 0)).toBe(true)
  expect(Object.is(third!.current[0]![0], -0)).toBe(true)
  expect(Object.isFrozen(first!.current)).toBe(true)
  expect(first!.current.every(Object.isFrozen)).toBe(true)
  const retained = first!.next[0]![0]
  bytes.fill(0)
  expect(first!.next[0]![0]).toBe(retained)
})

test("packet-local UUID reuse preserves all128 bits, alternating systems and retained values", () => {
  const bytes = output({ count: 18 })
  for (let index = 1; index <= 16; index++) bytes[40 + index * 436 + 16 + index - 1] = index
  const expected = Array.from({ length: 18 }, (_, index) => Buffer.from(bytes.subarray(40 + index * 436 + 16, 40 + index * 436 + 32)).toString("hex"))
  const retained = decodeParticleRenderOutput(bytes, ["smoke"])
  expect(retained.items.map(item => item.systemUuid)).toEqual(expected)
  bytes.fill(0xff, 40 + 16, 40 + 32)
  expect(retained.items.map(item => item.systemUuid)).toEqual(expected)
  expect(decodeParticleRenderOutput(bytes, ["smoke"]).items[0]!.systemUuid).toBe("ff".repeat(16))
})

test("sheet cache bounds do not omit unique rectangles or accept a nonfinite late record", () => {
  const bytes = output({ count: 600 }), view = new DataView(bytes.buffer)
  for (let index = 0; index < 600; index++) view.setFloat32(40 + index * 436 + 132, index, true)
  expect(decodeParticleRenderOutput(bytes, ["smoke"]).items.at(-1)!.primarySheet!.current[0]![0]).toBe(599)
  view.setFloat32(40 + 599 * 436 + 132, NaN, true)
  expect(() => decodeParticleRenderOutput(bytes, ["smoke"])).toThrow("sheet rectangle is invalid")
})

test("sheet interning compares every binary32 word after a hash collision", () => {
  const bytes = output({ count: 2 }), view = new DataView(bytes.buffer)
  const first = 40 + 132, second = 40 + 436 + 132
  let hash = 0x811c9dc5
  for (let offset = 0; offset < 56; offset += 4) hash = Math.imul(hash ^ view.getUint32(first + offset, true), 0x01000193)
  const original = view.getUint32(first + 56, true), changed = original ^ 1
  const final = (Math.imul(hash ^ original, 0x01000193) ^ view.getUint32(first + 60, true) ^ Math.imul(hash ^ changed, 0x01000193)) >>> 0
  view.setUint32(second + 56, changed, true); view.setUint32(second + 60, final, true)
  expect(Number.isFinite(view.getFloat32(second + 60, true))).toBe(true)
  const items = decodeParticleRenderOutput(bytes, ["smoke"]).items
  expect(items[0]!.primarySheet!.current).not.toBe(items[1]!.primarySheet!.current)
  expect(items[1]!.primarySheet!.current[3]).toEqual([
    view.getFloat32(second + 48, true), view.getFloat32(second + 52, true), view.getFloat32(second + 56, true), view.getFloat32(second + 60, true),
  ])
})

describe("Rust particle render-data adapter", () => {
  test("decodes bounded indexed rope geometry and rejects incomplete or invalid tails", () => {
    const bytes = new Uint8Array(40 + 436 + 8 + 4 * 24 + 6 * 4)
    bytes.set(output()); bytes[54] = 2
    const view = new DataView(bytes.buffer), at = 40 + 436
    view.setUint32(at, 4, true); view.setUint32(at + 4, 6, true)
    for (let index = 0; index < 12; index++) view.setFloat32(at + 8 + index * 4, index / 2, true)
    bytes.fill(127, at + 8 + 80, at + 8 + 96)
    ;[0, 1, 2, 1, 3, 2].forEach((value, index) => view.setUint32(at + 8 + 96 + index * 4, value, true))
    const item = decodeParticleRenderOutput(bytes, ["effects/beam.vmt"]).items[0]!
    expect(item.primitive).toBe("rope")
    expect([...item.mesh!.positions]).toEqual(Array.from({ length: 12 }, (_, index) => index / 2))
    expect([...item.mesh!.indices]).toEqual([0, 1, 2, 1, 3, 2])
    expect([...item.mesh!.colors]).toEqual(Array(16).fill(127))
    expect(() => decodeParticleRenderOutput(bytes.subarray(0, bytes.length - 1), ["effects/beam.vmt"])).toThrow()
    view.setUint32(bytes.length - 4, 4, true)
    expect(() => decodeParticleRenderOutput(bytes, ["effects/beam.vmt"])).toThrow()
    view.setUint32(bytes.length - 4, 2, true); view.setFloat32(at + 8, NaN, true)
    expect(() => decodeParticleRenderOutput(bytes, ["effects/beam.vmt"])).toThrow()
    view.setFloat32(at + 8, 0, true); view.setUint32(at, 0xffff_ffff, true)
    expect(() => decodeParticleRenderOutput(bytes, ["effects/beam.vmt"])).toThrow()
  })
  test("decodes bounded renderer-neutral sprite and trail records", () => {
    expect(decodeParticleRenderOutput(output(), ["effects/rocketrailsmoke.vmt"])).toEqual({
      bounds: { minimum: [-4, -5, -6], maximum: [4, 5, 6] },
      items: [{
        sky: false,
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
