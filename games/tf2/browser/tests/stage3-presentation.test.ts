import { expect, test } from "bun:test"
import { sourceHorizontal4By3FovToVertical } from "@playsrc/rendering"
import { createParticleBatchEncoder, createViewmodelPresenter, tf2Camera, type ProjectileParticleRequest } from "../src/presentation"
import type { PresentationArtifacts } from "../src/artifacts"
import type { Snapshot } from "../src/codec"

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

test("uses the default TF2 horizontal-4:3 world projection and Source clip planes", () => {
  const snapshot = {
    position: Object.freeze([10, 20, 30]),
    movement: { viewOffset: Object.freeze([0, 0, 68]) },
  } as unknown as Snapshot
  expect(tf2Camera(snapshot, 90, -10)).toEqual({
    position: [10, 20, 98],
    yawDegrees: 90,
    pitchDegrees: -10,
    verticalFovDegrees: sourceHorizontal4By3FovToVertical(75),
    near: 7,
    far: 28_377.919921875,
  })
})

test("joins the current team skin to the matching viewmodel template", () => {
  const identity = "models/weapons/v_models/v_rocketlauncher_soldier.mdl"
  const artifacts = {
    models: new Map([[identity, {
      identity,
      bodygroupCounts: Object.freeze([]),
      descriptor: Object.freeze({
        kind: "viewmodel",
        horizontalFov4By3: 54,
        minimumFov: 54,
        maximumFov: 70,
        near: 1,
        depthRange: Object.freeze([0, 0.1]),
        drawsAfterWorld: true,
        opaqueBeforeTranslucent: true,
        optionalViewSpaceYReflection: true,
      }),
      sequences: Object.freeze([
        { activity: "ACT_VM_DRAW", durationSeconds: 0.8 },
        { activity: "ACT_RELOAD_START", durationSeconds: 0.5 },
      ]),
    }]]),
  } as unknown as PresentationArtifacts
  const snapshot = (team: 1 | 2) => ({
    class: 1,
    team,
    tick: 1n,
    projectileEvents: Object.freeze([]),
    activities: Object.freeze([]),
    loadout: Object.freeze([{ weapon: 1, reload: 0, clip: 4 }]),
    weapon: 1,
    velocity: Object.freeze([0, 0, 0]),
  }) as unknown as Snapshot
  expect(createViewmodelPresenter(artifacts).map(snapshot(1)).item.skin).toBe(0)
  expect(createViewmodelPresenter(artifacts).map(snapshot(2)).item.skin).toBe(1)
  const reloading = {
    ...snapshot(1),
    activities: Object.freeze([{ tick: 1n, weapon: 1, activity: 3 }]),
  } as unknown as Snapshot
  expect(createViewmodelPresenter(artifacts).map(reloading).request.activity).toBe("ACT_RELOAD_START")
})
