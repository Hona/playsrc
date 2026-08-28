import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import Textures from "three/src/renderers/common/Textures.js"
import { SourceParticleDepth } from "../src/particle-depth"

test("particle compile and live framebuffer textures have distinct actual mutable owners", () => {
  const records: { kind: string; owner: string; bytes: number }[] = []
  const live = new Map<THREE.Texture, number>()
  const backend = {
    copyFramebufferToTexture() {},
    createTexture(texture: THREE.Texture, options: any) {
      expect(options.levels).toBe(1)
      const bytes = options.width * options.height * 4
      live.set(texture, bytes); records.push({ kind: "create", owner: texture.name, bytes })
    },
    destroyTexture(texture: THREE.Texture) {
      records.push({ kind: "destroy", owner: texture.name, bytes: live.get(texture)! }); live.delete(texture)
    },
  }
  const renderer = { getRenderTarget: () => null, getCanvasTarget: () => target, info: { calls: 0 },
    getDrawingBufferSize: (size: THREE.Vector2) => size.set(1280, 720),
    copyFramebufferToTexture: (texture: THREE.Texture) => textures.updateTexture(texture) }
  const target = {}, textures = new Textures(renderer, backend, { createTexture() {}, destroyTexture() {} })
  const camera = new THREE.PerspectiveCamera()
  for (let generation = 0; generation < 3; generation++) {
    const depth = new SourceParticleDepth(backend), sample = depth.sample()
    const compile = sample.value
    textures.updateTexture(compile)
    depth.capture(renderer as any, camera)
    expect(sample.value).not.toBe(compile)
    expect(sample.value.name).toBe("particle-depth:framebuffer")
    expect(live.size).toBe(2)
    expect([...live.values()]).toEqual([4, 3686400])
    renderer.info.calls++
    depth.capture(renderer as any, camera)
    expect(live.size).toBe(2)
    depth.dispose(); expect(live.size).toBe(0)
  }
  expect(records.filter(record => record.kind === "create")).toHaveLength(6)
  console.log(JSON.stringify({ particleDepthOwnerLoop: records, uploadInputBytes: 0 }))
})
