import { expect, test } from "bun:test"
import { Tf2EquipmentProfile } from "../src/equipment/profile"
import { decodeEquipmentState } from "../src/equipment/codec"
import nativeEquipment from "./fixtures/equipment-state.json"

const key = "playsrc.tf2.local-equipment.v1"
const encode = (bytes: Uint8Array) => btoa(String.fromCharCode(...bytes))

test("restored Rust normalization rewrites only changed equipment persistence", async () => {
  const state = decodeEquipmentState(new Uint8Array(nativeEquipment))
  const previous = state.persistence.slice()
  // v0.0.12 saved empty Demoman primary/secondary slots (19 slots per class).
  previous.fill(255, 8 + 3 * 19 * 4, 8 + 3 * 19 * 4 + 8)
  const values = new Map([[key, encode(previous)], ["settings", "unchanged"], ["other", "retained"]])
  const writes: string[] = []
  const storage = { getItem: (name: string) => values.get(name) ?? null,
    setItem: (name: string, value: string) => { writes.push(name); values.set(name, value) } }
  const requests: Uint8Array[] = []
  const client = { equipment: async (generation: number, mutation?: ArrayBuffer) => {
    expect(generation).toBe(0)
    requests.push(new Uint8Array(mutation!))
    return state
  } }
  await new Tf2EquipmentProfile(client, storage).initialize()
  expect(requests[0]!.byteLength).toBe(693)
  expect(requests[0]!.slice(1)).toEqual(previous)
  expect(values.get(key)).toBe(encode(state.persistence))
  expect(writes).toEqual([key])
  expect(values.get("settings")).toBe("unchanged")
  expect(values.get("other")).toBe("retained")
  await new Tf2EquipmentProfile(client, storage).initialize()
  expect(requests[1]!.slice(1)).toEqual(state.persistence)
  expect(writes).toEqual([key])
})

test("failed restore preserves the original storage and transition error", async () => {
  const saved = encode(decodeEquipmentState(new Uint8Array(nativeEquipment)).persistence)
  const failure = new Error("TransitionFailed")
  const storage = { getItem: () => saved, setItem: () => { throw new Error("must not write") } }
  const profile = new Tf2EquipmentProfile({ equipment: async () => { throw failure } }, storage)
  await expect(profile.initialize()).rejects.toBe(failure)
  expect(profile.state()).toBeUndefined()
})
