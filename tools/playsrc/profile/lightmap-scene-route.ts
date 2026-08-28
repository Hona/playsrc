/** Local correctness oracle only. No runtime switch or production import. */
export function instrumentLightmapSceneSource(source: string, reference: boolean): string {
  if (reference) {
    const borrow = /const borrowedLightmap = borrowWorldLightmapTextures\(lightmap, retained\?\.lightmapTextures\);?/gu
    if ([...source.matchAll(borrow)].length !== 1) throw new Error("Fresh-lightmap reference route differs from the checked owner")
    source = source.replace(borrow, "const borrowedLightmap = undefined;")
  }
  const pattern = /(const lightmapTextures = [^\n]*createWorldLightmapTextures\(lightmap, disposables\);?)/gu
  if ([...source.matchAll(pattern)].length !== 1) throw new Error("Lightmap registration route differs from the checked scene owner")
  return source.replace(pattern, "$1\nglobalThis.__playsrcLightmapEvidence?.register(lightmapTextures);")
}
