import { describe, expect, test } from "bun:test"
import { chunksForRole, encodeResourceBatch, parseResourceCatalog, parseResourceGraph, parseResourceGraphBytes, selectCatalogTarget } from "../src/graph"

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
  test("parses exact chunks and builds one bounded Rust batch", () => {
    const parsed = parseResourceGraph(graph)
    expect(chunksForRole(parsed, "menu")).toEqual([chunk])
    const batch = encodeResourceBatch([{ descriptor: chunk, bytes: new Uint8Array(64) }])
    expect(new TextDecoder().decode(batch.subarray(0, 4))).toBe("PSGB")
    expect(new DataView(batch.buffer).getUint32(8, true)).toBe(1)
  })

  test("rejects duplicate logical paths and noncanonical chunk order", () => {
    const second = { ...chunk, encodedSha256: hash("0") }
    expect(() => parseResourceGraph({ ...graph, chunks: [chunk, second] })).toThrow()
    expect(() => parseResourceGraph({ ...graph, chunks: [chunk, { ...chunk, encodedSha256: hash("4") }] })).toThrow()
    expect(() => parseResourceGraphBytes(new TextEncoder().encode(JSON.stringify(graph, null, 2)))).toThrow("resource graph is not canonical JSON")
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
