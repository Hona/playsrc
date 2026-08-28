import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import { withReferenceGpuUploads, type UploadBatchBackend } from "./webgpu-upload-batch"
import { RendererFrameInstrumentation } from "./frame-instrumentation"
import { withImmediateGpuSubmissions } from "./webgpu-submission-batch"
import { sourceModelBoneCount } from "./source-model-skinning"

type Request = { label: string; pass: string; allowRigid: boolean; resolve(value: unknown): void; reject(error: unknown): void }

export function unpackGpuRgbaRows(data: Uint8Array | Float32Array, width: number, height: number): Uint8Array | Float32Array {
  if (!Number.isSafeInteger(width) || width < 1 || !Number.isSafeInteger(height) || height < 1) throw new Error("GPU capture dimensions are invalid")
  const row = width * 4
  if (data.length === row * height) return data
  const stride = Math.ceil(row * data.BYTES_PER_ELEMENT / 256) * 256 / data.BYTES_PER_ELEMENT
  if (data.length !== (height - 1) * stride + row) throw new Error("GPU capture row layout differs")
  const output = data instanceof Float32Array ? new Float32Array(row * height) : new Uint8Array(row * height)
  for (let y = 0; y < height; y += 1) output.set(data.subarray(y * stride, y * stride + row), y * row)
  return output
}

