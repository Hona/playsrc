import path from "node:path"
import { mkdir } from "node:fs/promises"
import { loadLocalConfig } from "../../../../tools/playsrc/src/config"
import { instrumentParticleAliasSource } from "../../../../tools/playsrc/profile/particle-alias-route"

/** No browser or GPU. Expose the unchanged production owner solely in an
 * offline test bundle; replace only its native compilation/device boundary. */
let loadedOwners: Promise<Awaited<ReturnType<typeof buildOfflineTextureOwner>>[]> | undefined
export async function loadOfflineTextureOwner(reference = false) {
  loadedOwners ??= (async () => [await buildOfflineTextureOwner(false), await buildOfflineTextureOwner(true)])()
  return (await loadedOwners)[reference ? 1 : 0]!
}

export async function buildParticleCorrectnessBundle(entry: string, output: string) {
  return buildOfflineTextureOwner(false, { entry, output })
}

async function buildOfflineTextureOwner(reference: boolean, browser?: { entry: string; output: string }) {
  const config = await loadLocalConfig()
  const directory = path.join(config.sourceCacheDir, "evidence/tf2-browser-performance/texture-replacement/alias-investigation")
  await mkdir(directory, { recursive: true })
  const sourcePath = path.resolve(import.meta.dir, "../src/index.ts")
  let source = await Bun.file(sourcePath).text()
  if (reference) {
    const decision = "const alias = texture.frameCount === 1 ? particleTextureAlias(candidate, textures.values()) : undefined"
    if (source.split(decision).length !== 2) throw new Error("Particle alias reference owner differs")
    source = source.replace(decision, "const alias = undefined")
  }
  const decision = reference ? "const alias = undefined" : "const alias = texture.frameCount === 1 ? particleTextureAlias(candidate, textures.values()) : undefined"
  if (source.split(decision).length !== 2) throw new Error("Particle lookup timing seam differs")
  source = source.replace(decision, `const offlineAliasStart = performance.now(); ${decision};
    this.offlineAliasMilliseconds = (this.offlineAliasMilliseconds ?? 0) + performance.now() - offlineAliasStart;
    this.offlineAliasCalls = (this.offlineAliasCalls ?? 0) + 1`)
  const declaration = "class RendererOwner implements Renderer {"
  if (source.split(declaration).length !== 2) throw new Error("Renderer test owner declaration differs")
  const addition = `export class RendererOwner implements Renderer {
    offlineParticleScene(backend, inputs, states) {
      this.#backend = backend; this.#lifecycle = "Ready"; this.#deviceGeneration = 1;
      this.#particleVisibility.attach(backend);
      const disposables = new OwnedResourceGeneration(1, ++this.#sceneGeneration);
      const particleDepth = disposables.add(new SourceParticleDepth(backend.backend));
      const particleTextures = new Map(), particleBatchMaterials = new Map(), particlePipelineMeshes = new THREE.Group();
      this.#buildParticleMaterials(inputs, new Map(states), disposables, createSourceWaterFogUniforms(), particleDepth,
        particleTextures, particleBatchMaterials, particlePipelineMeshes, TSL.float(1), false);
      this.#active = { disposables, particleDepth, particleTextures, particleBatchMaterials, particlePipelineMeshes,
        reflectionTarget: null, refractionTarget: null, waterMaterials: new Map() };
      disposables.activate(); return this.#active;
    }
    offlineAttach(backend, textures) {
      this.#backend = backend;
      this.#lifecycle = "Ready";
      this.#deviceGeneration = 1;
      const disposables = new OwnedResourceGeneration(1, ++this.#sceneGeneration);
      for (const texture of new Set(textures.values())) disposables.add(texture);
      const meshes = new THREE.Group(); meshes.add(new THREE.Object3D());
      const particleDepth = disposables.add(new SourceParticleDepth(backend.backend));
      this.#active = { disposables, particleTextures: textures, particleDepth,
        particlePipelineMeshes: meshes, particleBatchMaterials: new Map() };
      this.#particleVisibility.attach(backend);
      disposables.activate();
      return disposables;
    }
    offlineInvalidate() { this.#loadOrdinal++; }
    offlineClearEffects() { this.#clearParticleBatches(); }
    offlineDispose() { this.#active.disposables.dispose(); this.#particleVisibility.dispose(); }
    offlineBuildScene(backend, map, payload, hash, request) {
      if (this.#backend !== backend) this.#particleVisibility.attach(backend);
      this.#backend = backend; this.#lifecycle = "Ready"; this.#deviceGeneration = 1;
      backend._geometries ??= { attributes: { delete() {} } };
      return this.#buildScene(map, payload, hash, validateDirectionalInputs(request.directionalTextures ?? []), request, ++this.#sceneGeneration);
    }
    offlineAdmitScene(scene) {
      scene.textureResidency.commitTransfers(scene.borrowedResources); scene.borrowedResources.length = 0;
      this.#active = scene;
      this.#world.clear(); this.#world.add(scene.group); this.#world.updateMatrixWorld(true);
    }
    async offlineWarmScene(scene, camera) {
      this.#world.clear(); this.#world.add(scene.group); this.#world.updateMatrixWorld(true);
      if (camera) this.#setCamera(camera);
      await this.#prepareReachablePipelines(scene, undefined, this.#loadOrdinal);
      await this.#prepareWaterPipelines(scene, scene.loadRequest.environment, undefined, this.#loadOrdinal);
      this.#world.remove(scene.group);
    }
  `
  if (browser) source = instrumentParticleAliasSource(source, false)
  const result = await Bun.build({ entrypoints: [browser?.entry ?? sourcePath], target: browser ? "browser" : "bun", format: "esm", minify: false,
    plugins: [{ name: "offline-owner-access-only", setup(builder) {
      builder.onLoad({ filter: /\/rendering\/src\/index\.ts$/ }, () => ({ loader: "ts", contents: source.replace(declaration, addition) + `
        export { textureFromAuthored, textureFromAuthoredCubemap };
        export { modelKey as offlineModelKey };
        export const OfflineThree = THREE;
        export { SharedTextureResidency, OwnedResourceGeneration };
        export { default as OfflineTextures } from "three/src/renderers/common/Textures.js";
        export { default as OfflineTextureUtils } from "three/src/renderers/webgpu/utils/WebGPUTextureUtils.js";
        export { offlineDecodeVisibility } from "../../../../games/tf2/browser/src/client";
      ` }))
      builder.onLoad({ filter: /\/rendering\/src\/texture-residency\.ts$/ }, async ({ path }) => {
        const original = await Bun.file(path).text(), anchor = "  snapshot(): TextureResidencySnapshot {"
        if (original.split(anchor).length !== 2) throw new Error("Residency read-only test seam differs")
        return { loader: "ts", contents: original.replace(anchor, "  offlineEntries() { return [...this.#resources]; }\n" + anchor) }
      })
      builder.onLoad({ filter: /\/tf2\/browser\/src\/client\.ts$/ }, async ({ path }) => {
        const source = await Bun.file(path).text(), start = source.indexOf("  #decodeVisibility(output: ArrayBuffer):"), end = source.indexOf("\n  }", start)
        if (start < 0 || end < start) throw new Error("Visibility parser test seam differs")
        return { loader: "ts", contents: source + "\n" + source.slice(start, end + 4).replace("  #decodeVisibility(", "export function offlineDecodeVisibility(") }
      })
    } }],
  })
  if (!result.success || result.outputs.length !== 1) throw new Error(`Offline owner bundle failed: ${result.logs}`)
  const file = browser?.output ?? path.join(directory, reference ? "owner-loop-reference.mjs" : "owner-loop.mjs")
  await Bun.write(file, result.outputs[0]!)
  return { module: browser ? undefined : await import(file + `?revision=${Bun.hash(source)}`), sourcePath, sourceSha256: new Bun.CryptoHasher("sha256").update(source).digest("hex") }
}

