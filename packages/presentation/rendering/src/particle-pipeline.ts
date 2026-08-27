import type { ParticleItem, MaterialStateInput } from "./index"
import * as THREE from "three/webgpu"

const FACTORS = ["zero", "one", "source-alpha", "one-minus-source-alpha"] as const

/** Both the particle output and PMST state are emitted from the same Rust
 * material compilation. No Cartesian product of blend modes is reachable. */
export function particlePipelineVariant(material: string, state: MaterialStateInput): Pick<ParticleItem, "material" | "blendSource" | "blendDestination"> {
  const blendSource = FACTORS[state.blendSource], blendDestination = FACTORS[state.blendDestination]
  if (!blendSource || !blendDestination) throw new Error(`Unsupported particle blend state: ${material}`)
  return { material: material.toLowerCase(), blendSource, blendDestination }
}

export function particlePipelineKey(item: Pick<ParticleItem, "material" | "blendSource" | "blendDestination">): string {
  return `${item.material.toLowerCase()}\0${item.blendSource}\0${item.blendDestination}`
}

/** Three draws translucent double-sided surfaces back then front. Its async
 * queue retains a material reference after that temporary side is restored;
 * preparation must instead retain the two actual side states until settlement. */
export function particlePreparationSides(material: THREE.Material): readonly THREE.Side[] {
  return material.transparent && material.side === THREE.DoubleSide && !material.forceSinglePass
    ? [THREE.BackSide, THREE.FrontSide] : [material.side]
}
