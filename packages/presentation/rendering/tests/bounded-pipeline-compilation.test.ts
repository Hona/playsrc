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

test("cancellation drains all admitted native work and preserves the first native failure", async () => {
  const nativeFailure = new Error("first native pipeline"), cancelled = new Error("generation replaced")
  const completions: { resolve(): void; reject(error: Error): void }[] = []
  const manager = { getForRender(_object: unknown, promises?: Promise<unknown>[] | null) {
    promises!.push(new Promise<void>((resolve, reject) => completions.push({ resolve, reject })))
  } }
  const descriptor = Object.getOwnPropertyDescriptor(manager, "getForRender")
  let settled = false
  const task = withBoundedPipelineCompilation(manager, async () => {
    manager.getForRender({}, []); manager.getForRender({}, []); manager.getForRender({}, [])
    throw cancelled
  }).catch(error => { settled = true; return error })
  await Promise.resolve()
  completions[1]!.reject(new Error("later native pipeline"))
  completions[0]!.reject(nativeFailure)
  for (let index = 0; index < 10; index++) await Promise.resolve()
  expect(settled).toBe(false)
  await expect(withBoundedPipelineCompilation(manager, async () => {})).rejects.toThrow("owner")
  completions[2]!.resolve()
  expect(await task).toBe(nativeFailure)
  expect(Object.getOwnPropertyDescriptor(manager, "getForRender")).toEqual(descriptor)
  await expect(withBoundedPipelineCompilation(manager, async () => { throw cancelled })).rejects.toBe(cancelled)
})

test("ordinary rendering never acquires async preparation work", async () => {
  const calls: unknown[] = []
  const manager = { getForRender(object: object, promises?: Promise<unknown>[] | null) { calls.push(promises); return object } }
  const object = {}
  await withBoundedPipelineCompilation(manager, async () => {
    expect(manager.getForRender(object)).toBe(object)
    expect(manager.getForRender(object, null)).toBe(object)
  })
  expect(calls).toEqual([undefined, null])
})

test("world preparation admits two native jobs without a third descriptor allocation", async () => {
  let live=0,maximum=0
  const manager={getForRender(_object:unknown,promises?:Promise<unknown>[]|null){maximum=Math.max(maximum,++live);promises!.push(new Promise<void>(resolve=>setTimeout(()=>{live--;resolve()},0)))}}
  await withBoundedPipelineCompilation(manager,async()=>{
    for(let index=0;index<8;index++){const promises:Promise<unknown>[]=[];manager.getForRender({},promises);await Promise.all(promises)}
  },2)
  expect(maximum).toBe(2);expect(live).toBe(0)
  await expect(withBoundedPipelineCompilation(manager,async()=>{},0)).rejects.toThrow("capacity")
})
