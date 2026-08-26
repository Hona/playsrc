import { describe, expect, test } from "bun:test"
import { createDevelopmentBuildCoherence } from "../src/development-coherence"

describe("development application and WASM build coherence", () => {
  test("replaces a stale producer before exposing its upgraded browser configuration", async () => {
    let current = "old"
    let published = "old"
    const events: string[] = []
    const coherence = createDevelopmentBuildCoherence("old", async () => current, async (identity) => {
      events.push(`build:${identity}`)
      await Promise.resolve()
      published = identity
    })

    await coherence.ensure()
    expect(events).toEqual([])
    current = "new"
    await Promise.all([coherence.ensure(), coherence.ensure(), coherence.ensure()])
    expect(events).toEqual(["build:new"])
    expect(published).toBe("new")
  })

  test("never publishes a failed replacement and retries against the exact current source", async () => {
    let attempts = 0
    const coherence = createDevelopmentBuildCoherence("old", async () => "new", async () => {
      attempts += 1
      if (attempts === 1) throw new Error("WASM producer failed")
    })

    await expect(coherence.ensure()).rejects.toThrow("WASM producer failed")
    await coherence.ensure()
    expect(attempts).toBe(2)
  })

  test("finishes the newest generation when the checkout advances during replacement", async () => {
    let current = "second"
    const published: string[] = []
    const coherence = createDevelopmentBuildCoherence("first", async () => current, async (identity) => {
      published.push(identity)
      if (identity === "second") current = "third"
    })

    await coherence.ensure()
    expect(published).toEqual(["second", "third"])
  })
})
