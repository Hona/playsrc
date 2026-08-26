import {
  SOURCE_PC_INTEGER_HDR,
  createRenderer,
  sourceHorizontal4By3FovToVertical,
  type Camera,
  type VisibilityFrame,
  type WaterFramePass,
} from "../src/index"
import {
  chunksForRole,
  encodeResourceBatch,
  parseResourceGraphBytes,
  parseResourceSet,
} from "../../../asset-store/src/graph"
import { parsePresentationArtifacts } from "../../../../games/tf2/browser/src/artifacts"

type Exports = Readonly<{
  memory: WebAssembly.Memory
  playsrc_alloc(length: number): number
  playsrc_free(pointer: number, length: number): void
  playsrc_resource_decode(pointer: number, length: number): number
  playsrc_resource_length(): number
  playsrc_resource_take(): number
  playsrc_resource_release(pointer: number, length: number): number
  playsrc_compile_map(bsp: number, length: number, profile: number, sections: number, sectionCount: number, configurationSha256: number): number
  playsrc_result_error(handle: number): number
  playsrc_result_length(handle: number): number
  playsrc_result_copy(handle: number, pointer: number, capacity: number): number
  playsrc_result_hash(handle: number, pointer: number): number
  playsrc_presentation_length(handle: number): number
  playsrc_presentation_copy(handle: number, pointer: number, capacity: number): number
  playsrc_visibility_query(handle: number, pointer: number): number
  playsrc_visibility_output_length(handle: number): number
  playsrc_visibility_output_copy(handle: number, pointer: number, capacity: number): number
  playsrc_dispose(handle: number): number
}>

const ORIGIN = (window as any).__sourceWaterEvidenceOrigin as string

function require(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message)
}

function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (value) => value.toString(16).padStart(2, "0")).join("")
}

async function bytes(path: string): Promise<Uint8Array> {
  const response = await fetch(`${ORIGIN}/${path}`)
  require(response.ok, `${path} returned ${response.status}`)
  return new Uint8Array(await response.arrayBuffer())
}

function copied(exports: Exports, bytes: Uint8Array): number {
  const pointer = exports.playsrc_alloc(bytes.byteLength) >>> 0
  require(pointer > 0, "WASM allocation failed")
  new Uint8Array(exports.memory.buffer, pointer, bytes.byteLength).set(bytes)
  return pointer
}

console.info("loading exact configured Water graph")
const graph = parseResourceGraphBytes(await bytes("graph"))
require(graph.target === "jump_beef" && graph.contentBuild === "24245096", "Water evidence target identity differs")
const [wasm, bsp, chunks] = await Promise.all([
  bytes("wasm"),
  bytes("bsp"),
  Promise.all(chunksForRole(graph, "gameplay").map(async (descriptor) => Object.freeze({
    descriptor,
    bytes: await bytes(`chunks/${descriptor.encodedSha256}`),
  }))),
])
console.info(`decoded ${chunks.length} exact gameplay chunk responses; instantiating Rust WASM`)
const loaded = await WebAssembly.instantiate(wasm, {
  playsrc_metrics: { monotonic_milliseconds: () => performance.now() },
})
const exports = loaded.instance.exports as unknown as Exports
const batch = encodeResourceBatch(chunks)
const batchPointer = copied(exports, batch)
require(exports.playsrc_resource_decode(batchPointer, batch.byteLength) === 1, "Rust resource graph decoding failed")
exports.playsrc_free(batchPointer, batch.byteLength)
const resourceLength = exports.playsrc_resource_length()
const resourcePointer = exports.playsrc_resource_take() >>> 0
require(resourcePointer !== 0, "Rust resource set ownership transfer failed")
const resources = new Uint8Array(exports.memory.buffer, resourcePointer, resourceLength).slice()
require(exports.playsrc_resource_release(resourcePointer, resourceLength) === 1, "Rust resource source release failed")

