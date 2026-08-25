import { sourceDepthBias } from "./material-state"

export type ProjectedDecalBias = Readonly<{
  enabled: boolean
  slopeScale: number
  units: number
}>

export function projectedDecalDepthBias(category: "none" | "decal"): ProjectedDecalBias {
  const configured = sourceDepthBias(category)
  if (!configured.enabled) return configured
  return Object.freeze({
    enabled: true,
    slopeScale: configured.slopeScale === 0 ? 0 : 1 / configured.slopeScale,
    units: configured.units === 0 ? 0 : Math.round((2 ** 24 - 1) / configured.units),
  })
}

export type ProjectedDecalReceiver = Readonly<{
  entity: bigint | null
  model: number
}>

export type ProjectedDecalFragment = Readonly<{
  model: number
  face: number
  visibility:
    | Readonly<{ kind: "world" }>
    | Readonly<{ kind: "brush-model"; entity: bigint; model: number }>
}>

export type ProjectedDecalInput = Readonly<{
  kind: number
  receiver: ProjectedDecalReceiver | null
  targetFaces: readonly number[]
  renderOrder: number
  fragments: readonly ProjectedDecalFragment[]
}>

export function projectedDecalReceiverIsValid(mark: ProjectedDecalInput): boolean {
  if (!Number.isSafeInteger(mark.renderOrder) || mark.renderOrder < 0 || mark.renderOrder > 3) return false
  if (mark.kind === 0 && mark.receiver === null) return false
  const faces = new Set<number>()
  for (const fragment of mark.fragments) {
    if (!Number.isSafeInteger(fragment.face) || fragment.face < 0) return false
    faces.add(fragment.face)
    if (fragment.visibility.kind === "world") {
      if (fragment.model !== 0 || (mark.receiver !== null && (mark.receiver.model !== 0 || mark.receiver.entity !== null))) {
        return false
      }
    } else if (
      !mark.receiver
      || mark.receiver.entity !== fragment.visibility.entity
      || mark.receiver.model !== fragment.visibility.model
      || fragment.model !== mark.receiver.model
    ) {
      return false
    }
  }
  if (faces.size !== mark.targetFaces.length) return false
  let index = 0
  for (const face of faces) {
    if (mark.targetFaces[index++] !== face) return false
  }
  return true
}
