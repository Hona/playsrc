import { expect, test } from "bun:test"
import { withBoundedPipelineCompilation } from "../src/bounded-pipeline-compilation"

test("builds objects in order while bounding native work and awaiting final readiness", async () => {
  const started: number[] = [], complete: (() => void)[] = []
  let live = 0, maximum = 0, published = false
  const manager = { getForRender(object: number, promises?: Promise<unknown>[] | null) {
    started.push(object); maximum = Math.max(maximum, ++live)
    promises!.push(new Promise<void>(resolve => complete.push(() => { live--; resolve() })))
    return object
  } }
  const original = manager.getForRender
  const task = withBoundedPipelineCompilation(manager, async () => {
    for (let object = 0; object < 8; object++) {
      const promises: Promise<unknown>[] = []
      expect(manager.getForRender(object, promises)).toBe(object)
      if (promises.length) { complete.shift()!(); await Promise.all(promises) }
    }
  }).then(() => { published = true })
  for (let i = 0; i < 40; i++) await Promise.resolve()
  expect(started).toEqual([0, 1, 2, 3, 4, 5, 6, 7])
  expect(maximum).toBe(4)
  expect(published).toBe(false)
  for (const finish of complete) finish()
  await task
  expect(live).toBe(0)
  expect(published).toBe(true)
  expect(manager.getForRender).toBe(original)
})

test("failed native compilation cannot publish readiness and restores the exact method", async () => {
  const failure = new Error("native compile failed")
  const manager = { getForRender(_object: unknown, promises?: Promise<unknown>[] | null) { promises!.push(Promise.reject(failure)) } }
  const original = manager.getForRender
  await expect(withBoundedPipelineCompilation(manager, async () => {
    manager.getForRender({}, [])
    await expect(withBoundedPipelineCompilation(manager, async () => {})).rejects.toThrow("owner")
  })).rejects.toBe(failure)
  expect(manager.getForRender).toBe(original)
  await withBoundedPipelineCompilation(manager, async () => {})
})
