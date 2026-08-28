/** Local correctness reference only; no production branch or upload suppression. */
export function instrumentParticleAliasSource(source: string, reference: boolean, register = true): string {
  const decision = /const alias = texture\.frameCount === 1 \? particleTextureAlias\(candidate, textures\.values\(\)\) : (?:undefined|void 0);?/gu
  if ([...source.matchAll(decision)].length !== 1) throw new Error("Particle alias owner route differs")
  if (reference) source = source.replace(decision, "const alias = undefined;")
  if (!register) return source
  const geometry = /(const geometry = this\.#createParticleBatchGeometry\(1\);\s*disposables\.add\(geometry\);?)/gu
  if ([...source.matchAll(geometry)].length !== 1) throw new Error("Particle material registration route differs")
  return source.replace(geometry, "$1\nglobalThis.__playsrcParticleAliasEvidence?.register(texture.material.toLowerCase(), material, value, geometry, current, next, depth);")
}
