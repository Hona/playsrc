import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import type { SourceModelEyeUniforms, SourceModelLightingUniforms } from "./source-model-lighting"

const reference = (path: string, type: string) => TSL.reference(`userData.${path}.value`, type) as ReturnType<typeof TSL.uniform>

const LIGHT_FIELDS = ["enabled", "kind", "color", "position", "direction", "attenuation", "falloff", "theta", "phi"] as const

class DrawLightingEvent extends THREE.EventNode {
  override build(builder: THREE.NodeBuilder): any {
    // Node.before forwards its consumer's output type. An update-only event
    // has no shader value, irrespective of that consumer's scalar/vector type.
    return super.build(builder, "void")
  }
}

/** All lighting members belong to the same drawn occurrence. Resolve that
 * owner once per draw, not once per scalar/vector. This is NOT a value cache:
 * every draw reads the current binding, including consecutive draws of the
 * same object within a pass. Three still compares/uploads each used uniform.
 * Each member declares the event dependency, so even a partial graph updates. */
function drawLighting(): SourceModelLightingUniforms {
  const event = new DrawLightingEvent("object", ({ object }: { object: THREE.Mesh }) => {
    const source = object.userData.sourceLighting as SourceModelLightingUniforms
    lighting.ambientEnabled.value = source.ambientEnabled.value
    lighting.cameraPosition.value = source.cameraPosition.value
    for (let side = 0; side < 6; side++) lighting.ambient[side]!.value = source.ambient[side]!.value
    for (let index = 0; index < 4; index++) {
      const target = lighting.local[index]!, light = source.local[index]!
      for (const name of LIGHT_FIELDS) target[name].value = light[name].value
    }
  })
  const member = (type: string) => TSL.uniform(null, type).before(event)
  const lighting: SourceModelLightingUniforms = Object.freeze({
    ambientEnabled: member("float"), cameraPosition: member("vec3"),
    ambient: Object.freeze(Array.from({ length: 6 }, () => member("vec3"))) as SourceModelLightingUniforms["ambient"],
    local: Object.freeze(Array.from({ length: 4 }, () => Object.freeze(Object.fromEntries(LIGHT_FIELDS.map(name =>
      [name, member(["color", "position", "direction", "attenuation"].includes(name) ? "vec3" : "float")],
    ))))) as unknown as SourceModelLightingUniforms["local"],
  })
  return lighting
}

/** One immutable graph family per scene/exposure/fog domain. Values are read
 * from the drawn primitive, not captured from the first actor using a shader.
 * The owner is scene-local, with an explicit handoff for an identical verified
 * world/resource closure on the same device. It is never a module-global cache;
 * every new draw binds the replacement's primitive values. */
export class ModelLightingGraphs {
  // Static VHV/unlit primitives share their material graph. Distance/screen
  // fade remains an occurrence binding, just like dynamic model lighting.
  readonly staticFade = reference("sourceStaticFade", "float")
  readonly lighting = drawLighting()
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
    const references = [this.staticFade, ...Object.values(this.eyes)]
    for (const node of references) (node as unknown as { reference: object | null }).reference = null
    // Vector identities are draw values too. Do not retain a retired occurrence's
    // values through a verified graph handoff; the next draw binds them afresh.
    for (const node of [this.lighting.ambientEnabled, this.lighting.cameraPosition, ...this.lighting.ambient,
      ...this.lighting.local.flatMap(light => Object.values(light))]) node.value = null
  }
}

export function bindStaticPropFade(mesh: THREE.Mesh, fade: ReturnType<typeof TSL.uniform>): void {
  Object.defineProperty(mesh.userData, "sourceStaticFade", { value: fade, configurable: true })
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
