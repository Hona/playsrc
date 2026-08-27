import { describe, expect, test } from "bun:test"
import { RetainedModelCache } from "../src/retained-model-cache"

test("draining transfers each retained owner without disposing or retaining a second reference", () => {
  const disposed: object[] = [], cache = new RetainedModelCache<object>(2, value => disposed.push(value))
  const first = {}, second = {}
  cache.retain("first", first); cache.retain("second", second)
  expect(cache.drain()).toEqual([first, second])
  expect(cache.size).toBe(0)
  cache.clear()
  expect(disposed).toEqual([])
})

test("replacement retains only prepared instances with an identical transferred template owner", () => {
  const disposed: object[] = [], template = {}, changed = {}, replacement = {}
  const cache = new RetainedModelCache<{ template: object }>(96, value => disposed.push(value))
  const same = { template }, other = { template: changed }
  cache.retain("world:scout", same)
  cache.retain("panel:animated", other)
  cache.discardWhere(value => ![template, replacement].includes(value.template))
  expect(cache.take("world:scout")).toBe(same)
  expect(cache.take("panel:animated")).toBeUndefined()
  expect(disposed).toEqual([other])
  cache.clear()
  expect(disposed).toEqual([other])
})

test("a panel without pose input cannot acquire a parked posed occurrence", () => {
  const cache = new RetainedModelCache<object>(32, () => {})
  const posed = { skeleton: new Float32Array(16) }, bind = { texture: "authored" }
  cache.retain("hud:class:skin=0:true", posed)
  expect(cache.take("hud:class:skin=0:false")).toBeUndefined()
  cache.retain("hud:class:skin=0:false", bind)
  expect(cache.take("hud:class:skin=0:false")).toBe(bind)
  expect(cache.take("hud:class:skin=0:true")).toBe(posed)
})

test("HUD, class panel and carried model slots retain independent class/skin state within the bound", () => {
  const disposed: object[] = []
  const cache = new RetainedModelCache<object>(32, value => disposed.push(value))
  const models = new Map<string, object>()
  for (const slot of ["hud-player", "class-player", "class-weapon"]) {
    for (let playerClass = 1; playerClass <= 9; playerClass++) {
      const key = `${slot}:${playerClass}:skin=0`
      const model = { slot, playerClass, palette: new Float32Array(16), eyes: [playerClass] }
      models.set(key, model)
      cache.retain(key, model)
    }
  }
  for (const [key, model] of models) {
    expect(cache.take(key)).toBe(model)
    cache.retain(key, model)
  }
  expect(disposed).toHaveLength(0)
  expect(cache.take("hud-player:1:skin=1")).toBeUndefined()
  cache.clear()
  expect(new Set(disposed).size).toBe(27)
  expect(cache.size).toBe(0)
})

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