/** Actual Three compiler/Bindings/Textures; only the WebGPU API is recorded.
 * No rendering device, browser, display, pixels, frame loop or GPU timings. */
export async function offlinePipelineDevice(module: any, recordCommands = false) {
  const simple = offlineTextureDevice(module), device = simple.backend.device
  let submits = 0
  const programs: any[] = []
  device.features.add("core-features-and-limits"); device.features.add("float32-filterable")
  device.limits = { maxTextureDimension2D: 8192, maxTextureArrayLayers: 256, maxUniformBufferBindingSize: 65536,
    minUniformBufferOffsetAlignment: 256, maxStorageBufferBindingSize: 134217728, maxSampledTexturesPerShaderStage: 16,
    maxSamplersPerShaderStage: 16, maxUniformBuffersPerShaderStage: 12, maxStorageBuffersPerShaderStage: 8 }
  device.lost = new Promise(() => {})
  device.pushErrorScope = () => {}
  device.popErrorScope = async () => null // recording boundary has no shader validator
  device.createShaderModule = (descriptor: any) => {
    programs.push({ label: descriptor.label, sha256: new Bun.CryptoHasher("sha256").update(descriptor.code).digest("hex") })
    return { getCompilationInfo: async () => ({ messages: [] }) }
  }
  device.createSampler = () => ({})
  device.createBindGroupLayout = () => ({})
  device.createPipelineLayout = () => ({})
  device.createBindGroup = () => ({})
  const pipeline = () => ({ getBindGroupLayout: () => ({}) })
  device.createRenderPipelineAsync = async () => pipeline()
  device.createRenderPipeline = () => pipeline()
  device.createBuffer = (descriptor: any) => {
    const size = descriptor.size
    let data: ArrayBuffer | undefined
    return { size, usage: descriptor.usage, mapState: descriptor.mappedAtCreation ? "mapped" : "unmapped",
      getMappedRange() { return data ??= new ArrayBuffer(size) },
      unmap() { data = undefined }, destroy() { data = undefined } }
  }
  const createTexture = device.createTexture
  device.createTexture = (descriptor: any) => { const texture = createTexture(descriptor); return { ...texture, createView: () => ({}) } }
  device.queue.writeBuffer = () => {}
  device.queue.submit = () => { submits++; if (!recordCommands) throw new Error("Offline compiler must not submit a render frame") }
  const command = { setPipeline() {}, setBindGroup() {}, setVertexBuffer() {}, setIndexBuffer() {}, setViewport() {}, setScissorRect() {},
    draw() {}, drawIndexed() {}, drawIndirect() {}, drawIndexedIndirect() {}, setBlendConstant() {}, setStencilReference() {}, executeBundles() {}, end() {}, finish: () => ({}) }
  device.createCommandEncoder = () => ({ beginRenderPass: () => ({ ...command }), copyBufferToBuffer() {}, copyTextureToTexture() {}, clearBuffer() {}, finish: () => ({}) })
  device.createRenderBundleEncoder = () => ({ ...command })
  Object.assign(globalThis, {
    GPUBufferUsage: { MAP_READ: 1, MAP_WRITE: 2, COPY_SRC: 4, COPY_DST: 8, INDEX: 16, VERTEX: 32, UNIFORM: 64, STORAGE: 128, INDIRECT: 256, QUERY_RESOLVE: 512 },
    GPUShaderStage: { VERTEX: 1, FRAGMENT: 2, COMPUTE: 4 }, GPUMapMode: { READ: 1, WRITE: 2 },
    self: { requestAnimationFrame: () => 1, cancelAnimationFrame() {}, scheduler: { yield: async () => {} } },
  })
  Object.defineProperty(globalThis.navigator, "gpu", { configurable: true, value: { getPreferredCanvasFormat: () => "bgra8unorm" } })
  const swapchain = { width: 1280, height: 720, depthOrArrayLayers: 1, mipLevelCount: 1, sampleCount: 1, format: "bgra8unorm", createView: () => ({}) }
  const context = { configure() {}, unconfigure() {}, getCurrentTexture: () => swapchain }
  const canvas = { width: 1280, height: 720, style: {}, addEventListener() {}, removeEventListener() {},
    setAttribute() {}, getContext() { if (!recordCommands) throw new Error("Offline compilation must not acquire a canvas context"); return context } }
  const renderer = new module.OfflineThree.WebGPURenderer({ canvas, device, antialias: false })
  await renderer.init()
  return { ...simple, renderer, backend: renderer.backend, programs, recordedSubmissions: () => submits }
}

