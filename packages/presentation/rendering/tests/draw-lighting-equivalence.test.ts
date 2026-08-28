import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import NodeFrame from "three/src/nodes/core/NodeFrame.js"
import { ModelLightingGraphs, bindModelLighting } from "../src/model-lighting-graphs"
import { createSourceModelLightingUniforms, sourceModelSurfaceNode, type SourceModelLightingUniforms, type SourceModelSurface } from "../src/source-model-lighting"

// The previous per-member reference path is an independent value/packed-buffer
// oracle, not a production fallback. Shader code must also be byte-identical.
function references(): SourceModelLightingUniforms {
  const ref = (path: string, type: string) => TSL.reference(`userData.sourceLighting.${path}.value`, type) as ReturnType<typeof TSL.uniform>
  return {
    ambientEnabled: ref("ambientEnabled", "float"), cameraPosition: ref("cameraPosition", "vec3"),
    ambient: Array.from({ length: 6 }, (_, i) => ref(`ambient.${i}`, "vec3")) as any,
    local: Array.from({ length: 4 }, (_, i) => Object.fromEntries(
      ["enabled", "kind", "color", "position", "direction", "attenuation", "falloff", "theta", "phi"].map(name =>
        [name, ref(`local.${i}.${name}`, ["color", "position", "direction", "attenuation"].includes(name) ? "vec3" : "float")]),
    )) as any,
  }
}

const members = (lighting: SourceModelLightingUniforms) => [lighting.ambientEnabled, lighting.cameraPosition, ...lighting.ambient,
  ...lighting.local.flatMap(light => Object.values(light))]

function build(lighting: SourceModelLightingUniforms, partial = false, surface: SourceModelSurface = { halfLambert: true }) {
  const renderer = new THREE.WebGPURenderer({ canvas: { width: 1, height: 1, style: {}, addEventListener() {} } as any })
  renderer.hasFeature = () => false
  const material = new THREE.MeshBasicNodeMaterial({ side: THREE.BackSide })
  material.colorNode = partial ? TSL.vec4(lighting.local[3].position, lighting.cameraPosition.x)
    : sourceModelSurfaceNode(TSL.vec4(1), lighting, surface, TSL.float(1)).color
  const object = new THREE.Mesh(new THREE.BoxGeometry(), material)
  bindModelLighting(object, createSourceModelLightingUniforms())
  const builder = new THREE.WGSLNodeBuilder(object, renderer)
  builder.scene = new THREE.Scene(); builder.camera = new THREE.PerspectiveCamera()
  builder.build()
  return builder
}

