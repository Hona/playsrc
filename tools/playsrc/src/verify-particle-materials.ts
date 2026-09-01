import { isDeepStrictEqual } from "node:util"
import type { PresentationArtifacts, StaticMaterialState } from "../../../games/tf2/browser/src/artifacts"

export type ParticleMaterialExpectation = Readonly<{
  identity: string
  materialSha256: string
  texture: string
  textureSha256: string
  spriteCard: boolean
  state: StaticMaterialState
}>

export function verifyParticleMaterials(
  expectations: readonly ParticleMaterialExpectation[],
  artifacts: Pick<PresentationArtifacts, "particleMaterials" | "materialStates"> & { particleTextures: readonly { material: string; logicalPath: string; sourceSha256: string }[] },
  resources: ReadonlyMap<string, Uint8Array>,
): void {
  const identities = expectations.map(row => row.identity).sort()
  if (!identities.length || new Set(identities).size !== identities.length || !isDeepStrictEqual(identities, [...artifacts.particleMaterials].sort())) {
    throw new Error("Particle material inventory differs from configured PCF renderer/lifetime dependencies")
  }
  const textures = artifacts.particleTextures.map(texture => texture.material).sort()
  if (!isDeepStrictEqual(textures, identities)) throw new Error("Particle texture inventory differs from configured materials")
  for (const expected of expectations) {
    for (const [path, hash] of [[`materials/${expected.identity}`, expected.materialSha256], [expected.texture, expected.textureSha256]] as const) {
      const bytes = resources.get(path)
      if (!bytes || new Bun.CryptoHasher("sha256").update(bytes).digest("hex") !== hash) throw new Error(`Particle expectation source differs: ${path}`)
    }
    const texture = artifacts.particleTextures.find(texture => texture.material === expected.identity)!
    if (texture.logicalPath !== expected.texture || texture.sourceSha256 !== expected.textureSha256) throw new Error(`Particle texture source differs: ${expected.identity}`)
    const actual = artifacts.materialStates.get(expected.identity)
    if (!actual || !expected.state || !isDeepStrictEqual(actual, expected.state)) {
      throw new Error(`Particle material state differs from configured VMT/VTF: ${expected.identity}; expected=${JSON.stringify(expected.state)} actual=${JSON.stringify(actual)}`)
    }
  }
}
