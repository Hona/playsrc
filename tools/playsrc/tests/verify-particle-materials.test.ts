import { expect, test } from "bun:test"
import type { StaticMaterialState } from "../../../games/tf2/browser/src/artifacts"
import { verifyParticleMaterials } from "../src/verify-particle-materials"

test("particle verification binds the exact source inventory, source bytes and every state field", () => {
  const material = new TextEncoder().encode('"UnlitGeneric" { "$basetexture" "fixture" }')
  const texture = new Uint8Array([1, 2, 3])
  const hash = (bytes: Uint8Array) => new Bun.CryptoHasher("sha256").update(bytes).digest("hex")
  const resources = new Map([["materials/fixture.vmt", material], ["materials/fixture.vtf", texture]])
  const state: StaticMaterialState = {
    lighting: 0, blendEnabled: false, blendSource: 1, blendDestination: 0, alphaTest: false, cull: 0,
    depthTest: true, depthWrite: true, depthFunction: 1, polygonOffset: 0, fog: 0, wireframe: false,
    noDraw: false, vertexColor: false, vertexAlpha: false, translucentQueue: false,
    wrapS: 0, wrapT: 0, wrapU: 0, minFilter: 0, magFilter: 0, mipmapped: true, noLod: false,
    allMips: false, samplingAvailable: true, alphaTestReference: 0, alphaModulation: 1,
    alphaOwnership: { baseTextureAvailable: true, opacity: false, alphaTest: false, selfIlluminationMask: false,
      environmentMask: false, phongMask: false, tintMask: false, vertexAlpha: false, materialAlphaModulation: false },
    fragmentDiscard: { kind: "none", source: "base-texture-or-one", pass: "greater", reference: 0 },
  }
  const expected = [{ identity: "fixture.vmt", materialSha256: hash(material), texture: "materials/fixture.vtf", textureSha256: hash(texture), spriteCard: false, state }]
  const artifacts = { particleMaterials: ["fixture.vmt"], particleTextures: [{ material: "fixture.vmt", logicalPath: "materials/fixture.vtf", sourceSha256: hash(texture) }], materialStates: new Map([["fixture.vmt", state]]) }
  expect(() => verifyParticleMaterials(expected, artifacts, resources)).not.toThrow()
  expect(() => verifyParticleMaterials(expected, { ...artifacts, particleMaterials: [] }, resources)).toThrow("inventory")
  expect(() => verifyParticleMaterials(expected, { ...artifacts, particleMaterials: ["fixture.vmt", "extra.vmt"] }, resources)).toThrow("inventory")
  expect(() => verifyParticleMaterials(expected, { ...artifacts, materialStates: new Map([["fixture.vmt", { ...state, depthWrite: false }]]) }, resources)).toThrow("state differs")
  expect(() => verifyParticleMaterials(expected, { ...artifacts, materialStates: new Map() }, resources)).toThrow("state differs")
  expect(() => verifyParticleMaterials(expected, { ...artifacts, particleTextures: [] }, resources)).toThrow("texture inventory")
  expect(() => verifyParticleMaterials(expected, { ...artifacts, particleTextures: [{ ...artifacts.particleTextures[0]!, logicalPath: "wrong.vtf" }] }, resources)).toThrow("texture source differs")
  texture[0] = 4
  expect(() => verifyParticleMaterials(expected, artifacts, resources)).toThrow("source differs")
})
