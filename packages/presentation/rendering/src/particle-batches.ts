export type ParticleBatchInput = Readonly<{
  material: string
  blendSource: string
  blendDestination: string
}>

export type ParticleBatchRange = Readonly<{ start: number; end: number }>

export function particleBatchRanges(items: readonly ParticleBatchInput[]): readonly ParticleBatchRange[] {
  const output: ParticleBatchRange[] = []
  let start = 0
  while (start < items.length) {
    const first = items[start]!
    let end = start + 1
    while (
      end < items.length
      && items[end]!.material.toLowerCase() === first.material.toLowerCase()
      && items[end]!.blendSource === first.blendSource
      && items[end]!.blendDestination === first.blendDestination
    ) {
      end += 1
    }
    output.push(Object.freeze({ start, end }))
    start = end
  }
  return Object.freeze(output)
}
