import Pipelines from "three/src/renderers/common/Pipelines.js"
import { withBoundedPipelineCompilation } from "../../src/bounded-pipeline-compilation"

/** Device-free ownership control using Three's real pipeline/program caches and
 * already compiled render objects. Deferred native completions are test inputs,
 * not a GPU benchmark. Shader and backend feature keys stay authoritative. */
export async function verifyPipelinePreparation(objects: readonly any[], backend: any) {
  const run = async (bounded: boolean) => {
    const descriptors: unknown[] = [], pending = new Set<() => void>()
    let maximum = 0, ready = false
    const pipelines = new Pipelines({
      getRenderCacheKey: (object: any) => backend.getRenderCacheKey(object),
      needsRenderUpdate: () => false,
      createProgram() {},
      createRenderPipeline(object: any, promises: Promise<void>[]) {
        const pipeline = object.pipeline
        descriptors.push({ vertex: pipeline.vertexProgram.code, fragment: pipeline.fragmentProgram.code,
          features: backend.getRenderCacheKey(object) })
        promises.push(new Promise<void>(resolve => {
          const complete = () => { pending.delete(complete); resolve() }
          pending.add(complete)
          maximum = Math.max(maximum, pending.size)
          // The work loop reaches its bounded wait before the next timer turn.
          setTimeout(complete, 0)
        }))
      },
    }, {}, { createProgram() {}, destroyProgram() {} })
    const original = pipelines.getForRender
    const compile = async () => {
      for (const object of objects) {
        const promises: Promise<void>[] = []
        pipelines.getForRender(object, promises)
        if (promises.length) await Promise.all(promises)
      }
    }
    try {
      if (bounded) await withBoundedPipelineCompilation(pipelines, compile)
      else await compile()
      if (pending.size) throw new Error("Preparation published before native readiness")
      ready = true
      const cold = descriptors.length
      await withBoundedPipelineCompilation(pipelines, compile)
      if (cold !== descriptors.length) throw new Error("Warm preparation rebuilt a pipeline")
      if (pipelines.getForRender !== original) throw new Error("Preparation retained its manager override")
      return { descriptors, maximum, ready, cold, warm: descriptors.length - cold }
    } finally {
      for (const complete of pending) complete()
      for (const object of objects) pipelines.delete(object)
      if (pipelines.caches.size || pipelines.programs.vertex.size || pipelines.programs.fragment.size) throw new Error("Preparation retained retired pipelines/programs")
    }
  }
  const serial = await run(false), bounded = await run(true)
  if (JSON.stringify(serial.descriptors) !== JSON.stringify(bounded.descriptors)) throw new Error("Preparation changed shader/feature keys or native submission order")
  if (!serial.cold || serial.maximum !== 1 || bounded.maximum !== Math.min(4, bounded.cold)) throw new Error("Preparation did not remove the serial native ownership barrier")
  return { pipelines: serial.cold, serialMaximum: serial.maximum, boundedMaximum: bounded.maximum,
    warmBuilds: bounded.warm, ready: bounded.ready, descriptorsEqual: true }
}
