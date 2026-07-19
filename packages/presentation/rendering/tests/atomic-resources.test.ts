import { expect, test } from "bun:test"
import { AtomicResourceSlot } from "../src/atomic-resources"

test("publishes resource replacement atomically and disposes failed/prior generations once", async () => {
  const slot = new AtomicResourceSlot<{ identity: string; dispose(): void }>()
  const disposals = new Map<string, number>()
  const resource = (identity: string) => ({ identity, dispose: () => disposals.set(identity, (disposals.get(identity) ?? 0) + 1) })
  await slot.replace(() => resource("one"), () => {})
  expect(slot.generation).toBe(1)
  await expect(slot.replace(() => resource("failed"), () => { throw new Error("sentinel") })).rejects.toThrow(/staging/i)
  expect(slot.current()?.identity).toBe("one")
  expect(slot.generation).toBe(1)
  expect(disposals.get("failed")).toBe(1)
  await slot.replace(() => resource("two"), () => {})
  expect(disposals.get("one")).toBe(1)
  expect(slot.generation).toBe(2)
  await slot.dispose()
  await slot.dispose()
  expect(disposals.get("two")).toBe(1)
})
