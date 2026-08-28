import { expect, test } from "bun:test"
import { verifyParticleCompilerParity } from "./fixtures/particle-compiler-parity"
import { ParticleMaterialGraphs, particleGraphKey } from "../src/particle-material-graphs"
import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import { createSourceWaterFogUniforms } from "../src/source-water"

const state = { cull: 0, depthTest: true, depthWrite: false, fog: 1, alphaModulation: 1, blendEnabled: true,
  alphaOwnership: { opacity: false }, fragmentDiscard: { kind: "none" } } as any

test("equivalent particle planes share compiler state but not alternating draw values", () => {
  const inputs = ["first", "second", "third"].map(material => ({ material, state, sourceSha256: material,
    width: 4, height: 4, sourceFormat: 0, scalarEncoding: "u8", spriteCard: null, additiveSprite: null }))
  const result = verifyParticleCompilerParity(inputs, false)
  expect(result.draws).toBe(3)
  expect(result.compilerStates).toBe(1)
  expect(result.graphFamilies).toBe(1)
  expect(result.warmBuilds).toBe(0)
})

test("particle shader inputs and texture interpretation remain structural, owners stay isolated", () => {
  const material = new THREE.MeshBasicNodeMaterial(), texture = new THREE.Texture()
  const input = { texture, state, waterFog: createSourceWaterFogUniforms(), depth: TSL.vec4(1), exposure: TSL.float(1), hdr: false,
    fog: { start: TSL.float(0), end: TSL.float(100), enabled: TSL.float(1), maximumDensity: TSL.float(1) } }
  const first = new ParticleMaterialGraphs(), second = new ParticleMaterialGraphs(), key = particleGraphKey(input)
  expect(first.get(material, input)).not.toBe(second.get(material, input))
  expect(particleGraphKey({ ...input, texture: new THREE.Texture() })).toBe(key)
  expect(particleGraphKey({ ...input, state: { ...state, alphaModulation: .5 } })).not.toBe(key)
  expect(particleGraphKey({ ...input, state: { ...state, fragmentDiscard: { kind: "alpha", pass: "greater", reference: .5 } } })).not.toBe(key)
  texture.flipY = !texture.flipY
  expect(particleGraphKey(input)).not.toBe(key)
  texture.dispose(); material.dispose()
})
