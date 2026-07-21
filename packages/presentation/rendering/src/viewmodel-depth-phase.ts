export type ViewModelDepthPhase = Readonly<{
  depthRange: readonly [number, number]
  worldDepthCleared: true
  depthRangeRestored: true
}>

export function executeViewModelDepthPhase(request: Readonly<{
  depthRange: readonly [number, number]
  clearWorldDepth(): void
  setDepthRange(depthRange: readonly [number, number]): void
  draw(): void
}>): ViewModelDepthPhase {
  const [minimum, maximum] = request.depthRange
  if (!Number.isFinite(minimum) || !Number.isFinite(maximum) || minimum < 0 || minimum >= maximum || maximum > 1) {
    throw new Error("viewmodel depth range is invalid")
  }
  request.clearWorldDepth()
  request.setDepthRange(request.depthRange)
  try {
    request.draw()
  } finally {
    request.setDepthRange([0, 1])
  }
  return Object.freeze({
    depthRange: Object.freeze([minimum, maximum]) as readonly [number, number],
    worldDepthCleared: true,
    depthRangeRestored: true,
  })
}
