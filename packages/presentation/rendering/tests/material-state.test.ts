import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { applyParticleDepthState, configureWorldLightmap, sourceDepthBias, sourceFragmentUsesAlpha, worldMaterialSide } from "../src/material-state"

test("world materials keep front-face culling unless no-cull is explicit", () => {
  expect(worldMaterialSide(0)).toBe(THREE.FrontSide)
  expect(worldMaterialSide(8)).toBe(THREE.DoubleSide)
})

test("categorical decal bias maps to the fixed WebGPU adapter", () => {
  expect(sourceDepthBias("none")).toEqual({ enabled: false, slopeScale: 0, units: 0 })
  expect(sourceDepthBias("decal")).toEqual({ enabled: true, slopeScale: -0.5, units: -262_144 })
})

test("opaque Source surfaces ignore non-opacity texture alpha while translucent and fading draws retain it", () => {
  expect(sourceFragmentUsesAlpha({ blendEnabled: false, alphaOwnership: { opacity: false } })).toBe(false)
  expect(sourceFragmentUsesAlpha({ blendEnabled: false, alphaOwnership: { opacity: true } })).toBe(true)
  expect(sourceFragmentUsesAlpha({ blendEnabled: true, alphaOwnership: { opacity: false } })).toBe(true)
  expect(sourceFragmentUsesAlpha({ blendEnabled: false, alphaOwnership: { opacity: false } }, true)).toBe(true)
  expect(sourceFragmentUsesAlpha(undefined)).toBe(true)
})

test("rocket Particle materials preserve authored wall occlusion without writing translucent depth", () => {
  const material = new THREE.MeshBasicNodeMaterial()
  applyParticleDepthState(material, { depthTest: true, depthWrite: false, depthFunction: 1, blendEnabled: true })
  expect(material.depthTest).toBe(true)
  expect(material.depthWrite).toBe(false)
  expect(material.depthFunc).toBe(THREE.LessEqualDepth)

  const worldDepth = 0.5
  const passesOpaqueWall = (fragmentDepth: number) => !material.depthTest || (
    material.depthFunc === THREE.LessDepth ? fragmentDepth < worldDepth : fragmentDepth <= worldDepth
  )
  expect(passesOpaqueWall(0.49)).toBe(true)
  expect(passesOpaqueWall(0.5)).toBe(true)
  expect(passesOpaqueWall(0.51)).toBe(false)

  applyParticleDepthState(material, { depthTest: false, depthWrite: false, depthFunction: 0, blendEnabled: true })
  expect(material.depthTest).toBe(false)
  expect(material.depthFunc).toBe(THREE.LessDepth)
  expect(() => applyParticleDepthState(material, { depthTest: true, depthWrite: true, depthFunction: 1, blendEnabled: true })).toThrow(/invalid/i)
})

test("world lightmaps bind the canonical UV1 atlas without color or wrap reinterpretation", () => {
  const texture = new THREE.Texture()
  configureWorldLightmap(texture)
  expect(texture.channel).toBe(1)
  expect(texture.flipY).toBe(false)
  expect(texture.colorSpace).toBe(THREE.NoColorSpace)
  expect(texture.minFilter).toBe(THREE.NearestFilter)
  expect(texture.magFilter).toBe(THREE.NearestFilter)
  expect(texture.generateMipmaps).toBe(false)
  expect(texture.wrapS).toBe(THREE.ClampToEdgeWrapping)
  expect(texture.wrapT).toBe(THREE.ClampToEdgeWrapping)

  const hdr = new THREE.Texture()
  configureWorldLightmap(hdr, "hdr")
  expect(hdr.minFilter).toBe(THREE.LinearFilter)
  expect(hdr.magFilter).toBe(THREE.LinearFilter)
})
