import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { configureWorldLightmap, worldMaterialSide } from "../src/material-state"

test("world materials keep front-face culling unless no-cull is explicit", () => {
  expect(worldMaterialSide(0)).toBe(THREE.FrontSide)
  expect(worldMaterialSide(8)).toBe(THREE.DoubleSide)
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
})
