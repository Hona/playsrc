import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import type { SourceModelEyeUniforms, SourceModelLightingUniforms } from "./source-model-lighting"

const reference = (path: string, type: string) => TSL.reference(`userData.${path}.value`, type) as ReturnType<typeof TSL.uniform>

/** One immutable graph family per scene/exposure/fog domain. Values are read
 * from the drawn primitive, not captured from the first actor using a shader.
 * The owner is scene-local, with an explicit handoff for an identical verified
 * world/resource closure on the same device. It is never a module-global cache;
 * every new draw binds the replacement's primitive values. */
export class ModelLightingGraphs {
  readonly lighting: SourceModelLightingUniforms = Object.freeze({
    ambientEnabled: reference("sourceLighting.ambientEnabled", "float"),
    cameraPosition: reference("sourceLighting.cameraPosition", "vec3"),
    ambient: Object.freeze(Array.from({ length: 6 }, (_, side) => reference(`sourceLighting.ambient.${side}`, "vec3"))) as SourceModelLightingUniforms["ambient"],
    local: Object.freeze(Array.from({ length: 4 }, (_, index) => Object.freeze(Object.fromEntries(
      ["enabled", "kind", "color", "position", "direction", "attenuation", "falloff", "theta", "phi"].map(name =>
        [name, reference(`sourceLighting.local.${index}.${name}`, ["color", "position", "direction", "attenuation"].includes(name) ? "vec3" : "float")]),
    )))) as unknown as SourceModelLightingUniforms["local"],
  })
  readonly eyes = Object.freeze(Object.fromEntries(
    ["irisU", "irisV", "glintU", "glintV", "origin"].map(name =>
      [name, reference(`sourceEye.${name}`, name === "origin" ? "vec3" : "vec4")]),
  )) as SourceModelEyeUniforms
  readonly #graphs = new Map<string, any>()

  get(key: string, create: () => any): any {
    let graph = this.#graphs.get(key)
    if (!graph) {
      graph = create()
      this.#graphs.set(key, graph)
    }
    return graph
  }

  get size(): number { return this.#graphs.size }

  releaseDrawReferences(): void {
    const references = [this.lighting.ambientEnabled, this.lighting.cameraPosition, ...this.lighting.ambient,
      ...this.lighting.local.flatMap(light => Object.values(light)), ...Object.values(this.eyes)]
    for (const node of references) (node as unknown as { reference: object | null }).reference = null
  }
}

export function bindModelLighting(mesh: THREE.Mesh, lighting: SourceModelLightingUniforms, eye?: SourceModelEyeUniforms): void {
  // Object3D.clone serializes enumerable userData; shader nodes are not scene
  // metadata and must never be copied/serialized into a different occurrence.
  Object.defineProperty(mesh.userData, "sourceLighting", { value: lighting, configurable: true })
  if (eye) Object.defineProperty(mesh.userData, "sourceEye", { value: eye, configurable: true })
}

/** Replacing the bind-pose mesh with its skinned mesh does not create another
 * occurrence. Preserve its non-enumerable bindings without making them cloneable. */
export function transferModelBindings(source: THREE.Mesh, target: THREE.Mesh): void {
  for (const name of ["sourceLighting", "sourceEye", "sourceEnvironment"]) {
    const descriptor = Object.getOwnPropertyDescriptor(source.userData, name)
    if (descriptor) Object.defineProperty(target.userData, name, descriptor)
  }
}

/** Texture identity is a draw binding, but texture interpretation is shader
 * structure. Keep the latter in the cache key, never the occurrence/cubemap id. */
export function modelEnvironmentShape(texture: THREE.CubeTexture | undefined): string {
  return texture ? `${texture.type}:${texture.format}:${texture.colorSpace}:${texture.mapping}:${Number(texture.isRenderTargetTexture)}` : "none"
}

export function bindModelEnvironment(mesh: THREE.Mesh, texture: THREE.CubeTexture): void {
  Object.defineProperty(mesh.userData, "sourceEnvironment", { value: texture, configurable: true })
}

export function perObjectModelEnvironment(color: any, node: any): any {
  // TextureNode.setup selects its own updateType. A separate object event is
  // required; attaching onObjectUpdate directly to the sampler is overwritten
  // during shader construction for cubemaps without a UV matrix.
  return TSL.Fn(() => {
    TSL.OnObjectUpdate(({ object }: { object: THREE.Mesh }) => { node.value = object.userData.sourceEnvironment })
    return color
  })()
}
