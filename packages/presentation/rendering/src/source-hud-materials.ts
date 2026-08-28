// Source HUD material behavior is adapted from Valve Source SDK 2013;
// the Source 1 SDK License applies.
import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import { createSourceRefractMaterial } from "./source-refract"

export type HudTexture = Readonly<{ width: number; height: number; clampS: boolean; clampT: boolean; noLod: boolean; encoding: "png" | "rgba-deflate"; mips: readonly string[] }>
export type HudMaterial = Readonly<
  | { kind: "solid"; color: readonly [number, number, number, number] }
  | { kind: "refract"; normal: HudTexture; tint: HudTexture; amount: number; blur: 0 | 1 }
  | { kind: "two-texture-additive"; base: HudTexture; second: HudTexture }
>
export type HudMaterialDraw = Readonly<{
  material: number
  bounds: readonly [number, number, number, number]
  uv: readonly [number, number, number, number]
  secondUv?: readonly [number, number, number, number]
}>
export type HudMaterialFrame = Readonly<{ materials: readonly HudMaterial[]; draws: readonly HudMaterialDraw[] }>

/** One owned framebuffer snapshot for an ordered VGUI material batch, never a DOM backdrop. */
export class SourceHudMaterials {
  readonly #scene = new THREE.Scene()
  readonly #camera = new THREE.OrthographicCamera(0, 1, 0, 1, 0, 2)
  readonly #framebuffer = new THREE.FramebufferTexture(1, 1)
  readonly #resolveMaterial = new THREE.MeshBasicNodeMaterial({ depthTest: false, depthWrite: false })
  readonly #resolve = new THREE.QuadMesh(this.#resolveMaterial)
  readonly #materials: THREE.MeshBasicNodeMaterial[] = []
  readonly #textures: THREE.Texture[] = []
  readonly #images: ImageBitmap[] = []
  readonly #meshes: THREE.Mesh[] = []
  #input?: readonly HudMaterial[]
  #draws?: readonly HudMaterialDraw[]
  #width = 0
  #height = 0
  #disposed = false