test("one draw dependency replaces member dispatch without changing full or partial shaders/buffers", () => {
  for (const partial of [false, true]) {
    const old = references(), graphs = new ModelLightingGraphs()
    const baseline = build(old, partial), candidate = build(graphs.lighting, partial)
    expect(candidate.vertexShader).toBe(baseline.vertexShader)
    expect(candidate.fragmentShader).toBe(baseline.fragmentShader)
    const events = candidate.updateNodes.filter((node: any) => node instanceof THREE.EventNode)
    expect(events).toHaveLength(1)
    const lightingReferences = (builder: any) => builder.updateNodes.filter((node: any) => node.property?.startsWith("userData.sourceLighting."))
    expect(lightingReferences(candidate)).toHaveLength(0)
    expect(lightingReferences(baseline)).toHaveLength(partial ? 2 : 43)
    const actors = [new THREE.Mesh(), new THREE.Mesh()]
    const sources = actors.map((actor, i) => {
      const lighting = createSourceModelLightingUniforms()
      members(lighting).forEach((node, j) => {
        if (typeof node.value === "number") node.value = i * 100 + j / 13
        else node.value.set(i + j / 7, -j, j * 1.25)
      })
      bindModelLighting(actor, lighting)
      return lighting
    })
    const frame = new NodeFrame(); frame.frameId = 1; frame.renderId = 1
    frame.camera = baseline.camera; frame.renderer = baseline.renderer
    frame.material = baseline.material; frame.scene = baseline.scene
    const groups = (builder: any) => builder.getBindings().flatMap((group: any) => group.bindings).filter((binding: any) => binding.isUniformsGroup)
    for (let draw = 0; draw < 32; draw++) {
      const index = [0, 1, 1, 0][draw % 4]!
      // Same object, same pass, in-place values, scalar and vector replacement,
      // and a complete binding replacement all remain observable.
      const source = sources[index]!
      source.cameraPosition.value.x += .125
      source.ambientEnabled.value = draw % 2
      source.local[3].position.value = new THREE.Vector3(draw, -draw, draw / 3)
      if (draw === 16) {
        sources[index] = createSourceModelLightingUniforms()
        bindModelLighting(actors[index]!, sources[index]!)
      }
      frame.object = actors[index]!
      for (const node of baseline.updateNodes) frame.updateNode(node)
      for (const node of candidate.updateNodes) frame.updateNode(node)
      const a = groups(baseline), b = groups(candidate)
      expect(b.length).toBeGreaterThan(0)
      expect(b.length).toBe(a.length)
      for (let i = 0; i < a.length; i++) {
        expect(b[i].update()).toBe(a[i].update())
        expect(new Uint8Array(b[i].buffer.buffer, b[i].buffer.byteOffset, b[i].buffer.byteLength))
          .toEqual(new Uint8Array(a[i].buffer.buffer, a[i].buffer.byteOffset, a[i].buffer.byteLength))
      }
      if (draw % 4 === 0) frame.renderId++
      if (draw % 8 === 0) frame.frameId++
    }
    graphs.releaseDrawReferences()
    expect(members(graphs.lighting).every(node => node.value === null)).toBe(true)
    frame.object = actors[0]!
    frame.updateNode(events[0]!)
    members(graphs.lighting).forEach((node, i) => expect(node.value).toBe(members(sources[0]!)[i]!.value))
  }
})

test("Phong/rim, warped diffuse, eye occlusion and cubemap shader/texture contracts are unchanged", () => {
  const texture = new THREE.Texture(), environment = new THREE.CubeTexture()
  const surfaces: SourceModelSurface[] = [
    { halfLambert: false, diffuseWarp: texture },
    { halfLambert: true, exponentTexture: texture, phong: {
      maskSource: 0, invertMask: false, albedoTint: true, exponent: 4, exponentFactor: 8,
      tint: [1, .75, .5], boost: 2, packedFresnel: [0, .5, 1],
      rim: { exponent: 4, boost: 1, exponentTextureAlphaMask: true },
    } },
    { halfLambert: true, eye: { ambientOcclusion: texture, ambientOcclusionColor: [.2, .3, .4], glossiness: .5 },
      environment: { texture: environment, tint: [.5, .75, 1], scale: 1 } },
  ]
  for (const surface of surfaces) {
    const baseline = build(references(), false, surface), candidate = build(new ModelLightingGraphs().lighting, false, surface)
    expect(candidate.vertexShader).toBe(baseline.vertexShader)
    expect(candidate.fragmentShader).toBe(baseline.fragmentShader)
    const textures = (builder: any) => builder.getBindings().flatMap((group: any) => group.bindings)
      .filter((binding: any) => binding.isSampledTexture).map((binding: any) => binding.texture)
    expect(textures(candidate)).toEqual(textures(baseline))
  }
})

test("same-draw mutations preserve all values including signed zero and non-finite comparisons", () => {
  const graphs = new ModelLightingGraphs(), source = createSourceModelLightingUniforms(), actor = new THREE.Mesh()
  bindModelLighting(actor, source)
  const frame = new NodeFrame(); frame.object = actor
  const event = (graphs.lighting.ambientEnabled as any)._beforeNodes[0]
  for (const value of [-0, NaN, Infinity, -Infinity, 1 / 3]) {
    source.ambientEnabled.value = value
    source.local[0].color.value.set(value, -0, 1 / 3)
    frame.updateNode(event)
    expect(Object.is(graphs.lighting.ambientEnabled.value, value)).toBe(true)
    expect(graphs.lighting.local[0].color.value).toBe(source.local[0].color.value)
  }
})