// Explicitly installed only by the headed acceptance harness. The normal
// application never imports this module or allocates these diagnostic targets.
export function installSkinningEvidence(referenceRender?: (draw: () => void) => void) {
  const prototype = THREE.WebGPURenderer.prototype
  const render = prototype.render
  const instrument = RendererFrameInstrumentation.prototype.pass
  let currentPass = ""
  RendererFrameInstrumentation.prototype.pass = function (identity, callback) {
    const prior = currentPass
    currentPass = identity
    try { return instrument.call(this, identity, callback) }
    finally { currentPass = prior }
  }
  let requested: Request | undefined
  let capturing = false
  let disposed = false
  const targets = new Map<string, THREE.RenderTarget>()
  let targetOwner: THREE.WebGPURenderer | undefined
  let targetSize = ""
  const normal = new THREE.MeshBasicNodeMaterial({ side: THREE.DoubleSide })
  normal.colorNode = TSL.vec4(TSL.normalView.mul(0.5).add(0.5), 1)
  normal.toneMapped = false
  normal.fog = false
  const depth = new THREE.MeshBasicNodeMaterial({ side: THREE.DoubleSide })
  depth.colorNode = TSL.vec4(TSL.positionView.z.negate(), 0, 0, 1)
  depth.toneMapped = false
  depth.fog = false

  prototype.render = function (scene: THREE.Scene, camera: THREE.Camera) {
    const result = render.call(this, scene, camera)
    const request = requested
    if (!request || capturing || disposed) return result
    const meshes: THREE.Mesh[] = []
    scene.traverseVisible(object => {
      if ((object instanceof THREE.SkinnedMesh || currentPass === "hud-model" && object instanceof THREE.Mesh) && camera.layers.test(object.layers)) meshes.push(object)
    })
    const pass = currentPass
    if (request.pass !== pass || !(request.allowRigid || (pass === "hud-model" ? meshes.length > 0 : meshes.some(mesh => mesh instanceof THREE.SkinnedMesh && mesh.skeleton.bones.length > 1)))) return result
    requested = undefined
    capturing = true
    const renderer = this
    const originalTarget = renderer.getRenderTarget()
    const size = originalTarget ? new THREE.Vector2(originalTarget.width, originalTarget.height) : renderer.getDrawingBufferSize(new THREE.Vector2())
    if (targetOwner !== renderer || targetSize !== `${size.x}:${size.y}`) {
      for (const target of targets.values()) target.dispose()
      targets.clear()
      targetOwner = renderer
      targetSize = `${size.x}:${size.y}`
    }
    const previous = {
      target: renderer.getRenderTarget(), viewport: renderer.getViewport(new THREE.Vector4()),
      scissor: renderer.getScissor(new THREE.Vector4()), scissorTest: renderer.getScissorTest(),
      autoClear: renderer.autoClear, override: scene.overrideMaterial, background: scene.background,
      autoClearColor: renderer.autoClearColor, autoClearDepth: renderer.autoClearDepth, autoClearStencil: renderer.autoClearStencil,
    }
    const viewport = originalTarget?.viewport ?? previous.viewport.clone().multiplyScalar(renderer.getPixelRatio())
    const scissor = originalTarget?.scissor ?? previous.scissor.clone().multiplyScalar(renderer.getPixelRatio())
    const scissorTest = originalTarget?.scissorTest ?? previous.scissorTest
    const configure = (target: THREE.RenderTarget) => {
      target.viewport.copy(viewport)
      target.scissor.copy(scissor)
      target.scissorTest = scissorTest
    }
    const reads: Promise<unknown>[] = []
    const bundles: any[] = []
    scene.traverse(object => { if ((object as any).isBundleGroup) bundles.push(object) })
    const read = (target: THREE.RenderTarget) => withImmediateGpuSubmissions(
      (renderer.backend as any).device.queue,
      () => renderer.readRenderTargetPixelsAsync(target, 0, 0, size.x, size.y) as Promise<Uint8Array | Float32Array>,
    ).then(data => unpackGpuRgbaRows(data, size.x, size.y))
    try {
      renderer.autoClear = true
      renderer.autoClearColor = true
      renderer.autoClearDepth = true
      renderer.autoClearStencil = true
      renderer.setScissorTest(false)
      for (const plane of ["color", "normal", "depth"] as const) {
        for (const bundle of bundles) bundle.isBundleGroup = plane === "color"
        scene.overrideMaterial = plane === "color" ? previous.override : plane === "normal" ? normal : depth
        scene.background = plane === "color" ? previous.background : null
        const pair: Promise<Uint8Array | Float32Array>[] = []
        const drawOrders: number[][][] = []
        const drawPair = () => {
          if (!referenceRender) return render.call(renderer, scene, camera)
          const backend = renderer.backend as any, original = backend.draw
          const order: number[][] = []
          backend.draw = function (draw: any, ...args: any[]) {
            const geometry = draw.geometry
            order.push([draw.object.id, draw.material.id, geometry.id, geometry.drawRange.start, geometry.drawRange.count,
              geometry.index?.version ?? -1, geometry.index?.count ?? 0, geometry.attributes.position?.version ?? -1, geometry.attributes.position?.count ?? 0])
            return original.call(this, draw, ...args)
          }
          try { return render.call(renderer, scene, camera) }
          finally { backend.draw = original; drawOrders.push(order) }
        }
        for (const reference of [false, true]) {
          const key = `${plane}:${size.x}:${size.y}`
          let target = targets.get(key)
          if (!target) {
            target = new THREE.RenderTarget(size.x, size.y, {
              type: plane === "color" ? THREE.UnsignedByteType : THREE.FloatType,
              format: THREE.RGBAFormat, depthBuffer: true,
            })
            target.texture.name = `playsrc-skinning-evidence-${plane}`
            targets.set(key, target)
          }
          configure(target)
          renderer.setRenderTarget(target)
          renderer.setViewport(0, 0, size.x, size.y)
          if (reference) (referenceRender ?? (draw => withReferenceGpuUploads(renderer.backend as unknown as UploadBatchBackend, draw)))(drawPair)
          else drawPair()
          pair.push(read(target))
          if (reference && plane === "color") {
            ;(referenceRender ?? (draw => withReferenceGpuUploads(renderer.backend as unknown as UploadBatchBackend, draw)))(drawPair)
            pair.push(read(target))
          }
        }
        if (plane === "color") {
          const key = `absent:${size.x}:${size.y}`
          let absent = targets.get(key)
          if (!absent) {
            absent = new THREE.RenderTarget(size.x, size.y, { depthBuffer: true })
            absent.texture.name = "playsrc-skinning-evidence-absent"
            targets.set(key, absent)
          }
          configure(absent)
          renderer.setRenderTarget(absent)
          for (const mesh of meshes) mesh.visible = false
          try { render.call(renderer, scene, camera) }
          finally { for (const mesh of meshes) mesh.visible = true }
          pair.push(read(absent))
        }
        reads.push(Promise.all(pair).then(async ([optimized, reference, repeatedReference, absent]) => {
          if (optimized.length !== reference.length) throw new Error("skinning parity plane lengths differ")
          let maximumAbsolute = 0, mismatches = 0, actorPixels = 0, referenceMismatches = 0, minimumValue = Infinity, maximumValue = -Infinity
          const channels = [0, 0, 0, 0]
          for (let index = 0; index < optimized.length; index += 1) {
            if (!Number.isFinite(optimized[index]) || !Number.isFinite(reference[index])) throw new Error(`non-finite ${plane} pixel`)
            const difference = Math.abs(optimized[index]! - reference[index]!)
            minimumValue = Math.min(minimumValue, optimized[index]!)
            maximumValue = Math.max(maximumValue, optimized[index]!)
            channels[index % 4] = Math.max(channels[index % 4]!, Math.abs(optimized[index]!))
            maximumAbsolute = Math.max(maximumAbsolute, difference)
            if (difference !== 0) mismatches += 1
            if (repeatedReference && repeatedReference[index] !== reference[index]) referenceMismatches += 1
            if (plane === "depth" && index % 4 === 1 && optimized[index]! > 0.5) actorPixels += 1
            if (absent && index % 4 === 0 && (optimized[index] !== absent[index] || optimized[index + 1] !== absent[index + 1] || optimized[index + 2] !== absent[index + 2])) actorPixels += 1
          }
          const digest = async (values: Uint8Array | Float32Array) => Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", values as BufferSource)), byte => byte.toString(16).padStart(2, "0")).join("")
          const [sha256, referenceSha256] = await Promise.all([digest(optimized), digest(reference)])
          return { plane, values: optimized.length, mismatches, maximumAbsolute, actorPixels, referenceMismatches, minimumValue, maximumValue, channels, sha256, referenceSha256,
            ...(referenceRender ? { drawOrders, identicalDrawOrder: drawOrders.slice(1).every(order => JSON.stringify(order) === JSON.stringify(drawOrders[0])) } : {}) }
        }))
      }
    } catch (error) {
      request.reject(error)
    } finally {
      scene.overrideMaterial = previous.override
      for (const bundle of bundles) bundle.isBundleGroup = true
      scene.background = previous.background
      renderer.setRenderTarget(previous.target)
      renderer.setViewport(previous.viewport)
      renderer.setScissor(previous.scissor)
      renderer.setScissorTest(previous.scissorTest)
      renderer.autoClear = previous.autoClear
      renderer.autoClearColor = previous.autoClearColor
      renderer.autoClearDepth = previous.autoClearDepth
      renderer.autoClearStencil = previous.autoClearStencil
    }
    const timestamps = withImmediateGpuSubmissions((renderer.backend as any).device.queue, () => renderer.resolveTimestampsAsync("render"))
    void Promise.all([Promise.all(reads), timestamps]).then(([planes]) => request.resolve({
      label: request.label, pass, width: size.x, height: size.y, planes,
      meshes: meshes.length,
      materials: meshes.map(mesh => String(mesh.userData.materialIdentity ?? "")),
      palettes: [...new Set(meshes.filter(mesh => mesh instanceof THREE.SkinnedMesh).map(mesh => (mesh as THREE.SkinnedMesh).skeleton))].map(skeleton => ({ authored: sourceModelBoneCount(skeleton), capacity: skeleton.bones.length, sourceBytes: sourceModelBoneCount(skeleton) * 48, gpuBytes: skeleton.boneMatrices.byteLength })),
    }), request.reject).finally(() => { capturing = false })
    return result
  }

  return {
    capture(label: string, pass: Request["pass"], allowRigid = false): Promise<unknown> {
      if (requested || capturing || disposed) return Promise.reject(new Error("skinning parity capture is busy or disposed"))
      return new Promise((resolve, reject) => { requested = { label, pass, allowRigid, resolve, reject } })
    },
    dispose() {
      disposed = true
      prototype.render = render
      RendererFrameInstrumentation.prototype.pass = instrument
      requested?.reject(new Error("skinning parity capture disposed"))
      requested = undefined
      for (const target of targets.values()) target.dispose()
      targets.clear()
      targetOwner = undefined
      targetSize = ""
      normal.dispose()
      depth.dispose()
    },
  }
}
