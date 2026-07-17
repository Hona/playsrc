import { expect, test } from "bun:test"
import { createParticleBatchEncoder, type ProjectileParticleRequest } from "../src/presentation"

test("encodes one bounded complete PCF phase without per-particle calls", () => {
  const request: ProjectileParticleRequest = Object.freeze({
    kind: "start",
    identity: "7:0:fire:9:start:projectile:9:trail",
    effectIdentity: "projectile:9:trail",
    eventIdentity: "7:0:fire:9",
    projectileIdentity: 9,
    ownerIdentity: 1,
    launcherIdentity: 2,
    team: "red",
    system: "rockettrail",
    attachment: Object.freeze({ entityIdentity: 9, name: "trail" }),
    controlPoints: Object.freeze([
      Object.freeze({
        index: 0,
        position: Object.freeze([1, 2, 3]),
        orientation: Object.freeze([0, 0, 0, 1]),
        ownerIdentity: 1,
      }),
    ]),
  })
  const bytes = createParticleBatchEncoder().encode(7n, [4, 5, 6], [request])
  const view = new DataView(bytes.buffer)
  expect(new TextDecoder().decode(bytes.subarray(0, 4))).toBe("PPTX")
  expect(view.getUint32(4, true)).toBe(1)
  expect(view.getUint32(28, true)).toBe(1)
  expect(bytes[32]).toBe(1)
  expect(new TextDecoder().decode(bytes.subarray(68, 79))).toBe("rockettrail")
})
