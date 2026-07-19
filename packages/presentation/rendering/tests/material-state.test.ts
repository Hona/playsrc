import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { configureWorldLightmap, sourceDepthBias, worldMaterialSide } from "../src/material-state"

test("world materials keep front-face culling unless no-cull is explicit", () => {
  expect(worldMaterialSide(0)).toBe(THREE.FrontSide)
  expect(worldMaterialSide(8)).toBe(THREE.DoubleSide)
})

test("categorical decal bias maps to the fixed WebGPU adapter", () => {
  expect(sourceDepthBias("none")).toEqual({ enabled: false, slopeScale: 0, units: 0 })
  expect(sourceDepthBias("decal")).toEqual({ enabled: true, slopeScale: -0.5, units: -262_144 })
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
