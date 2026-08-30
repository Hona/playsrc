import { expect, test } from "bun:test"
import { Tf2EquipmentProfile } from "../src/equipment/profile"
import { nativeEquipment } from "./fixtures/equipment"
import type { Tf2EquipmentState } from "../src/equipment/types"

test("cancelled queued equipment never reaches the native mutation owner", async () => {
  const pending = Promise.withResolvers<Tf2EquipmentState>(), writes: string[] = []
  let calls = 0
  const profile = new Tf2EquipmentProfile({ equipment: async (_generation, mutation) => {
    if (!mutation) return nativeEquipment
    calls++
    return pending.promise
  } }, { getItem: () => null, setItem: (_key, value) => writes.push(value) })
  await profile.initialize()
  const first = profile.equip(3, 0, 18)
  const cancelled = new AbortController(), second = profile.equip(3, 0, 127, cancelled.signal)
  const rejected = second.catch(error => error)
  cancelled.abort()
  const state = { ...nativeEquipment, revision: nativeEquipment.revision + 1 }
  pending.resolve(state)
  expect(await first).toBe(state)
  expect(await rejected).toMatchObject({ name: "AbortError" })
  expect(calls).toBe(1)
  expect(writes).toHaveLength(1)
  expect(profile.state()).toBe(state)
})

test("a dispatched mutation remains authoritative after navigation cancellation", async () => {
  const pending = Promise.withResolvers<Tf2EquipmentState>(), signal = new AbortController()
  const profile = new Tf2EquipmentProfile({ equipment: async (_generation, mutation) => mutation ? pending.promise : nativeEquipment })
  await profile.initialize()
  const mutation = profile.equip(3, 0, 127, signal.signal)
  await Promise.resolve()
  signal.abort()
  const state = { ...nativeEquipment, revision: nativeEquipment.revision + 1 }
  pending.resolve(state)
  expect(await mutation).toBe(state)
  expect(profile.state()).toBe(state)
})

test("native failure preserves state and does not poison the next equipment transaction", async () => {
  let fail = true
  const error = new Error("native equip rejected")
  const profile = new Tf2EquipmentProfile({ equipment: async (_generation, mutation) => {
    if (mutation && fail) throw error
    return nativeEquipment
  } })
  await profile.initialize()
  await expect(profile.equip(3, 0, 127)).rejects.toBe(error)
  expect(profile.state()).toBe(nativeEquipment)
  fail = false
  expect(await profile.equip(3, 0, 18)).toBe(nativeEquipment)
})

test("closing the profile prevents a late native result from persisting into its replacement", async () => {
  const pending = Promise.withResolvers<Tf2EquipmentState>(), writes: string[] = []
  const profile = new Tf2EquipmentProfile({ equipment: async (_generation, mutation) => mutation ? pending.promise : nativeEquipment },
    { getItem: () => null, setItem: (_key, value) => writes.push(value) })
  await profile.initialize()
  const mutation = profile.equip(3, 0, 127)
  const rejected = mutation.catch(error => error)
  await Promise.resolve()
  profile.close(); pending.resolve(nativeEquipment)
  expect((await rejected).message).toContain("replaced")
  expect(profile.state()).toBeUndefined()
  expect(writes).toEqual([])
})