console.info(`decoded ${resources.byteLength} resource bytes; compiling exact HDR map`)
const bspPointer = copied(exports, bsp)
const configPointer = copied(exports, resources)
const configSections = exports.playsrc_alloc(8)
new DataView(exports.memory.buffer, configSections, 8).setUint32(0, configPointer, true)
new DataView(exports.memory.buffer, configSections, 8).setUint32(4, resources.byteLength, true)
const configHash = exports.playsrc_alloc(32)
new Uint8Array(exports.memory.buffer, configHash, 32).set(new Uint8Array(await crypto.subtle.digest("SHA-256", resources)))
const handle = exports.playsrc_compile_map(bspPointer, bsp.byteLength, 1, configSections, 1, configHash)
exports.playsrc_free(configSections, 8)
exports.playsrc_free(configHash, 32)
exports.playsrc_free(bspPointer, bsp.byteLength)
exports.playsrc_free(configPointer, resources.byteLength)
require(exports.playsrc_result_error(handle) === 0, `Rust HDR Water map compilation failed: ${exports.playsrc_result_error(handle)}`)
const payloadLength = exports.playsrc_result_length(handle)
const payloadPointer = exports.playsrc_alloc(payloadLength) >>> 0
require(exports.playsrc_result_copy(handle, payloadPointer, payloadLength) === payloadLength, "Rust HDR payload copy failed")
const payload = new Uint8Array(exports.memory.buffer, payloadPointer, payloadLength).slice()
exports.playsrc_free(payloadPointer, payloadLength)
const hashPointer = exports.playsrc_alloc(32) >>> 0
require(exports.playsrc_result_hash(handle, hashPointer) === 1, "Rust HDR payload hash is unavailable")
const payloadSha256 = hex(new Uint8Array(exports.memory.buffer, hashPointer, 32))
exports.playsrc_free(hashPointer, 32)
require(hex(new Uint8Array(await crypto.subtle.digest("SHA-256", payload))) === payloadSha256, "Rust HDR payload identity differs")

const presentationLength = exports.playsrc_presentation_length(handle)
const presentationPointer = exports.playsrc_alloc(presentationLength) >>> 0
require(exports.playsrc_presentation_copy(handle, presentationPointer, presentationLength) === presentationLength, "Rust presentation copy failed")
const presentation = new Uint8Array(exports.memory.buffer, presentationPointer, presentationLength).slice()
exports.playsrc_free(presentationPointer, presentationLength)
console.info(`compiled ${payloadLength} map bytes and ${presentationLength} presentation bytes; decoding presentation`)
const artifacts = await parsePresentationArtifacts(presentation, parseResourceSet(resources))
require(artifacts.environment.waterVolumeFacts.length === 1, "configured Water volume count differs")
require(artifacts.environment.waterMaterials.size === 2, "configured Water material count differs")

const canvas = document.querySelector("canvas")!
console.info("creating the shipped visible WebGPU renderer")
const renderer = await createRenderer({ canvas, configuration: SOURCE_PC_INTEGER_HDR, powerPreference: "high-performance" })
renderer.resize(960, 540, 1)
const scene = await renderer.loadMap({
  payload,
  payloadSha256,
  directionalTextures: artifacts.directionalTextures,
  environment: artifacts.environment,
  materialStates: artifacts.materialStates,
  particleTextures: artifacts.particleTextures,
  modelOccurrences: artifacts.modelOccurrences,
  modelFacing: new Map([...artifacts.models].map(([identity, artifact]) => [identity.toLowerCase(), Object.freeze({
    frontFace: artifact.descriptor.frontFace,
    cullFace: artifact.descriptor.cullFace,
  })])),
  modelMaterials: artifacts.modelMaterials,
  authoredTextures: artifacts.authoredTextures,
  brushModels: artifacts.brushModels,
  staticProps: artifacts.staticProps,
  diagnostic: true,
})

