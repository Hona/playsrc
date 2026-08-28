// Cloak blended pass from Valve Source SDK 2013; Source 1 SDK License applies.
import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import { bindSourceModelMesh } from "./source-model-skinning"
import { sourceModelWorldNormal } from "./source-model-lighting"

type V3 = readonly [number, number, number]
const SORT_VIEWS = new WeakMap<THREE.Group, { camera: THREE.Camera; call: number; depth: number; viewmodelOrder: number | null }>()
const SORT_POSITION = new THREE.Vector3()
export type SourceCloakState = Readonly<{ enabled: boolean; factor: number; colorTint: V3; refractAmount: number }>
export type SourceCloakBinding = Readonly<{ localFactor: number; worldFactor: number; rawFactor: number; playerTint: V3; local: boolean; player: boolean }>

const POISSON = [[0, 0], [-0.0876, 0.9703], [0.4802, 0.5651], [0.1851, 0.1580], [-0.2616, -0.0617], [-0.5477, -0.6603], [-0.5325, 0.0711], [-0.0751, -0.8954], [0.6384, 0.4054]] as const
const saturate = (v: number) => Math.min(1, Math.max(0, v))

export function sourceCloakPassState(factor: number) {
  if (!Number.isFinite(factor)) throw new Error("Invalid cloak factor")
  return { standard: saturate(1 + saturate(factor) * (-0.35 - 1)) > 0.4, cloak: factor > 0 && factor < 1 }
}

export function evaluateSourceCloakPixel(input: Readonly<{
  factor: number; refractAmount: number; tint: V3; normalDotView: number;
  coordinate: readonly [number, number]; projectedNormal: readonly [number, number];
  sample: (uv: readonly [number, number]) => V3;
}>): readonly [number, number, number, number] {
  sourceCloakPassState(input.factor)
  const f = saturate(input.factor), fresnel = 1 - saturate(input.normalDotView)
  const mask = saturate((saturate(1 + (fresnel - 2.35) * f) - 0.4) / 0.025)
  const alpha = 1 - mask * mask * (3 - 2 * mask)
  const color = [0, 0, 0]
  for (const [x, y] of POISSON) {
    const pixel = input.sample([
      input.coordinate[0] + input.projectedNormal[0] * input.refractAmount * (1 - f) + x * 0.05 * (1 - f),
      input.coordinate[1] + input.projectedNormal[1] * input.refractAmount * (1 - f) + y * 0.05 * (1 - f),
    ])
    for (let channel = 0; channel < 3; channel++) color[channel]! += pixel[channel]! / 9
  }
  const brightness = (fresnel * 0.4 + 0.8) * (1 - f * f) + f * f
  const tintStrength = saturate((f - 0.75) * 4)
  return [color[0]! * brightness * (input.tint[0] * (1 - tintStrength) + tintStrength), color[1]! * brightness * (input.tint[1] * (1 - tintStrength) + tintStrength), color[2]! * brightness * (input.tint[2] * (1 - tintStrength) + tintStrength), alpha]
}