export function offlineTextureDevice(module: any) {
  const T = module.OfflineThree
  const allocations: any[] = [], writes: any[] = []
  const textureEvents: { kind: string; id: number; label: string; phase: string }[] = []
  let phase = "initial", next = 0
  const data = new WeakMap<object, any>()
  const backend: any = {
    isWebGPUBackend: true,
    get(object: object) { let record = data.get(object); if (!record) data.set(object, record = {}); return record },
    has: (object: object) => data.has(object),
    delete: (object: object) => data.delete(object),
    copyFramebufferToTexture() {}, finishRender() {},
    device: {
      features: new Set(["texture-compression-bc"]),
      createShaderModule: () => ({}), createRenderPipelineAsync: async () => ({}),
      createTexture(descriptor: any) {
        const record = { id: ++next, phase, label: descriptor.label, format: descriptor.format,
          size: { ...descriptor.size }, levels: descriptor.mipLevelCount, samples: descriptor.sampleCount, destroyed: false, destroyPhase: "" }
        allocations.push(record)
        textureEvents.push({ kind: "create", id: record.id, label: record.label, phase })
        return { ...record.size, format: record.format, destroy() { if (!record.destroyed) textureEvents.push({ kind: "destroy", id: record.id, label: record.label, phase }); record.destroyed = true; record.destroyPhase = phase }, id: record.id }
      },
      queue: {
        writeTexture(destination: any, bytes: ArrayBufferView, layout: any, extent: any) {
          writes.push({ phase, id: destination.texture.id, mip: destination.mipLevel ?? 0, bytes: bytes.byteLength,
            sourceHash: new Bun.CryptoHasher("sha256").update(new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength)).digest("hex"),
            bytesPerRow: layout.bytesPerRow, rowsPerImage: layout.rowsPerImage, width: extent.width, height: extent.height })
        },
        onSubmittedWorkDone: async () => {},
      },
    },
    utils: { getTextureSampleData: (texture: any) => ({ samples: texture.samples ?? 1, primarySamples: 1, isMSAA: false }),
      getPreferredCanvasFormat: () => "rgba8unorm", getCurrentColorFormat: () => "rgba8unorm" },
  }
  const utils = new module.OfflineTextureUtils(backend)
  backend.createTexture = (texture: any, options: any) => utils.createTexture(texture, options)
  backend.updateTexture = (texture: any, options: any) => utils.updateTexture(texture, options)
  backend.destroyTexture = (texture: any) => utils.destroyTexture(texture)
  const renderer: any = { backend, getRenderTarget: () => null, _pipelines: { getForRender() {} } }
  const textures = new module.OfflineTextures(renderer, backend, { memory: { renderTargets: 0 }, createTexture() {}, destroyTexture() {} })
  // The browser API constants, not a graphics device or software renderer.
  Object.assign(globalThis, { GPUTextureUsage: { COPY_SRC: 1, COPY_DST: 2, TEXTURE_BINDING: 4, STORAGE_BINDING: 8, RENDER_ATTACHMENT: 16 } })
  return { T, backend, renderer, textures, allocations, writes, textureEvents, phase: (value: string) => { phase = value },
    compile(values: Map<string, any>, failure?: () => void) {
      renderer.compileAsync = async () => { for (const texture of values.values()) textures.updateTexture(texture); failure?.() }
    },
  }
}
