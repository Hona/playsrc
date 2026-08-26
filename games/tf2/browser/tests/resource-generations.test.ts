import { expect, test } from "bun:test"
import { resourceSectionIdentity, type ResourceChunkDescriptor } from "@playsrc/asset-store/graph"
import { ResourceGenerations, type ResourceSection } from "../src/resource-generations"

const HASH = "ab".repeat(32)
const section = (pointer: number): ResourceSection => ({ pointer, length: 32, authoredBacking: true })

test("generation leases preserve current/old-frame owners through staged cancellation and retirement", () => {
  const freed: number[] = []
  const resources = new ResourceGenerations((owner) => freed.push(owner.pointer))
  resources.adopt(1, section(100))
  resources.get(1)!.sha256 = HASH
  expect(resources.retain(2, 1, 0)).toBe(true)
  resources.adopt(2, section(200))
  resources.release(2)
  expect(freed).toEqual([200])
  expect(resources.get(1)!.sections[0]!.references).toBe(1)
  expect(resources.retain(3, 1, 0)).toBe(true)
  resources.get(3)!.sha256 = HASH
  resources.release(1)
  expect(freed).toEqual([200])
  expect(resources.retain(4, 1, 0)).toBe(false)
  expect(resources.retain(4, 3, 0)).toBe(true)
  resources.release(3)
  resources.release(4)
  expect(resources.release(4)).toBe(false)
  expect(freed).toEqual([200, 100])
})

test("only finalized exact source sections can be shared and finalized destinations cannot mutate", () => {
  const resources = new ResourceGenerations(() => {})
  resources.adopt(1, section(10))
  expect(resources.retain(2, 1, 0)).toBe(false)
  resources.get(1)!.sha256 = HASH
  expect(() => resources.adopt(1, section(20))).toThrow("not writable")
  for (const [target, source, index] of [[1, 1, 0], [0, 1, 0], [2, 1, 1], [2, 1, -1], [2, 1, 0.5]]) {
    expect(resources.retain(target!, source!, index!)).toBe(false)
  }
  expect(resources.get(1)!.sections[0]!.references).toBe(1)
})

test("deterministic randomized generations/cancel/shutdown release every unique allocation exactly once", () => {
  let seed = 0x5eed1234
  const random = () => { seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0; return seed }
  const freed = new Set<number>()
  const resources = new ResourceGenerations((owner) => {
    expect(freed.has(owner.pointer)).toBe(false)
    freed.add(owner.pointer)
  })
  const expected = new Map<number, number[]>()
  let allocated = 0
  for (let generation = 1; generation <= 500; generation += 1) {
    const pointers: number[] = []
    const sources = [...expected.keys()]
    for (let i = 0, count = random() % 6 + 1; i < count; i += 1) {
      if (sources.length > 0 && random() % 3 !== 0) {
        const source = sources[random() % sources.length]!
        const prior = expected.get(source)!
        const index = random() % prior.length
        expect(resources.retain(generation, source, index)).toBe(true)
        pointers.push(prior[index]!)
      } else {
        const pointer = ++allocated
        resources.adopt(generation, section(pointer))
        pointers.push(pointer)
      }
    }
    resources.get(generation)!.sha256 = HASH
    expected.set(generation, pointers)
    // Both cancelled candidates and old submitted-frame generations can retire out of order.
    for (const candidate of expected.keys()) if (random() % 3 === 0) {
      resources.release(candidate)
      expected.delete(candidate)
    }
    const references = new Map<number, number>()
    for (const values of expected.values()) for (const pointer of values) references.set(pointer, (references.get(pointer) ?? 0) + 1)
    for (const value of resources.values()) for (const owner of value.sections) {
      expect(freed.has(owner.pointer)).toBe(false)
      expect(owner.references).toBe(references.get(owner.pointer)!)
    }
    expect(freed.size + references.size).toBe(allocated)
  }
  for (const generation of resources.keys()) resources.release(generation)
  expect(freed.size).toBe(allocated)
})

test("shared section identity authenticates decoded content and the complete entry table, independently of roles", () => {
  const chunk: ResourceChunkDescriptor = {
    codec: "deflate", encodedByteLength: "20", encodedSha256: HASH,
    decodedByteLength: "32", decodedSha256: HASH, roles: ["gameplay"],
    entries: [{ logicalPath: "models/test.mdl", offset: "0", byteLength: "32", sha256: HASH }],
  }
  const identity = resourceSectionIdentity(chunk)
  expect(resourceSectionIdentity({ ...chunk, roles: ["gameplay", "startup"] })).toBe(identity)
  for (const changed of [
    { ...chunk, codec: "identity" as const },
    { ...chunk, decodedSha256: "cd".repeat(32) },
    { ...chunk, decodedByteLength: "33" },
    ...["logicalPath", "offset", "byteLength", "sha256"].map((key) => ({ ...chunk, entries: [{ ...chunk.entries[0]!, [key]: "different" }] })),
  ]) expect(resourceSectionIdentity(changed)).not.toBe(identity)
})
