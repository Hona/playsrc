import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { evaluateSourceCloakPixel, sourceCloakPassState, sourceCloakTransparentSort, SourceCloakFramebuffer, SourceModelCloak } from "../src/source-cloak"

test("cloak uses nine authored samples, Fresnel transition and linear tint rather than actor alpha", () => {
  const coordinates: (readonly [number, number])[] = []
  const input = { factor: 0.5, refractAmount: 0.1, tint: [1, 0.5, 0.4] as const, normalDotView: 1,
    coordinate: [0.5, 0.5] as const, projectedNormal: [1, 0] as const,
    sample: (uv: readonly [number, number]) => { coordinates.push(uv); return [1, 1, 1] as const } }
  const pixel = evaluateSourceCloakPixel(input)
  expect(coordinates).toHaveLength(9)
  expect(coordinates[0]).toEqual([0.55, 0.5])
  expect(pixel[3]).toBe(1)
  expect(pixel[0]).toBeCloseTo(0.85)
  expect(pixel[1]).toBeCloseTo(0.425)
  expect(pixel[2]).toBeCloseTo(0.34)
  expect(sourceCloakPassState(0)).toEqual({ standard: true, cloak: false })
  expect(sourceCloakPassState(0.22)).toEqual({ standard: true, cloak: true })
  expect(sourceCloakPassState(0.5)).toEqual({ standard: false, cloak: true })
  expect(sourceCloakPassState(1)).toEqual({ standard: false, cloak: false })
  expect(sourceCloakPassState(2)).toEqual({ standard: false, cloak: false })
  expect(sourceCloakPassState(-1)).toEqual({ standard: true, cloak: false })
})

test("two model instances have independent uniforms and never mutate the template", () => {
  const framebuffer = new SourceCloakFramebuffer(), template = new THREE.MeshBasicNodeMaterial()
  const state = { enabled: true, factor: 0, colorTint: [1, 1, 1] as const, refractAmount: 0.1 }
  const make = () => {
    const root = new THREE.Group(), base = new THREE.Mesh(new THREE.BufferGeometry(), template.clone())
    base.userData.materialIdentity = "spy"
    root.add(base)
    const cloak = new SourceModelCloak([base], framebuffer, () => ({ cloakProxy: 5, state: { cloak: state } }), () => undefined)
    return { root, base, cloak }
  }
  const a = make(), b = make()
  const camera = new THREE.PerspectiveCamera()
  const binding = { localFactor: 0.5, worldFactor: 0.95, rawFactor: 1, playerTint: [1, 0.5, 0.4] as const, local: true, player: false }
  a.cloak.update(binding, false, a.root, camera, 1)
  b.cloak.update(null, false, b.root, camera, 2)
  expect(a.base.material.visible).toBe(false)
  expect(b.base.material.visible).toBe(true)
  expect(template.visible).toBe(true)
  expect(a.root.children[1]!.visible).toBe(true)
  expect(b.root.children[1]!.visible).toBe(false)
  b.cloak.update(binding, false, b.root, camera, 2)
  a.root.position.z = -10; b.root.position.z = -20
  a.root.updateMatrixWorld(true); b.root.updateMatrixWorld(true); camera.updateMatrixWorld(true)
  const item = (object: THREE.Object3D) => ({ object, id: object.id, z: 0, groupOrder: 0, renderOrder: 0 })
  expect(sourceCloakTransparentSort(item(a.root.children[1]!), item(b.root.children[1]!), 1)).toBeGreaterThan(0)
  camera.position.z = -30; camera.rotation.y = Math.PI; camera.updateMatrixWorld(true)
  expect(sourceCloakTransparentSort(item(a.root.children[1]!), item(b.root.children[1]!), 2)).toBeLessThan(0)
  a.cloak.update(binding, true, a.root, camera, 1)
  b.cloak.update(binding, true, b.root, camera, 2)
  expect(sourceCloakTransparentSort(item(a.root.children[1]!), item(b.root.children[1]!), 3)).toBeLessThan(0)
  b.base.visible = false
  b.cloak.update(binding, true, b.root, camera, 2)
  expect(b.root.children[1]!.visible).toBe(false)
  a.cloak.update(null, true, a.root, camera, 1)
  expect(a.base.material.visible).toBe(true)
  expect(framebuffer.samplerCount).toBe(2)
  ;(a.root.children[1] as THREE.Mesh).material.dispose()
  ;(b.root.children[1] as THREE.Mesh).material.dispose()
  expect(framebuffer.samplerCount).toBe(0)
  framebuffer.dispose()
})

test("framebuffer copy is current per actor and render pass, resizes, and is reused across primitives", () => {
  const framebuffer = new SourceCloakFramebuffer(), owners = [{}, {}], copies: number[] = []
  const renderer = { info: { calls: 1 }, getRenderTarget: () => ({ width: 320, height: 200, texture: { type: THREE.UnsignedByteType } }), copyFramebufferToTexture: (texture: THREE.Texture) => copies.push(texture.image.width) } as any
  framebuffer.capture(renderer, owners[0]!)
  framebuffer.capture(renderer, owners[0]!)
  framebuffer.capture(renderer, owners[1]!)
  renderer.info.calls++
  renderer.getRenderTarget = () => ({ width: 640, height: 400, texture: { type: THREE.UnsignedByteType } })
  framebuffer.capture(renderer, owners[1]!)
  expect(copies).toEqual([320, 320, 640])
  const ldr = framebuffer.texture, sampler = framebuffer.sample()
  renderer.info.calls++
  renderer.getRenderTarget = () => ({ width: 640, height: 400, texture: { type: THREE.HalfFloatType } })
  framebuffer.capture(renderer, owners[0]!)
  expect(framebuffer.texture).not.toBe(ldr)
  expect(sampler.value).toBe(framebuffer.texture)
  renderer.info.calls++
  renderer.getRenderTarget = () => ({ width: 640, height: 400, texture: { type: THREE.UnsignedByteType } })
  framebuffer.capture(renderer, owners[0]!)
  expect(framebuffer.texture).toBe(ldr)
  framebuffer.dispose()
})
