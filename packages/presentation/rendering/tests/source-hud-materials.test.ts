import { describe, expect, test } from "bun:test"
import { SourceHudMaterials, type HudMaterial } from "../src/source-hud-materials"

describe("renderer-owned HUD material lifetime", () => {
  const texture = { width: 1, height: 1, clampS: true, clampT: true, noLod: true, encoding: "png", mips: ["data:image/png;base64,AA=="] } as const
  const input: readonly HudMaterial[] = [{ kind: "refract", normal: texture, tint: texture, amount: .1, blur: 1 }]

  test("decodes once, retains on reentry, and closes each bitmap exactly once", async () => {
    const previous = globalThis.createImageBitmap
    let decoded = 0, closed = 0
    globalThis.createImageBitmap = (async () => { decoded++; return { width: 1, height: 1, close: () => { closed++ } } }) as any
    const owner = new SourceHudMaterials()
    try {
      await owner.prepare(input)
      await owner.prepare(input)
      expect(owner.prepared).toBe(true)
      expect(decoded).toBe(2)
      expect(closed).toBe(0)
      owner.dispose()
      owner.dispose()
      expect(closed).toBe(2)
      await expect(owner.prepare(input)).rejects.toThrow("disposed")
    } finally { owner.dispose(); globalThis.createImageBitmap = previous }
  })

  test("closes a decode that finishes after its renderer was retired", async () => {
    const previous = globalThis.createImageBitmap
    let resolve!: (bitmap: any) => void, started!: () => void
    const decoding = new Promise<void>(yes => { started = yes })
    const result = new Promise<ImageBitmap>(yes => { resolve = yes })
    globalThis.createImageBitmap = (() => { started(); return result }) as any
    const owner = new SourceHudMaterials()
    let closed = 0
    try {
      const pending = owner.prepare(input)
      await decoding
      owner.dispose()
      resolve({ width: 1, height: 1, close: () => { closed++ } })
      await expect(pending).rejects.toThrow("disposed")
      expect(closed).toBe(1)
    } finally { owner.dispose(); globalThis.createImageBitmap = previous }
  })
})
