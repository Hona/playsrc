import { describe, expect, test } from "bun:test"
import { createApplicationGenerationRecovery } from "../src/application-generation"

describe("authenticated application generation recovery", () => {
  test("rejects a stale application before resource publication and performs one shared upgrade", async () => {
    let reloads = 0
    const records = new Map<string, string>()
    const recovery = createApplicationGenerationRecovery({
      currentBuild: "a".repeat(64),
      storage: { getItem: (key) => records.get(key) ?? null, setItem: (key, value) => { records.set(key, value) } },
      visible: () => true,
      whenVisible: async () => {},
      reload: () => { reloads += 1 },
    })

    await Promise.all([recovery.ensure("b".repeat(64)), recovery.ensure("b".repeat(64))])
    expect(reloads).toBe(1)
    await expect(recovery.ensure("b".repeat(64))).rejects.toThrow("Application generation upgrade did not converge")
    expect(reloads).toBe(1)
  })

  test("waits for visible restoration without charging a hidden tab or touching persistent object caches", async () => {
    let visible = false
    let release!: () => void
    let reloads = 0
    const ready = new Promise<void>((resolve) => { release = resolve })
    const recovery = createApplicationGenerationRecovery({
      currentBuild: "a".repeat(64),
      storage: { getItem: () => null, setItem: () => {} },
      visible: () => visible,
      whenVisible: () => ready,
      reload: () => { reloads += 1 },
    })

    const pending = recovery.ensure("b".repeat(64))
    await Promise.resolve()
    expect(reloads).toBe(0)
    visible = true
    release()
    await pending
    expect(reloads).toBe(1)
    await expect(recovery.ensure("foreign")).rejects.toThrow("Application generation identity is invalid")
  })

  test("recovers a proven stale Worker once even when its authenticated configuration matches", async () => {
    let record: string | null = null
    let reloads = 0
    const recovery = createApplicationGenerationRecovery({
      currentBuild: "a".repeat(64), storage: { getItem: () => record, setItem: (_, value) => { record = value } },
      visible: () => true, whenVisible: async () => {}, reload: () => { reloads += 1 },
    })
    expect(await recovery.ensure("a".repeat(64))).toBe(true)
    expect(await recovery.ensure("a".repeat(64), true)).toBe(false)
    await expect(recovery.ensure("a".repeat(64), true)).rejects.toThrow("did not converge")
    expect(reloads).toBe(1)
  })

  test("rejects corrupt foreign session metadata and bounds distinct racing upgrades", async () => {
    let record: string | null = "{foreign"
    let reloads = 0
    const recovery = createApplicationGenerationRecovery({
      currentBuild: "a".repeat(64), storage: { getItem: () => record, setItem: (_, value) => { record = value } },
      visible: () => true, whenVisible: async () => {}, reload: () => { reloads += 1 },
    })
    await expect(recovery.ensure("b".repeat(64))).rejects.toThrow("recovery state is invalid")
    record = null
    for (const build of ["b", "c", "d"]) await recovery.ensure(build.repeat(64))
    await expect(recovery.ensure("e".repeat(64))).rejects.toThrow("did not converge")
    expect(reloads).toBe(3)
  })

  test("allows later authenticated releases without sharing another tab's bounded recovery", async () => {
    let record = JSON.stringify([`${"b".repeat(64)}:${"c".repeat(64)}`, `${"b".repeat(64)}:${"d".repeat(64)}`, `${"b".repeat(64)}:${"e".repeat(64)}`])
    let reloads = 0
    const recovery = createApplicationGenerationRecovery({
      currentBuild: "a".repeat(64), storage: { getItem: () => record, setItem: (_, value) => { record = value } },
      visible: () => true, whenVisible: async () => {}, reload: () => { reloads += 1 },
    })
    await recovery.ensure("f".repeat(64))
    expect(JSON.parse(record)).toEqual([`${"a".repeat(64)}:${"f".repeat(64)}`])
    expect(reloads).toBe(1)
  })
})