  constructor(readonly quality?: Readonly<{ mipOffset: number; trilinear: boolean; anisotropy: number }>) {
    this.#camera.position.z = 1
    this.#framebuffer.name = "hud-materials:framebuffer"
    this.#framebuffer.minFilter = this.#framebuffer.magFilter = THREE.LinearFilter
    // The opaque canvas presents RenderOutput's RGB but discards its alpha.
    // Resolve that displayed RGB back to linear before VGUI blends into it.
    // Sampling the alpha-bearing intermediate instead brightens scope edges
    // over translucent world surfaces when the scope changes destination alpha.
    const presented = TSL.renderOutput(TSL.texture(this.#framebuffer, TSL.screenUV), THREE.NoToneMapping, THREE.SRGBColorSpace)
    this.#resolveMaterial.colorNode = TSL.vec4(TSL.sRGBTransferEOTF(presented.rgb.clamp(0, 1)), 1)
    this.#resolveMaterial.toneMapped = false
  }

  get prepared(): boolean { return this.#input !== undefined }

  async prepare(input: readonly HudMaterial[]): Promise<void> {
    if (this.#disposed) throw new Error("HUD material owner is disposed")
    if (this.#input === input) return
    if (this.#input) throw new Error("HUD material resources changed without retiring their renderer")
    try {
      await this.#prepare(input)
    } catch (error) {
      this.dispose()
      throw error
    }
  }

  async #prepare(input: readonly HudMaterial[]): Promise<void> {
    const texture = async (source: HudTexture, color: boolean) => {
      const offset = source.noLod ? 0 : Math.min(Math.max(0, this.quality?.mipOffset ?? 0), source.mips.length - 1)
      const images = await Promise.all(source.mips.slice(offset).map(async (url, level) => {
        const mip = level + offset, width = Math.max(1, source.width >> mip), height = Math.max(1, source.height >> mip)
        if (source.encoding === "rgba-deflate") {
          const compressed = Uint8Array.from(atob(url), value => value.charCodeAt(0))
          const data = new Uint8Array(await new Response(new Blob([compressed]).stream().pipeThrough(new DecompressionStream("deflate"))).arrayBuffer())
          if (data.byteLength !== width * height * 4) throw new Error("HUD scalar texture dimensions differ")
          return { data, width, height }
        }
        const image = await createImageBitmap(await (await fetch(url)).blob(), { premultiplyAlpha: "none", colorSpaceConversion: "none" })
        if (this.#disposed) { image.close(); throw new Error("HUD material owner was disposed during texture decoding") }
        this.#images.push(image)
        if (image.width !== width || image.height !== height) throw new Error("HUD material texture dimensions differ")
        return image
      }))
      if (this.#disposed) throw new Error("HUD material owner was disposed during texture decoding")
      const base = images[0]!
      const value = "data" in base ? new THREE.DataTexture(base.data, base.width, base.height, THREE.RGBAFormat, THREE.UnsignedByteType) : new THREE.Texture(base)
      if (images.length > 1) value.mipmaps = images
      value.name = `hud-materials:input=${this.#textures.length}`
      value.flipY = false
      value.colorSpace = color ? THREE.SRGBColorSpace : THREE.NoColorSpace
      value.wrapS = source.clampS ? THREE.ClampToEdgeWrapping : THREE.RepeatWrapping
      value.wrapT = source.clampT ? THREE.ClampToEdgeWrapping : THREE.RepeatWrapping
      value.magFilter = THREE.LinearFilter
      value.minFilter = images.length > 1
        ? this.quality?.trilinear || (this.quality?.anisotropy ?? 1) > 1 ? THREE.LinearMipmapLinearFilter : THREE.LinearMipmapNearestFilter
        : THREE.LinearFilter
      value.anisotropy = this.quality?.anisotropy ?? 1
      value.generateMipmaps = false
      value.needsUpdate = true
      this.#textures.push(value)
      return value
    }
    for (const source of input) {
      let material: THREE.MeshBasicNodeMaterial
      if (source.kind === "refract") {
        // RefractTintTexture is a *linear RGB multiplier*, not coverage. The
        // normal alpha owns both distortion strength and source-alpha blending.
        material = createSourceRefractMaterial({
          state: { refractAmount: source.amount, refractTint: [1, 1, 1], blurAmount: source.blur, ignoreDepth: true },
          normal: await texture(source.normal, false), tint: await texture(source.tint, true), framebuffer: this.#framebuffer,
        }).material
      } else {
        material = new THREE.MeshBasicNodeMaterial({ depthTest: false, depthWrite: false, transparent: true })
        if (source.kind === "two-texture-additive") {
          material.colorNode = TSL.texture(await texture(source.base, true), TSL.uv())
            .mul(TSL.texture(await texture(source.second, true), TSL.uv(1)))
          material.blending = THREE.AdditiveBlending
        } else material.colorNode = TSL.vec4(...source.color)
      }
      material.side = THREE.FrontSide
      material.toneMapped = false
      this.#materials.push(material)
    }
    this.#input = input
  }

  render(renderer: THREE.WebGPURenderer, frame: HudMaterialFrame, width: number, height: number, dpr: number): void {
    if (this.#disposed) throw new Error("HUD material owner is disposed")
    if (frame.materials !== this.#input) throw new Error("HUD material batch was not prepared")
    const pixelsWide = Math.floor(width * dpr), pixelsTall = Math.floor(height * dpr)
    if (this.#framebuffer.image.width !== pixelsWide || this.#framebuffer.image.height !== pixelsTall) {
      this.#framebuffer.image.width = pixelsWide
      this.#framebuffer.image.height = pixelsTall
      this.#framebuffer.needsUpdate = true
    }
    if (this.#draws !== frame.draws || this.#width !== width || this.#height !== height) {
      this.#camera.right = width
      this.#camera.bottom = height
      this.#camera.updateProjectionMatrix()
      for (let index = 0; index < frame.draws.length; index++) {
        const draw = frame.draws[index]!
        if (this.#draws?.[index] === draw && this.#width === width && this.#height === height) continue
        let mesh = this.#meshes[index]
        if (!mesh) {
          const geometry = new THREE.BufferGeometry()
          geometry.setAttribute("position", new THREE.BufferAttribute(new Float32Array(12), 3))
          geometry.setAttribute("uv", new THREE.BufferAttribute(new Float32Array(8), 2))
          geometry.setAttribute("uv1", new THREE.BufferAttribute(new Float32Array(8), 2))
          geometry.setIndex([0, 1, 2, 0, 2, 3])
          mesh = new THREE.Mesh(geometry, this.#materials[draw.material])
          mesh.frustumCulled = false
          mesh.renderOrder = index
          this.#scene.add(mesh)
          this.#meshes.push(mesh)
        }
        mesh.material = this.#materials[draw.material]!
        const [x, y, w, h] = draw.bounds
        mesh.visible = w > 0 && h > 0
        const positions = mesh.geometry.getAttribute("position") as THREE.BufferAttribute
        positions.array.set([x, y, 0, x, y + h, 0, x + w, y + h, 0, x + w, y, 0])
        positions.needsUpdate = true
        for (const [name, coordinates] of [["uv", draw.uv], ["uv1", draw.secondUv ?? draw.uv]] as const) {
          const attribute = mesh.geometry.getAttribute(name) as THREE.BufferAttribute
          const [u0, v0, u1, v1] = coordinates
          attribute.array.set([u0, v0, u0, v1, u1, v1, u1, v0])
          attribute.needsUpdate = true
        }
      }
      for (let index = frame.draws.length; index < this.#meshes.length; index++) this.#meshes[index]!.visible = false
      this.#draws = frame.draws
      this.#width = width
      this.#height = height
    }
    // UpdateRefractTexture precedes every scope quadrant, so overlaps must not
    // sample a previously tinted quadrant. No readback or CPU pixel copies.
    const autoClear = renderer.autoClear
    try {
      renderer.autoClear = false
      renderer.copyFramebufferToTexture(this.#framebuffer)
      this.#resolve.render(renderer)
      renderer.copyFramebufferToTexture(this.#framebuffer)
      renderer.render(this.#scene, this.#camera)
    } finally { renderer.autoClear = autoClear }
  }

  dispose(): void {
    if (this.#disposed) return
    this.#disposed = true
    for (const mesh of this.#meshes) mesh.geometry.dispose()
    for (const material of this.#materials) material.dispose()
    for (const texture of this.#textures) texture.dispose()
    for (const image of this.#images) image.close()
    this.#framebuffer.dispose()
    this.#resolveMaterial.dispose()
    this.#scene.clear()
  }
}
