/** Local reference only: restore the old sampled-before-attachment admission order. */
export function instrumentWaterTargetSceneSource(source: string, reference: boolean): string {
  const targets = /[ \t]*if \(scene\.reflectionTarget\)\s+this\.#backend\.initRenderTarget\(scene\.reflectionTarget\);?\s+if \(scene\.refractionTarget\)\s+this\.#backend\.initRenderTarget\(scene\.refractionTarget\);?\n/gu
  const matches = [...source.matchAll(targets)]
  if (matches.length !== 1) throw new Error("Water target initialization owner differs")
  const warmup = source.search(/async\s*#prepareReachablePipelines\(/u), water = source.search(/async\s*#prepareWaterPipelines\(/u)
  if (matches[0]!.index! < warmup || matches[0]!.index! > water) throw new Error("Water attachment admission is not before sampled pipeline compilation")
  if (!reference) return source
  const anchor = /[ \t]*const initializedTextures = new Set(?:\(\))?;?/gu
  if ([...source.matchAll(anchor)].length !== 1) throw new Error("Water reference admission route differs")
  return source.replace(targets, "").replace(anchor, value => matches[0]![0] + value)
}
