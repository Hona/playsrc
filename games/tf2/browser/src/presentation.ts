import type { ParticleRenderItem } from "@playsrc/particle"
import type { Effect } from "@playsrc/rendering"
import type { Snapshot } from "./codec"

export type PresentationDiagnostic = Readonly<{
  code: "MissingProjectileModel" | "MissingParticleContext" | "MissingAudioContext"
  identity: string
}>

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
