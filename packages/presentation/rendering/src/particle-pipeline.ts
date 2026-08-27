import type { ParticleItem, MaterialStateInput } from "./index"

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
