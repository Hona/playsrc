import { describe, expect, test } from "bun:test"
import { RetainedModelCache } from "../src/retained-model-cache"

describe("generation-owned retained stock viewmodels", () => {
  test("retains every class and reacquires its exact weapon without disposal", () => {
    const disposed: string[] = []
    const cache = new RetainedModelCache<{ identity: string }>(32, value => disposed.push(value.identity))
    const identities = ["scout", "soldier", "pyro", "demoman", "heavy", "engineer", "medic", "sniper", "spy"]
    const models = identities.map(identity => ({ identity }))
    for (const model of models) cache.retain(`1:${model.identity}`, model)
    for (const model of models) expect(cache.take(`1:${model.identity}`)).toBe(model)
    expect(cache.size).toBe(0)
    expect(disposed).toEqual([])
  })

  test("bounds stock weapon residency and disposes only the least recently parked instance", () => {
    const disposed: string[] = []
    const cache = new RetainedModelCache<string>(2, value => disposed.push(value))
    cache.retain("primary", "minigun")
    cache.retain("secondary", "shotgun")
    expect(cache.take("primary")).toBe("minigun")
    cache.retain("primary", "minigun")
    cache.retain("melee", "fists")
    expect(disposed).toEqual(["shotgun"])
    expect(cache.take("secondary")).toBeUndefined()
    cache.clear()
    cache.clear()
    expect(disposed).toEqual(["shotgun", "minigun", "fists"])
  })

  test("atomically separates map/device generations and disposes replaced identities once", () => {
    const disposed: string[] = []
    const cache = new RetainedModelCache<string>(4, value => disposed.push(value))
    cache.retain("7:rocket", "old")
    cache.retain("7:rocket", "new")
    expect(disposed).toEqual(["old"])
    cache.clear()
    expect(cache.take("7:rocket")).toBeUndefined()
    cache.retain("8:rocket", "next-generation")
    expect(cache.take("8:rocket")).toBe("next-generation")
    expect(disposed).toEqual(["old", "new"])
    expect(() => new RetainedModelCache(0, () => {})).toThrow("capacity is invalid")
  })
})

test("world visibility and view weapon changes share bounded residency without instance aliasing", () => {
  const disposed: object[] = []
  const cache = new RetainedModelCache<object>(2, value => disposed.push(value))
  const actor = { lighting: {}, skeleton: {}, meshes: [] }, view = { root: {} }, other = {}
  cache.retain("world:7:player:skin0:posed", actor)
  cache.retain("view:7:player:skin0:posed", view)
  expect(cache.take("world:7:player:skin1:posed")).toBeUndefined()
  expect(cache.take("world:8:player:skin0:posed")).toBeUndefined()
  expect(cache.take("world:7:player:skin0:unposed")).toBeUndefined()
  const restored = cache.take("world:7:player:skin0:posed")
  expect(restored).toBe(actor)
  cache.retain("world:7:player:skin0:posed", restored!)
  cache.retain("world:8:player:skin0:posed", other)
  expect(cache.size).toBe(2)
  expect(disposed).toEqual([view])
  cache.clear(); cache.clear()
  expect(disposed).toEqual([view, actor, other])
  expect(cache.take("world:7:player:skin0:posed")).toBeUndefined()
})
