import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { particleTextureAlias } from "../src/particle-texture-alias"

const data = [new Uint8Array(64), new Uint8Array(16)]
function texture() {
  const texture = new THREE.CompressedTexture(data.map((data, mip) => ({ data, width: 8 >> mip, height: 8 >> mip })), 8, 8, THREE.RGBA_S3TC_DXT5_Format)
  texture.colorSpace = THREE.SRGBColorSpace; texture.needsUpdate = true
  return texture
}

test("only complete canonical mip-view and normalized upload state can alias an existing owner", () => {
  const owned = texture(), candidate = texture()
  owned.needsUpdate = true // ordinary dirty version after cold backing retirement
  expect(particleTextureAlias(candidate, [owned])).toBe(owned)
  for (const [key, value] of Object.entries({ format: THREE.RGBA_S3TC_DXT1_Format, type: THREE.HalfFloatType, colorSpace: THREE.NoColorSpace,
    channel: 1, internalFormat: "other", wrapS: THREE.RepeatWrapping, wrapT: THREE.RepeatWrapping, minFilter: THREE.NearestFilter,
    magFilter: THREE.NearestFilter, anisotropy: 4, generateMipmaps: true, flipY: true, premultiplyAlpha: true, unpackAlignment: 1,
    matrixAutoUpdate: false, rotation: 1, mapping: THREE.CubeReflectionMapping, onUpdate: () => {} })) {
    const changed: any = texture(); changed[key] = value
    expect(particleTextureAlias(changed, [owned])).toBeUndefined()
  }
  for (const alter of [(t: any) => t.offset.set(1, 0), (t: any) => t.repeat.set(2, 1), (t: any) => t.center.set(1, 0),
    (t: any) => t.matrix.set(2,0,0,0,1,0,0,0,1), (t: any) => t.source.dataReady = false,
    (t: any) => t.image.depth = 2, (t: any) => t.image.width = 4, (t: any) => t.mipmaps.pop(),
    (t: any) => t.mipmaps[1].width = 2, (t: any) => t.mipmaps[1].data = new Uint8Array(data[1]!.buffer),
    (t: any) => t.mipmaps[0].data = data[0]!.slice()]) {
    const changed = texture(); alter(changed); expect(particleTextureAlias(changed, [owned])).toBeUndefined()
  }
  expect(particleTextureAlias(new THREE.DataTexture(data[0], 4, 4), [owned])).toBeUndefined()
})

test("aliases share only the image; material blend, UV nodes and sprite-sheet animation remain independent", () => {
  const a = texture(), b = texture(), image = particleTextureAlias(b, [a])!
  const first = new THREE.MeshBasicNodeMaterial({ map: image, blending: THREE.AdditiveBlending }), second = new THREE.MeshBasicNodeMaterial({ map: image, blending: THREE.NormalBlending })
  const firstUv = new THREE.BufferAttribute(new Float32Array([0, 0, .5, .5]), 2), secondUv = new THREE.BufferAttribute(new Float32Array([.5, .5, 1, 1]), 2)
  firstUv.setXY(0, .25, .125)
  expect(secondUv.getX(0)).toBe(.5); expect(secondUv.getY(0)).toBe(.5)
  expect(first.map).toBe(second.map); expect(first.blending).not.toBe(second.blending)
  first.dispose(); second.dispose(); a.dispose()
})
