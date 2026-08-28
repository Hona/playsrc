export type ParticleBatchInput = Readonly<{
  sky: boolean
  material: string
  blendSource: string
  blendDestination: string
  primitive?: string
}>

export type ParticleBatchRange = Readonly<{ start: number; end: number }>
export type MutableParticleBatchRange = { start: number; end: number }

export function fillParticleBatchRanges(
  items: readonly ParticleBatchInput[],
  output: MutableParticleBatchRange[],
): number {
  let start = 0
  let count = 0
  while (start < items.length) {
    const first = items[start]!
    const material = first.material.toLowerCase()
    let end = start + 1
    while (
      end < items.length
      && first.primitive !== "rope"
      && items[end]!.primitive !== "rope"
      && items[end]!.material.toLowerCase() === material
      && items[end]!.blendSource === first.blendSource
      && items[end]!.blendDestination === first.blendDestination
      && items[end]!.sky === first.sky
    ) end += 1
    const retained = output[count]
    if (retained) {
      retained.start = start
      retained.end = end
    } else {
      output[count] = { start, end }
    }
    count += 1
    start = end
  }
  output.length = count
  return count
}

export function particleBatchRanges(items: readonly ParticleBatchInput[]): readonly ParticleBatchRange[] {
  const output: MutableParticleBatchRange[] = []
  fillParticleBatchRanges(items, output)
  return Object.freeze(output.map((range) => Object.freeze(range)))
}
