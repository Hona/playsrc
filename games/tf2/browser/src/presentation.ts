import type { ParticleRenderItem } from "@playsrc/particle"
import type { Camera, Effect } from "@playsrc/rendering"
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
}>

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
): Readonly<{ effects: readonly Effect[]; diagnostics: readonly PresentationDiagnostic[] }> {
  const effects: Effect[] = particleItems.map((item) => Object.freeze({ ...item }))
  const diagnostics: PresentationDiagnostic[] = []
  for (const projectile of snapshot.projectiles) {
    const model = projectile.kind === 1
      ? "models/weapons/w_models/w_rocket.mdl"
      : "models/weapons/w_models/w_stickybomb.mdl"
    diagnostics.push(Object.freeze({ code: "MissingProjectileModel", identity: model }))
    if (debugMissingProjectileModels) {
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
  return Object.freeze({ effects: Object.freeze(effects), diagnostics: Object.freeze(diagnostics) })
}
