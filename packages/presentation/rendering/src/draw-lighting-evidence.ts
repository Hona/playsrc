import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import { ModelLightingGraphs } from "./model-lighting-graphs"
import { installSkinningEvidence } from "./skinning-evidence"

// Explicit local acceptance import only. No production import, value cache or
// scene retention. The independent oracle runs the previous ReferenceNode path
// against the exact same objects before each differential GPU draw.
export function installDrawLightingEvidence() {
  const graphs = new ModelLightingGraphs()
  const eventPrototype = Object.getPrototypeOf((graphs.lighting.ambientEnabled as any)._beforeNodes[0])
  const descriptor = Object.getOwnPropertyDescriptor(eventPrototype, "update")!, update = descriptor.value
  const pairs = new WeakMap<object, { target: any; reference: any }[]>()
  let reference = false, active = false, draws = 0, values = 0
  const register = (event: any) => {
    const lighting = event.lighting as ModelLightingGraphs["lighting"]
    if (pairs.has(event)) return
    const entries: { target: any; reference: any }[] = []
    const add = (path: string, target: any) => entries.push({ target, reference: TSL.reference(`userData.sourceLighting.${path}.value`, target.nodeType) })
    add("ambientEnabled", lighting.ambientEnabled); add("cameraPosition", lighting.cameraPosition)
    lighting.ambient.forEach((node, i) => add(`ambient.${i}`, node))
    lighting.local.forEach((light, i) => Object.entries(light).forEach(([name, node]) => add(`local.${i}.${name}`, node)))
    pairs.set(event, entries)
  }
  eventPrototype.update = function (frame: any) {
    if (!active) return update.call(this, frame)
    register(this)
    const entries = pairs.get(this)
    if (!entries) throw new Error("Unregistered lighting draw owner")
    if (!reference) update.call(this, frame)
    for (const entry of entries) {
      entry.reference.updateReference(frame); entry.reference.updateValue()
      if (reference) entry.target.value = entry.reference.node.value
      else if (!Object.is(entry.target.value, entry.reference.node.value)) throw new Error("Lighting draw value differs")
      entry.reference.reference = null; entry.reference.node.value = null
      values++
    }
    draws++
  }
  const pixels = installSkinningEvidence(draw => {
    reference = true
    try { draw() } finally { reference = false }
  })
  return {
    async capture(label: string, pass: string) {
      active = true; draws = 0; values = 0
      try { return { ...await pixels.capture(label, pass) as object, lightingDraws: draws, lightingValues: values } }
      finally { active = false }
    },
    dispose() {
      pixels.dispose()
      Object.defineProperty(eventPrototype, "update", descriptor)
    },
  }
}
