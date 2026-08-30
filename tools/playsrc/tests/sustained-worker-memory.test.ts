import { expect, test } from "bun:test"
import { sustainedWorkerMemory } from "../profile/sustained-worker-memory"

test("memory boundaries query the gameplay owner, never blocking Rayon helpers", async () => {
  let helperQueries = 0
  const owner = { url: () => "http://localhost/gameplay-worker.ts", evaluate: async () => ({ memory: { linearBytes: 1024 } }) }
  const helper = { url: () => "http://localhost/wasm-bindgen-rayon/workerHelpers.js", evaluate: () => { helperQueries++; return new Promise(() => {}) } }
  const result = await sustainedWorkerMemory([helper, owner] as any)
  expect(result).toEqual({ owner: { memory: { linearBytes: 1024 } }, unqueriedWorkerUrls: [helper.url()] })
  expect(helperQueries).toBe(0)
})

test("unresponsive or ambiguous gameplay memory fails rather than consuming the entire headed budget", async () => {
  const owner = { url: () => "gameplay-worker.ts", evaluate: () => new Promise(() => {}) }
  await expect(sustainedWorkerMemory([owner] as any, 5)).rejects.toThrow("deadline")
  await expect(sustainedWorkerMemory([], 5)).rejects.toThrow("one gameplay")
  await expect(sustainedWorkerMemory([owner, owner] as any, 5)).rejects.toThrow("one gameplay")
})
