// A sampled render-target texture must not be allocated first by a material
// binding and then replaced by updateRenderTarget in a later pass. Initialize
// the complete attachment generation before any pass can encode a consumer.
export function resizeSampledRenderTargets<T extends { setSize(width: number, height: number): unknown }>(
  targets: readonly (T | null)[],
  width: number,
  height: number,
  initialize: (target: T) => void,
): void {
  for (const target of targets) {
    if (!target) continue
    target.setSize(width, height)
    initialize(target)
  }
}