/** Shared copies per framebuffer format, before each model occurrence's first pass. */
export class SourceCloakFramebuffer {
  #texture = new THREE.FramebufferTexture(1, 1)
  readonly #textures = new Map<number, THREE.FramebufferTexture>()
  readonly #nodes = new Set<WeakRef<ReturnType<typeof TSL.texture>>>()
  #call = -1
  #owner: object | undefined
  readonly #size = new THREE.Vector2()
  copies = 0
  constructor() {
    this.#textures.set(THREE.UnsignedByteType, this.#texture)
    this.#configure(this.#texture)
  }
  get texture(): THREE.FramebufferTexture { return this.#texture }
  get samplerCount(): number { return this.#nodes.size }
  #configure(texture: THREE.FramebufferTexture): void {
    texture.name = "cloak:framebuffer"
    texture.minFilter = THREE.LinearFilter; texture.magFilter = THREE.LinearFilter; texture.generateMipmaps = false
  }
  sample(): ReturnType<typeof TSL.texture> {
    const node = TSL.texture(this.#texture)
    this.#nodes.add(new WeakRef(node))
    return node
  }
  release(node: ReturnType<typeof TSL.texture>): void {
    for (const reference of this.#nodes) {
      const retained = reference.deref()
      if (!retained || retained === node) this.#nodes.delete(reference)
    }
  }
  capture(renderer: THREE.WebGPURenderer, owner: object): void {
    if (this.#call === renderer.info.calls && this.#owner === owner) return
    const target = renderer.getRenderTarget()
    // Water and the completed linear framebuffer can use different formats.
    // Keep each live format until disposal: replacing a texture while commands
    // for an earlier pass are still batched would destroy a submitted input.
    const type = target?.texture.type ?? THREE.UnsignedByteType
    let texture = this.#textures.get(type)
    if (!texture) {
      texture = new THREE.FramebufferTexture(1, 1)
      texture.type = type
      this.#configure(texture)
      this.#textures.set(type, texture)
    }
    if (this.#texture !== texture) {
      this.#texture = texture
      for (const reference of this.#nodes) {
        const node = reference.deref()
        if (node) node.value = texture
        else this.#nodes.delete(reference)
      }
    }
    if (target) this.#size.set(target.width, target.height)
    else renderer.getDrawingBufferSize(this.#size)
    if (this.texture.image.width !== this.#size.x || this.texture.image.height !== this.#size.y) {
      this.texture.image.width = this.#size.x
      this.texture.image.height = this.#size.y
      this.texture.needsUpdate = true
    }
    renderer.copyFramebufferToTexture(this.texture)
    this.#call = renderer.info.calls
    this.#owner = owner
    this.copies++
  }
  dispose(): void {
    for (const texture of this.#textures.values()) texture.dispose()
    this.#textures.clear(); this.#nodes.clear(); this.#owner = undefined; this.#call = -1
  }
}

export function createSourceCloakMaterial(framebuffer: SourceCloakFramebuffer, state: SourceCloakState, normal?: THREE.Texture) {
  const factor = TSL.uniform(0), tint = TSL.uniform(new THREE.Vector3(...state.colorTint))
  const vertexNormal = sourceModelWorldNormal.toVarying()
  const viewVector = TSL.positionWorld.sub(TSL.cameraPosition).normalize().toVarying()
  let worldNormal: any = vertexNormal.normalize()
  if (normal) {
    const tangent = TSL.modelWorldMatrix.mul(TSL.vec4(TSL.tangentLocal, 0)).xyz.toVarying()
    const binormal = vertexNormal.cross(tangent).mul(TSL.tangentGeometry.w).toVarying()
    const bump = TSL.texture(normal).xyz.mul(2).sub(1)
    worldNormal = tangent.mul(bump.x).add(binormal.mul(bump.y)).add(vertexNormal.mul(bump.z))
  }
  const projected = TSL.cameraProjectionMatrix.mul(TSL.cameraViewMatrix).mul(TSL.vec4(worldNormal, 0)).xy
  const f = factor.clamp(0, 1), scale = TSL.float(1).sub(f)
  const uv = TSL.screenUV.add(projected.mul(scale).mul(state.refractAmount))
  const scene = framebuffer.sample()
  let color: any = TSL.vec3(0)
  for (const [x, y] of POISSON) color = color.add(scene.sample(uv.add(TSL.vec2(x, y).mul(scale).mul(0.05))).rgb)
  color = color.div(9)
  const fresnel = TSL.float(1).sub(vertexNormal.dot(viewVector.negate().normalize()).clamp(0, 1))
  const alpha = TSL.float(1).sub(TSL.smoothstep(0.4, 0.425, TSL.mix(1, fresnel.sub(1.35), f).clamp(0, 1)))
  color = color.mul(TSL.mix(fresnel.mul(0.4).add(0.8), 1, f.mul(f)))
    .mul(TSL.mix(tint, TSL.vec3(1), f.sub(0.75).mul(4).clamp(0, 1)))
  const material = new THREE.MeshBasicNodeMaterial({ transparent: true, depthTest: true, depthWrite: true,
    blending: THREE.CustomBlending, blendSrc: THREE.SrcAlphaFactor, blendDst: THREE.OneMinusSrcAlphaFactor,
    blendSrcAlpha: THREE.ZeroFactor, blendDstAlpha: THREE.OneFactor })
  material.colorNode = TSL.vec4(color, alpha)
  material.toneMapped = false
  material.addEventListener("dispose", () => framebuffer.release(scene))
  return { material, factor, tint }
}

type Part = ReturnType<typeof createSourceCloakMaterial> & { base: THREE.Mesh; overlay: THREE.Mesh; proxy: number; state: SourceCloakState; originalBlending: THREE.Blending; originalVisible: boolean }
export class SourceModelCloak {
  readonly #parts: Part[] = []
  readonly #owner = {}
  constructor(meshes: readonly THREE.Mesh[], framebuffer: SourceCloakFramebuffer,
    material: (identity: string) => Readonly<{ cloakProxy: number; state: { cloak?: SourceCloakState } }> | undefined,
    normal: (identity: string) => THREE.Texture | undefined) {
    meshes.forEach((base, index) => {
      const identity = String(base.userData.materialIdentity).toLowerCase(), authored = material(identity), state = authored?.state.cloak
      if (!state?.enabled) return
      if (Array.isArray(base.material)) throw new Error("Cloak requires an authored primitive material")
      const shader = createSourceCloakMaterial(framebuffer, state, normal(identity))
      shader.material.side = base.material.side
      const overlay = base instanceof THREE.SkinnedMesh ? bindSourceModelMesh(base.geometry, shader.material, base.skeleton) : new THREE.Mesh(base.geometry, shader.material)
      overlay.layers.mask = base.layers.mask
      overlay.userData = { dynamicMaterial: true, sourceCloakOverlay: true, materialIdentity: identity }
      overlay.frustumCulled = base.frustumCulled
      overlay.visible = false
      const copy = (renderer: THREE.WebGPURenderer) => { if (overlay.visible) framebuffer.capture(renderer, this.#owner) }
      base.onBeforeRender = copy
      overlay.onBeforeRender = copy
      base.userData.sourceCloakOrder = index * 2
      overlay.userData.sourceCloakOrder = index * 2 + 1
      base.parent!.add(overlay)
      this.#parts.push({ ...shader, base, overlay, proxy: authored!.cloakProxy, state, originalBlending: base.material.blending, originalVisible: base.material.visible })
    })
  }
  update(binding: SourceCloakBinding | null | undefined, viewmodel: boolean, root: THREE.Group, camera: THREE.Camera, identity: number): void {
    const viewmodelOrder = viewmodel ? identity : null
    const prior = SORT_VIEWS.get(root)
    if (prior?.camera !== camera || prior.viewmodelOrder !== viewmodelOrder) SORT_VIEWS.set(root, { camera, call: -1, depth: 0, viewmodelOrder })
    for (const part of this.#parts) {
      part.overlay.geometry = part.base.geometry
      const proxy = part.proxy & 3
      const factor = proxy === 0 ? part.state.factor : !binding ? 0 : proxy === 2 ? binding.worldFactor
        : proxy === 3 ? viewmodel ? binding.localFactor : binding.rawFactor
        : binding.local ? binding.localFactor : binding.worldFactor
      const passes = sourceCloakPassState(factor)
      part.factor.value = factor
      part.tint.value.set(...(binding?.player && (part.proxy & 4) !== 0 ? binding.playerTint : part.state.colorTint))
      const base = part.base.material as THREE.Material
      base.visible = part.originalVisible && passes.standard
      if (passes.cloak) { base.transparent = true; base.depthWrite = true }
      base.blending = part.originalBlending
      part.overlay.visible = part.base.visible && part.originalVisible && passes.cloak
      part.base.userData.sourceCloakRoot = passes.cloak ? root : undefined
      part.overlay.userData.sourceCloakRoot = root
    }
  }
}

function cloakSortDepth(root: THREE.Group | undefined, original: number, renderCall: number): number {
    const view = root && SORT_VIEWS.get(root)
    if (!view) return original
    // The renderer has now updated both world and camera matrices. Resolve at
    // sorting, not at simulation staging: late mouse input and water views can
    // change the camera without a new model publication.
    if (view.call !== renderCall) {
      view.depth = SORT_POSITION.setFromMatrixPosition(root!.matrixWorld).applyMatrix4(view.camera.matrixWorldInverse).applyMatrix4(view.camera.projectionMatrix).z
      view.call = renderCall
    }
    return view.depth
}

/** Keep each model's standard/cloak draws contiguous at its translucent depth. */
export function sourceCloakTransparentSort(a: any, b: any, renderCall: number): number {
  if (a.groupOrder !== b.groupOrder) return a.groupOrder - b.groupOrder
  if (a.renderOrder !== b.renderOrder) return a.renderOrder - b.renderOrder
  const ar = a.object.userData.sourceCloakRoot, br = b.object.userData.sourceCloakRoot
  if (ar && ar === br) return a.object.userData.sourceCloakOrder - b.object.userData.sourceCloakOrder
  const av = ar && SORT_VIEWS.get(ar)?.viewmodelOrder, bv = br && SORT_VIEWS.get(br)?.viewmodelOrder
  if (typeof av === "number" && typeof bv === "number") return av - bv
  const az = cloakSortDepth(ar, a.z, renderCall), bz = cloakSortDepth(br, b.z, renderCall)
  return az !== bz ? bz - az : a.id - b.id
}
