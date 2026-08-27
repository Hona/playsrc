import { describe, expect, test } from "bun:test"
import { MAX_CHUNK_ROLES, MAX_GRAPH_CHUNKS, chunksForRole, encodeResourceBatch, parseResourceCatalog, parseResourceGraph, parseResourceGraphBytes, parseResourceSet, partitionResourceChunkDescriptors, partitionResourceChunks, selectCatalogTarget } from "../src/graph"

const hash = (value: string) => value.repeat(64)
const chunk = Object.freeze({
  codec: "identity" as const,
  encodedByteLength: "64",
  encodedSha256: hash("1"),
  decodedByteLength: "64",
  decodedSha256: hash("2"),
  roles: Object.freeze(["menu"]),
  entries: Object.freeze([{ logicalPath: "materials/a.vmt", offset: "60", byteLength: "4", sha256: hash("3") }]),
})
const graph = Object.freeze({
  schema: "playsrc-resource-graph-v1" as const,
  game: "tf2" as const,
  contentBuild: "24245096",
  target: "jump_beef",
  chunks: Object.freeze([chunk]),
})

describe("resource graph", () => {
  test("admits expanded map and item regions without changing chunk or entry byte bounds", () => {
    const chunks = Array.from({ length: 1_025 }, (_, index) => ({ ...chunk,
      encodedSha256: index.toString(16).padStart(64, "0"),
      entries: [{ ...chunk.entries[0]!, logicalPath: `materials/region${index}/a.vmt` }],
    }))
    const parsed = parseResourceGraph({ ...graph, chunks })
    expect(parsed.chunks).toHaveLength(1_025)
    const batch = encodeResourceBatch(parsed.chunks.map(descriptor => ({ descriptor, bytes: new Uint8Array(64) })))
    expect(new DataView(batch.buffer).getUint32(8, true)).toBe(1_025)
    expect(() => parseResourceGraph({ ...graph, chunks: Array(MAX_GRAPH_CHUNKS + 1).fill(chunk) })).toThrow()
  })
  test("admits shared per-item closure roles with an explicit producer-consumer bound", () => {
    const roles = Array.from({ length: MAX_CHUNK_ROLES }, (_, index) => `equipment-${String(index).padStart(4, "0")}`)
    const parsed = parseResourceGraph({ ...graph, chunks: [{ ...chunk, roles }] })
    expect(chunksForRole(parsed, "equipment-0031")).toHaveLength(1)
    expect(parsed.chunks[0]!.roles).toHaveLength(MAX_CHUNK_ROLES)
    expect(() => parseResourceGraph({ ...graph, chunks: [{ ...chunk, roles: [...roles, "gameplay"] }] })).toThrow("resource chunk descriptor is malformed")
  })
  test("parses exact chunks and builds one bounded Rust batch", () => {
    const parsed = parseResourceGraph(graph)
    expect(chunksForRole(parsed, "menu")).toEqual([chunk])
    const batch = encodeResourceBatch([{ descriptor: chunk, bytes: new Uint8Array(64) }])
    expect(new TextDecoder().decode(batch.subarray(0, 4))).toBe("PSGB")
    expect(new DataView(batch.buffer).getUint32(8, true)).toBe(1)
  })

  test("partitions source-backed chunks by both encoded and decoded bounds without duplicate transfers", () => {
    const records = Array.from({ length: 3 }, (_, index) => ({
      descriptor: {
        ...chunk,
        encodedSha256: hash(String(index + 1)),
        entries: [{ ...chunk.entries[0]!, logicalPath: `materials/${index}.vmt`, byteLength: "260" }],
      },
      bytes: new Uint8Array(64),
    }))
    const maximum = 650
    const sections = partitionResourceChunks([records[0]!, records[0]!, records[1]!, records[2]!], maximum)
    expect(sections.map((section) => section.map((entry) => entry.descriptor.encodedSha256))).toEqual([
      [hash("1")],
      [hash("2")],
      [hash("3")],
    ])
    for (const section of sections) expect(encodeResourceBatch(section).byteLength).toBeLessThanOrEqual(maximum)
    expect(() => partitionResourceChunks(records, 300)).toThrow("resource chunk exceeds section byte bound")
    expect(() => partitionResourceChunks([{ ...records[0]!, bytes: new Uint8Array(63) }])).toThrow("encoded byte length differs")
  })

  test("rejects duplicate logical paths and noncanonical chunk order", () => {
    const second = { ...chunk, encodedSha256: hash("0") }
    expect(() => parseResourceGraph({ ...graph, chunks: [chunk, second] })).toThrow()
    expect(() => parseResourceGraph({ ...graph, chunks: [chunk, { ...chunk, encodedSha256: hash("4") }] })).toThrow()
    expect(() => parseResourceGraphBytes(new TextEncoder().encode(JSON.stringify(graph, null, 2)))).toThrow("resource graph is not canonical JSON")
  })

  test("keeps gameplay source texture and model bytes on their shared decoded section", () => {
    const path = new TextEncoder().encode("materials/a.vtf")
    const section = new Uint8Array(new SharedArrayBuffer(20 + path.byteLength + 3))
    const view = new DataView(section.buffer)
    section.set([0x50, 0x53, 0x52, 0x45])
    view.setUint32(4, 1, true)
    view.setUint32(8, 1, true)
    view.setUint32(12, path.byteLength, true)
    section.set(path, 16)
    view.setUint32(16 + path.byteLength, 3, true)
    section.set([4, 5, 6], 20 + path.byteLength)
    const texture = parseResourceSet(section).get("materials/a.vtf")!
    expect(texture.buffer).toBe(section.buffer)
    expect([...texture]).toEqual([4, 5, 6])
  })

  test("partitions exact graph chunks into bounded transferable decoded sections", () => {
    const descriptors = Array.from({ length: 3 }, (_, index) => ({
      ...chunk,
      encodedSha256: hash(String(index + 1)),
      entries: [{ ...chunk.entries[0]!, logicalPath: `materials/${index}.vmt`, byteLength: "260" }],
    }))
    expect(partitionResourceChunkDescriptors([descriptors[0]!, descriptors[0]!, descriptors[1]!, descriptors[2]!], 650))
      .toEqual([[descriptors[0]], [descriptors[1]], [descriptors[2]]])
    expect(() => partitionResourceChunkDescriptors([descriptors[0]!], 300)).toThrow("resource chunk exceeds")
    expect(() => partitionResourceChunkDescriptors([], 32)).toThrow("resource batch chunk bound")
  })

  test("bounds a synthetic 100-map catalog selection without loading every map", () => {
    const catalog = parseResourceCatalog({
      schema: "playsrc-resource-catalog-v1",
      application: "tf2",
      entries: Array.from({ length: 100 }, (_, index) => ({
        target: `map_${index.toString().padStart(3, "0")}`,
        resources: { kind: "source-root", mediaType: "application/vnd.playsrc.resource-graph+json", byteLength: "1024", sha256: hash((index % 10).toString(16)) },
      })),
    })
    expect(selectCatalogTarget(catalog, "map_042").resources.sha256).toBe(hash("2"))
    expect(new Set(catalog.entries.map((entry) => entry.resources.sha256)).size).toBe(10)
  })
})
