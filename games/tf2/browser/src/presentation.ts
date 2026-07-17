import type { ParticleRenderItem } from "@playsrc/particle"
import type { Camera, Effect, ModelItem } from "@playsrc/rendering"
import type { Snapshot } from "./codec"

export type PresentationDiagnostic = Readonly<{
  code: "MissingProjectileModel" | "MissingParticleContext" | "MissingAudioContext"
  identity: string
}>

export type Tf2Hud = Readonly<{
  health: number
  maxHealth: number
  className: "Soldier" | "Demoman"
  weaponName: "Rocket Launcher" | "Original" | "Stickybomb Launcher"
  speed: number
  projectileCount: number
}>
export type Tf2AudioRequest = Readonly<{
  voice: number
  resource: string
  gain: number
  pan: number
  loop: false
}>

export function tf2Audio(events: Snapshot["events"]): readonly Tf2AudioRequest[] {
  const requests: Tf2AudioRequest[] = []
  for (const event of events) {
    let resource: string | undefined
    if (event.kind === 3) {
      resource = event.detail === 1
        ? "sound/weapons/rocket_shoot.wav"
        : event.detail === 2
          ? "sound/weapons/stickybomblauncher_shoot.wav"
          : undefined
    } else if (event.kind === 4 && (event.detail === 1 || event.detail === 2)) {
      const ordinal = event.subject % 3 + 1
      resource = event.detail === 1
        ? `sound/weapons/explode${ordinal}.wav`
        : `sound/weapons/pipe_bomb${ordinal}.wav`
    }
    if (resource) {
      requests.push(Object.freeze({
        voice: event.subject * 2 + event.kind,
        resource,
        gain: 1,
        pan: 0,
        loop: false,
      }))
    }
  }
  return Object.freeze(requests)
}

export function tf2Hud(snapshot: Snapshot): Tf2Hud {
  return Object.freeze({
    health: snapshot.health,
    maxHealth: snapshot.class === 1 ? 200 : 175,
    className: snapshot.class === 1 ? "Soldier" : "Demoman",
    weaponName: snapshot.weapon === 1
      ? "Rocket Launcher"
      : snapshot.weapon === 2
        ? "Original"
        : "Stickybomb Launcher",
    speed: Math.hypot(...snapshot.velocity),
    projectileCount: snapshot.projectiles.length,
  })
}

export function tf2Camera(snapshot: Snapshot, yawDegrees: number, pitchDegrees: number): Camera {
  return Object.freeze({
    position: Object.freeze([
      snapshot.position[0],
      snapshot.position[1],
      snapshot.position[2] + (snapshot.crouched ? 45 : 68),
    ]) as readonly [number, number, number],
    yawDegrees,
    pitchDegrees,
    verticalFovDegrees: 74,
    near: 1,
    far: 32_768,
  })
}

export function tf2Presentation(
  snapshot: Snapshot,
  particleItems: readonly ParticleRenderItem[],
  debugMissingProjectileModels: boolean,
): Readonly<{
  effects: readonly Effect[]
  models: readonly ModelItem[]
  diagnostics: readonly PresentationDiagnostic[]
}> {
  const effects: Effect[] = particleItems.map((item) => Object.freeze({ ...item }))
  const models: ModelItem[] = []
  const diagnostics: PresentationDiagnostic[] = []
  models.push(Object.freeze({
    identity: 0x7fff_0000 + snapshot.weapon,
    model: snapshot.class === 1
      ? "models/weapons/v_models/v_rocketlauncher_soldier.mdl"
      : "models/weapons/v_models/v_stickybomb_launcher_demo.mdl",
    position: Object.freeze([0, 0, 0]) as readonly [number, number, number],
    angles: Object.freeze([0, 0, 0]) as readonly [number, number, number],
    scale: 1,
    viewModel: true,
  }))
  for (const projectile of snapshot.projectiles) {
    const model = projectile.kind === 1
      ? "models/weapons/w_models/w_rocket.mdl"
      : "models/weapons/w_models/w_stickybomb.mdl"
    const speed = Math.hypot(...projectile.velocity)
    const yaw = speed > 0.001
      ? Math.atan2(projectile.velocity[1], projectile.velocity[0]) * 180 / Math.PI
      : 0
    const pitch = speed > 0.001
      ? -Math.asin(projectile.velocity[2] / speed) * 180 / Math.PI
      : 0
    models.push(Object.freeze({
      identity: projectile.id,
      model,
      position: projectile.position,
      angles: Object.freeze([pitch, yaw, 0]) as readonly [number, number, number],
      scale: 1,
    }))
    if (debugMissingProjectileModels) {
      diagnostics.push(Object.freeze({ code: "MissingProjectileModel", identity: model }))
      effects.push(Object.freeze({
        identity: projectile.id,
        position: projectile.position,
        radius: projectile.kind === 1 ? 2 : 3,
        color: projectile.kind === 1 ? 0xff7a1a : 0x7b4bb7,
        opacity: 1,
      }))
    }
  }
  for (const event of snapshot.events) {
    if (event.kind === 3) {
      diagnostics.push(Object.freeze({
        code: "MissingAudioContext",
        identity: `projectile-fire:${event.subject}`,
      }))
    } else if (event.kind === 4) {
      diagnostics.push(Object.freeze({
        code: "MissingParticleContext",
        identity: `projectile-explosion:${event.subject}`,
      }))
      diagnostics.push(Object.freeze({
        code: "MissingAudioContext",
        identity: `projectile-explosion:${event.subject}`,
      }))
    }
  }
  diagnostics.sort((left, right) => left.code.localeCompare(right.code) || left.identity.localeCompare(right.identity))
  return Object.freeze({
    effects: Object.freeze(effects),
    models: Object.freeze(models),
    diagnostics: Object.freeze(diagnostics),
  })
}