function visibility(camera: Camera, time: number): VisibilityFrame {
  const values = [
    ...camera.position,
    ...camera.position,
    camera.yawDegrees,
    camera.pitchDegrees,
    camera.verticalFovDegrees,
    960 / 540,
    camera.near,
    camera.far,
    time,
    -1,
  ]
  const pointer = exports.playsrc_alloc(56) >>> 0
  new Float32Array(exports.memory.buffer, pointer, 14).set(values)
  require(exports.playsrc_visibility_query(handle, pointer) === 1, "Rust Water visibility query failed")
  exports.playsrc_free(pointer, 56)
  const length = exports.playsrc_visibility_output_length(handle)
  const outputPointer = exports.playsrc_alloc(length) >>> 0
  require(exports.playsrc_visibility_output_copy(handle, outputPointer, length) === length, "Rust Water visibility copy failed")
  const output = new Uint8Array(exports.memory.buffer, outputPointer, length).slice()
  exports.playsrc_free(outputPointer, length)
  const view = new DataView(output.buffer)
  const decoder = new TextDecoder("utf-8", { fatal: true })
  require(decoder.decode(output.subarray(0, 4)) === "PVIS" && view.getUint32(4, true) === 6, "Rust Water visibility identity differs")
  let offset = 76
  const take = (count: number): number => {
    require(offset + count <= output.byteLength, "Rust Water visibility is truncated")
    const start = offset
    offset += count
    return start
  }
  const u8 = () => output[take(1)]!
  const u32 = () => view.getUint32(take(4), true)
  const i32 = () => view.getInt32(take(4), true)
  const f32 = () => view.getFloat32(take(4), true)
  const vector = (): readonly [number, number, number] => Object.freeze([f32(), f32(), f32()])
  const indices = (): Uint32Array => {
    const count = u32()
    const values = new Uint32Array(count)
    for (let index = 0; index < count; index += 1) values[index] = u32()
    return values
  }
  const surfaces = indices()
  const drawSurfaces = indices()
  const leaf = u32()
  const leaves = Object.freeze(Array.from({ length: u32() }, u32))
  const areas = Object.freeze(Array.from({ length: u32() }, u32))
  const present = u8(), cheap = u8(), reflect = u8(), refract = u8()
  const reflectEntities = u8(), drawSurface = u8(), opaque = u8(), nearPlaneIntersects = u8()
  let current: VisibilityFrame["water"]["visibleWater"] = null
  if (present) {
    const volume = u32(), visibleLeaf = u32(), eyeLeaf = u32(), eyeInVolume = u8(), translucent = u8(), hasOverlay = u8()
    require(hasOverlay <= 1 && u8() === 0, "Rust Water visibility padding differs")
    const surfaceZ = f32(), distance = u32()
    const size = u32()
    const material = decoder.decode(output.subarray(take(size), offset))
    const normalFrame = i32()
    const normalTransform = new Float32Array(16)
    for (let index = 0; index < 16; index += 1) normalTransform[index] = f32()
    const evaluated = Object.freeze({ normalFrame, normalTransform, cheapStart: f32(), cheapEnd: f32() })
    let overlay: NonNullable<VisibilityFrame["water"]["visibleWater"]>["overlay"] = null
    if (hasOverlay) {
      const identityLength = u32()
      const identity = decoder.decode(output.subarray(take(identityLength), offset))
      const overlayFrame = i32(), overlayTransform = new Float32Array(16)
      for (let index = 0; index < overlayTransform.length; index += 1) overlayTransform[index] = f32()
      overlay = Object.freeze({ identity, normalFrame: overlayFrame, normalTransform: overlayTransform })
    }
    current = Object.freeze({
      volume,
      visibleLeaf,
      eyeLeaf,
      eyeInVolume: eyeInVolume === 1,
      surfaceZ,
      distanceToWater: distance === 0xffff ? null : distance,
      material,
      translucent: translucent === 1,
      evaluated,
      overlay,
    })
  }
  const passes: WaterFramePass[] = []
  for (let count = u32(); count > 0; count -= 1) {
    const kind = u8(), above = u8(), below = u8(), water = u8(), entities = u8(), sky = u8(), hasClip = u8(), keep = u8()
    const origin = vector(), angles = vector(), clipHeight = f32(), forced = u32(), fog = u8(), height = u8()
    require(u8() === 0 && u8() === 0, "Rust Water view padding differs")
    const fogVolume = u32()
    passes.push(Object.freeze({
      kind: (["reflection", "refraction", "main", "intersection"] as const)[kind]!,
      origin,
      angles,
      renderAboveWater: above === 1,
      renderUnderWater: below === 1,
      renderWaterSurface: water === 1,
      drawEntities: entities === 1,
      drawSky2d: sky === 1,
      clip: hasClip ? Object.freeze({ height: clipHeight, keep: keep === 1 ? "above" as const : "below" as const }) : null,
      forcedVisibilityLeaf: forced === 0xffff_ffff ? null : forced,
      fog: fog ? Object.freeze({ kind: "water" as const, volume: fogVolume, heightFog: height === 1 }) : Object.freeze({ kind: "world" as const }),
      surfaces: indices(),
    }))
  }
  const worldMaterials: VisibilityFrame["worldMaterials"][number][] = []
  for (let count = u32(); count > 0; count--) {
    const identityLength = u32()
    const identity = decoder.decode(output.subarray(take(identityLength), offset))
    const mapMaterial = u32()
    const textures: VisibilityFrame["worldMaterials"][number]["textures"][number][] = []
    for (let textureCount = u32(); textureCount > 0; textureCount--) {
      const role = u8(), hasFrame = u8(), hasTransform = u8()
      require(u8() === 0, "Rust world material texture padding differs")
      const frame = i32(), matrix = new Float32Array(16)
      for (let index = 0; index < matrix.length; index++) matrix[index] = f32()
      textures.push(Object.freeze({ role, frame: hasFrame ? frame : null, transform: hasTransform ? matrix : null }))
    }
    worldMaterials.push(Object.freeze({ identity, mapMaterial, textures: Object.freeze(textures) }))
  }
  require(offset === output.byteLength, "Rust Water visibility contains trailing bytes")
  return Object.freeze({
    cacheIdentity: hex(output.subarray(8, 40)),
    worldIdentity: hex(output.subarray(40, 72)),
    outsideWorld: output[72] === 1,
    sky: output[73] as 0 | 1 | 2,
    eyeLeaf: leaf === 0xffff_ffff ? null : leaf,
    leaves,
    areas,
    surfaces,
    drawSurfaces,
    water: Object.freeze({
      visibleWater: current,
      render: Object.freeze({
        cheap: cheap === 1,
        reflect: reflect === 1,
        refract: refract === 1,
        reflectEntities: reflectEntities === 1,
        drawSurface: drawSurface === 1,
        opaque: opaque === 1,
      }),
      nearPlaneIntersects: nearPlaneIntersects === 1,
      passes: Object.freeze(passes),
    }),
    worldMaterials: Object.freeze(worldMaterials),
  })
}

