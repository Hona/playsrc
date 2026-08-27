type Pipelines = { getForRender(object: any, promises?: Promise<unknown>[] | null): any }
const owners = new WeakSet<object>()

/** Keep Three's node construction and resource updates sequential, but don't
 * serialize independent native pipeline compilation behind every primitive.
 * Four native jobs at most; every job settles before preparation can publish. */
export async function withBoundedPipelineCompilation(pipelines: Pipelines, compile: () => Promise<void>): Promise<void> {
  if (owners.has(pipelines)) throw new Error("Pipeline preparation already has an owner")
  owners.add(pipelines)
  const original = pipelines.getForRender
  const descriptor = Object.getOwnPropertyDescriptor(pipelines, "getForRender")
  const inFlight = new Set<Promise<void>>()
  const results: Promise<{ error?: unknown }>[] = []
  pipelines.getForRender = function (object, promises) {
    if (!promises) return original.call(this, object, promises)
    const native: Promise<unknown>[] = []
    const result = original.call(this, object, native)
    for (const promise of native) {
      const settled = promise.then(() => ({}), error => ({ error }))
      results.push(settled)
      const tracked = settled.then(() => { inFlight.delete(tracked) })
      inFlight.add(tracked)
    }
    if (inFlight.size >= 4) promises.push(Promise.race(inFlight))
    return result
  }
  let buildFailure: { error: unknown } | undefined
  try { await compile() } catch (error) { buildFailure = { error } }
  finally {
    if (descriptor) Object.defineProperty(pipelines, "getForRender", descriptor)
    else delete (pipelines as Partial<Pipelines>).getForRender
  }
  try {
    const completed = await Promise.all(results)
    const failed = completed.find(result => "error" in result)
    if (failed) throw failed.error
    if (buildFailure) throw buildFailure.error
  } finally { owners.delete(pipelines) }
}
