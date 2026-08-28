import { test, expect } from "bun:test"
import { encodeLegacyParticleFrame, createParticleBatchEncoder } from "../../src/presentation"
import { createClientRenderFrameClock } from "../../src/client-render-frame"

const camera = { position: [1, 2, 3] as const, yawDegrees: 90, pitchDegrees: 5, verticalFovDegrees: 75, aspectRatio: 16 / 9 }
test("legacy frames acknowledge only accepted draws, independent of publication ticks", () => {
  const clock = createClientRenderFrameClock()
  const first = clock.prepare(10), bytes = encodeLegacyParticleFrame(1, first, camera)
  const header = new DataView(bytes.buffer)
  expect(bytes.byteLength).toBe(64)
  expect(header.getUint32(4, true)).toBe(5)
  expect(header.getUint32(28, true)).toBe(0x8000_0000)
  expect(header.getFloat32(32, true)).toBe(0)
  expect(header.getUint32(60, true)).toBe(0)
  first.accept()
  const abandoned = clock.prepare(10.016)
  const third = clock.prepare(10.05)
  const view = new DataView(encodeLegacyParticleFrame(1.015, third, camera).buffer)
  expect(view.getFloat32(32, true)).toBe(Math.fround(0.05))
  expect(view.getUint32(36, true)).toBe(1)
  expect(view.getUint32(40, true)).toBe(2)
  expect(view.getFloat32(44, true)).toBe(90)
  expect(() => abandoned.accept()).toThrow()
  third.accept()
  expect(() => first.accept()).toThrow()
  const stalled = clock.prepare(10.35)
  expect(Math.fround(stalled.clientFrameSeconds)).toBe(Math.fround(0.3))
  clock.suspend()
  stalled.accept() // A submitted GPU frame may finish during pause.
  expect(clock.prepare(40).clientFrameSeconds).toBe(0)
  const pcf = createParticleBatchEncoder().encode(1n, [1, 2, 3], [])
  expect(new DataView(pcf.buffer).getUint32(28, true)).toBe(0)
})

test("malformed client-frame times do not consume identities", () => {
  const clock = createClientRenderFrameClock()
  expect(() => clock.prepare(Number.NaN)).toThrow()
  const frame = clock.prepare(10)
  expect(frame.clientFrame).toBe(1)
  frame.accept()
  expect(() => clock.prepare(9)).toThrow()
  expect(() => encodeLegacyParticleFrame(1, clock.prepare(10), { ...camera, aspectRatio: 0 })).toThrow()
})

test("one legacy frame envelope owns its separately framed visual payload", () => {
  const frame = createClientRenderFrameClock().prepare(10)
  const payload = new Uint8Array([0x50, 0x4c, 0x56, 0x51, 1, 2, 3, 4])
  const bytes = encodeLegacyParticleFrame(1, frame, camera, payload)
  expect(new DataView(bytes.buffer).getUint32(60, true)).toBe(payload.byteLength)
  expect(bytes.subarray(64)).toEqual(payload)
  payload.fill(0)
  expect(bytes[64]).toBe(0x50)
})