async function renderSpawn() {
  const camera: Camera = Object.freeze({
    position: Object.freeze([5328, 3376, -3068]) as readonly [number, number, number],
    yawDegrees: 180,
    pitchDegrees: 0,
    verticalFovDegrees: sourceHorizontal4By3FovToVertical(75),
    near: 7,
    far: 32768,
  })
  const selected = visibility(camera, 0)
  require(selected.water.visibleWater === null, "Configured spawn unexpectedly selected Water")
  const started = performance.now()
  const output = await renderer.render({
    camera,
    effects: [],
    particles: [],
    models: [],
    visibility: selected,
    collisionWorldIdentity: artifacts.environment.collisionWorldIdentity,
    deltaSeconds: 0.015,
  })
  return Object.freeze({
    milliseconds: performance.now() - started,
    passes: Object.freeze([...output.waterPasses]),
    drawableSurfaces: scene.drawableSurfaces,
  })
}

async function renderScenario(name: "above-frame-0" | "above-frame-30" | "below" | "crossing") {
  const below = name === "below"
  const crossing = name === "crossing"
  const position = [-4800, 3000, below ? -2300 : crossing ? -2160 : -2100] as const
  const time = name === "above-frame-0" ? 0 : 1
  const camera = Object.freeze({
    position,
    yawDegrees: 0,
    pitchDegrees: below ? -20 : crossing ? 0 : 20,
    verticalFovDegrees: sourceHorizontal4By3FovToVertical(75),
    near: 7,
    far: 32768,
  })
  const selected = visibility(camera, time)
  require(selected.water.visibleWater !== null, `${name} did not select configured Water`)
  const started = performance.now()
  const output = await renderer.render({
    camera,
    effects: [],
    particles: [],
    models: [],
    visibility: selected,
    collisionWorldIdentity: artifacts.environment.collisionWorldIdentity,
    deltaSeconds: 0.015,
  })
  const elapsedMilliseconds = performance.now() - started
  const targetSamples = await Promise.all([
    renderer.captureWaterTargetEvidence(480, 135),
    renderer.captureWaterTargetEvidence(480, 270),
    renderer.captureWaterTargetEvidence(480, 405),
  ])
  const targets = targetSamples[1]!
  return Object.freeze({
    name,
    camera,
    normalFrame: selected.water.visibleWater.evaluated.normalFrame,
    material: selected.water.visibleWater.material,
    eyeInVolume: selected.water.visibleWater.eyeInVolume,
    visibilitySky: selected.sky,
    plannedPasses: selected.water.passes.map((pass) => pass.kind),
    renderedPasses: output.waterPasses,
    passTimings: output.waterPassTimings,
    stateRestored: output.waterStateRestored,
    nearPlaneIntersects: selected.water.nearPlaneIntersects,
    targets,
    targetSamples: Object.freeze(targetSamples),
    elapsedMilliseconds,
    worldMilliseconds: output.timings.worldMilliseconds,
    drawableSurfaces: scene.drawableSurfaces,
  })
}

async function benchmarkWater() {
  const conditions = [
    { name: "above-surface", heights: [-2100], pitches: [20] },
    { name: "underwater", heights: [-2300], pitches: [-20] },
    { name: "enter-exit", heights: [-2150, -2170], pitches: [20, -20] },
    { name: "near-plane", heights: [-2160], pitches: [0] },
  ] as const
  const distributions = []
  for (const condition of conditions) {
    const samples = []
    for (let index = 0; index < 40; index += 1) {
      const choice = index % condition.heights.length
      const camera: Camera = Object.freeze({
        position: Object.freeze([-4800, 3000, condition.heights[choice]!]) as readonly [number, number, number],
        yawDegrees: 0,
        pitchDegrees: condition.pitches[choice]!,
        verticalFovDegrees: sourceHorizontal4By3FovToVertical(75),
        near: 7,
        far: 32768,
      })
      const started = performance.now()
      const selected = visibility(camera, 2 + index * 0.015)
      const visibilityMilliseconds = performance.now() - started
      require(selected.water.visibleWater !== null, `${condition.name} frame ${index} lost Water visibility`)
      const output = await renderer.render({
        camera,
        effects: [],
        particles: [],
        models: [],
        visibility: selected,
        collisionWorldIdentity: artifacts.environment.collisionWorldIdentity,
        deltaSeconds: 0.015,
      })
      const elapsedMilliseconds = performance.now() - started
      require(output.waterStateRestored, `${condition.name} frame ${index} did not restore view state`)
      samples.push(Object.freeze({
        ordinal: index,
        milliseconds: elapsedMilliseconds,
        visibilityMilliseconds,
        renderMilliseconds: output.timings.totalMilliseconds,
        worldMilliseconds: output.timings.worldMilliseconds,
        normalFrame: selected.water.visibleWater.evaluated.normalFrame,
        eyeInVolume: selected.water.visibleWater.eyeInVolume,
        passes: Object.freeze([...output.waterPasses]),
        passTimings: output.waterPassTimings,
      }))
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
    }
    const ordered = samples.map((sample) => sample.milliseconds).toSorted((left, right) => left - right)
    const percentile = (value: number) => ordered[Math.min(ordered.length - 1, Math.floor(ordered.length * value))]!
    distributions.push(Object.freeze({
      name: condition.name,
      frames: samples.length,
      p50Milliseconds: percentile(0.5),
      p95Milliseconds: percentile(0.95),
      p99Milliseconds: percentile(0.99),
      maximumMilliseconds: ordered.at(-1)!,
      samples: Object.freeze(samples),
    }))
  }
  return Object.freeze(distributions)
}

async function renderOverhead(position: readonly [number, number, number], pitchDegrees: number, suppressWater = false) {
  const camera: Camera = Object.freeze({ position, yawDegrees: 0, pitchDegrees, verticalFovDegrees: sourceHorizontal4By3FovToVertical(75), near: 7, far: 32768 })
  let selected = visibility(camera, 0)
  require(selected.water.visibleWater !== null, `overhead ${position.join(",")} did not select configured Water`)
  const water = selected.water.visibleWater
  if (suppressWater) {
    const main = selected.water.passes.find((pass) => pass.kind === "main")!
    selected = Object.freeze({
      ...selected,
      water: Object.freeze({
        visibleWater: null,
        render: Object.freeze({ cheap: true, reflect: false, refract: false, reflectEntities: false, drawSurface: false, opaque: true }),
        nearPlaneIntersects: false,
        passes: Object.freeze([Object.freeze({ ...main, renderWaterSurface: false, renderUnderWater: true, clip: null, surfaces: selected.surfaces })]),
      }),
    })
  }
  const output = await renderer.render({ camera, effects: [], particles: [], models: [], visibility: selected, collisionWorldIdentity: artifacts.environment.collisionWorldIdentity, deltaSeconds: 0.015 })
  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
  return Object.freeze({ position, pitchDegrees, suppressWater, normalFrame: water.evaluated.normalFrame, material: water.material, eyeLeaf: water.eyeLeaf, visibleLeaf: water.visibleLeaf, passes: output.waterPasses, stateRestored: output.waterStateRestored, drawableSurfaces: scene.drawableSurfaces })
}

console.info("shipped Water renderer is ready for exact above/below captures")
Object.assign(window, {
  __sourceWaterMapReady: true,
  __sourceWaterMapWarmSpawn: renderSpawn,
  __sourceWaterMapScenario: renderScenario,
  __sourceWaterMapOverhead: renderOverhead,
  __sourceWaterMapBenchmark: benchmarkWater,
  __sourceWaterMapDispose: async () => {
    await renderer.dispose()
    require(exports.playsrc_dispose(handle) === 1, "Rust Water map handle disposal failed")
  },
})
