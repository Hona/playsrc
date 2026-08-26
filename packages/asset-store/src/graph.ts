import type { ObjectDescriptor } from "./index"

const HASH = /^[0-9a-f]{64}$/
const DECIMAL = /^(0|[1-9]\d*)$/
const ROLE = /^[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?$/
const MAX_CHUNK_BYTES = 32 * 1024 * 1024
const MAX_CHUNK_ENTRIES = 2_048
const MAX_GRAPH_ENTRIES = 8_192
const MAX_GRAPH_CHUNKS = 1_024

export const RESOURCE_GRAPH_MEDIA_TYPE = "application/vnd.playsrc.resource-graph+json"
export const RESOURCE_CHUNK_MEDIA_TYPE = "application/vnd.playsrc.resource-chunk"

export type ResourceEntryDescriptor = Readonly<{
  logicalPath: string
  offset: string
  byteLength: string
  sha256: string
}>

export type ResourceChunkDescriptor = Readonly<{
  codec: "identity" | "deflate"
  encodedByteLength: string
  encodedSha256: string
  decodedByteLength: string
  decodedSha256: string
  roles: readonly string[]
  entries: readonly ResourceEntryDescriptor[]
}>

export type ResourceGraph = Readonly<{
  schema: "playsrc-resource-graph-v1"
  game: "tf2"
  contentBuild: string
  target: string
  chunks: readonly ResourceChunkDescriptor[]
}>

export type ResourceCatalogEntry = Readonly<{ target: string; resources: ObjectDescriptor }>
export type ResourceCatalog = Readonly<{
  schema: "playsrc-resource-catalog-v1"
  application: string
  entries: readonly ResourceCatalogEntry[]
}>

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function integer(value: unknown, maximum: number): number | undefined {
  if (typeof value !== "string" || !DECIMAL.test(value)) return undefined
  const parsed = Number(value)
  return Number.isSafeInteger(parsed) && parsed >= 0 && parsed <= maximum ? parsed : undefined
}

export function parseResourceGraph(value: unknown): ResourceGraph {
  if (
    !record(value)
    || Object.keys(value).sort().join("\0") !== "chunks\0contentBuild\0game\0schema\0target"
    || value.schema !== "playsrc-resource-graph-v1"
    || value.game !== "tf2"
    || typeof value.contentBuild !== "string"
    || !/^[0-9]{1,32}$/.test(value.contentBuild)
    || typeof value.target !== "string"
    || !/^[a-z0-9_]{1,64}$/.test(value.target)
    || !Array.isArray(value.chunks)
    || value.chunks.length < 1
    || value.chunks.length > MAX_GRAPH_CHUNKS
  ) throw new Error("resource graph is malformed")

  const identities = new Set<string>()
  const logicalPaths = new Set<string>()
  let priorChunk = ""
  let entryCount = 0
  const chunks = value.chunks.map((candidate): ResourceChunkDescriptor => {
    if (
      !record(candidate)
      || Object.keys(candidate).sort().join("\0") !== "codec\0decodedByteLength\0decodedSha256\0encodedByteLength\0encodedSha256\0entries\0roles"
      || (candidate.codec !== "identity" && candidate.codec !== "deflate")
      || integer(candidate.encodedByteLength, MAX_CHUNK_BYTES) === undefined
      || integer(candidate.decodedByteLength, MAX_CHUNK_BYTES) === undefined
      || !HASH.test(candidate.encodedSha256 as string)
      || !HASH.test(candidate.decodedSha256 as string)
      || (candidate.encodedSha256 as string) <= priorChunk
      || identities.has(candidate.encodedSha256 as string)
      || !Array.isArray(candidate.roles)
      || candidate.roles.length < 1
      || candidate.roles.length > 4
      || candidate.roles.some((role) => typeof role !== "string" || !ROLE.test(role))
      || candidate.roles.some((role, index) => index > 0 && candidate.roles[index - 1] >= role)
      || !Array.isArray(candidate.entries)
      || candidate.entries.length < 1
      || candidate.entries.length > MAX_CHUNK_ENTRIES
    ) throw new Error("resource chunk descriptor is malformed")
    priorChunk = candidate.encodedSha256 as string
    identities.add(priorChunk)
    let priorPath = ""
    const entries = candidate.entries.map((entry): ResourceEntryDescriptor => {
      if (
        !record(entry)
        || Object.keys(entry).sort().join("\0") !== "byteLength\0logicalPath\0offset\0sha256"
        || typeof entry.logicalPath !== "string"
        || entry.logicalPath.length < 1
        || entry.logicalPath.length > 4_096
        || entry.logicalPath !== entry.logicalPath.toLowerCase()
        || entry.logicalPath <= priorPath
        || logicalPaths.has(entry.logicalPath)
        || integer(entry.offset, MAX_CHUNK_BYTES) === undefined
        || integer(entry.byteLength, MAX_CHUNK_BYTES) === undefined
        || !HASH.test(entry.sha256 as string)
      ) throw new Error("resource entry descriptor is malformed")
      priorPath = entry.logicalPath
      logicalPaths.add(priorPath)
      entryCount += 1
      return Object.freeze(entry as ResourceEntryDescriptor)
    })
    return Object.freeze({ ...candidate, roles: Object.freeze([...candidate.roles] as string[]), entries: Object.freeze(entries) } as ResourceChunkDescriptor)
  })
  if (entryCount > MAX_GRAPH_ENTRIES) throw new Error("resource graph entry bound is exceeded")
  return Object.freeze({ ...value, chunks: Object.freeze(chunks) } as ResourceGraph)
}

export function parseResourceCatalog(value: unknown): ResourceCatalog {
  if (
    !record(value)
    || Object.keys(value).sort().join("\0") !== "application\0entries\0schema"
    || value.schema !== "playsrc-resource-catalog-v1"
    || typeof value.application !== "string"
    || !/^[a-z0-9-]{1,64}$/.test(value.application)
    || !Array.isArray(value.entries)
    || value.entries.length < 1
    || value.entries.length > 100_000
  ) throw new Error("resource catalog is malformed")
  let prior = ""
  const entries = value.entries.map((entry): ResourceCatalogEntry => {
    if (
      !record(entry)
      || Object.keys(entry).sort().join("\0") !== "resources\0target"
      || typeof entry.target !== "string"
      || !/^[a-z0-9_]{1,64}$/.test(entry.target)
      || entry.target <= prior
      || !record(entry.resources)
      || Object.keys(entry.resources).sort().join("\0") !== "byteLength\0kind\0mediaType\0sha256"
      || entry.resources.kind !== "source-root"
      || entry.resources.mediaType !== RESOURCE_GRAPH_MEDIA_TYPE
      || integer(entry.resources.byteLength, 8 * 1024 * 1024) === undefined
      || !HASH.test(entry.resources.sha256 as string)
    ) throw new Error("resource catalog entry is malformed")
    prior = entry.target
    return Object.freeze({ target: entry.target, resources: Object.freeze(entry.resources as ObjectDescriptor) })
  })
  return Object.freeze({ schema: "playsrc-resource-catalog-v1", application: value.application, entries: Object.freeze(entries) })
}

export function selectCatalogTarget(catalog: ResourceCatalog, target: string): ResourceCatalogEntry {
  if (!/^[a-z0-9_]{1,64}$/.test(target)) throw new Error("resource catalog target is malformed")
  const entry = catalog.entries.find((candidate) => candidate.target === target)
  if (!entry) throw new Error("resource catalog target is absent")
  return entry
}

export function parseResourceGraphBytes(bytes: Uint8Array): ResourceGraph {
  if (bytes.byteLength < 2 || bytes.byteLength > 8 * 1024 * 1024) throw new Error("resource graph byte bound is exceeded")
  const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes)
  const value = JSON.parse(text) as unknown
  if (canonicalGraphJson(value) !== text) throw new Error("resource graph is not canonical JSON")
  return parseResourceGraph(value)
}

export function parseResourceCatalogBytes(bytes: Uint8Array): ResourceCatalog {
  if (bytes.byteLength < 2 || bytes.byteLength > 8 * 1024 * 1024) throw new Error("resource catalog byte bound is exceeded")
  const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes)
  const value = JSON.parse(text) as unknown
  if (canonicalGraphJson(value) !== text) throw new Error("resource catalog is not canonical JSON")
  return parseResourceCatalog(value)
}

function canonicalGraphJson(value: unknown): string {
  if (value === null || typeof value === "boolean" || typeof value === "string") return JSON.stringify(value)
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value)) throw new Error("graph JSON number is not an integer")
    return String(value)
  }
  if (Array.isArray(value)) return `[${value.map(canonicalGraphJson).join(",")}]`
  if (record(value)) return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalGraphJson(value[key])}`).join(",")}}`
  throw new Error("graph JSON value is malformed")
}

export function canonicalGraphBytes(value: unknown): Uint8Array {
  return new TextEncoder().encode(canonicalGraphJson(value))
}

export function resourceChunkObject(chunk: ResourceChunkDescriptor): ObjectDescriptor {
  return Object.freeze({ kind: "derived-object", mediaType: RESOURCE_CHUNK_MEDIA_TYPE, byteLength: chunk.encodedByteLength, sha256: chunk.encodedSha256 })
}

export function chunksForRole(graph: ResourceGraph, role: string): readonly ResourceChunkDescriptor[] {
  if (!ROLE.test(role)) throw new Error("resource role is malformed")
  return Object.freeze(graph.chunks.filter((chunk) => chunk.roles.includes(role)))
}

export type EncodedResourceChunk = Readonly<{ descriptor: ResourceChunkDescriptor; bytes: Uint8Array }>

export function partitionResourceChunkDescriptors(
  chunks: readonly ResourceChunkDescriptor[],
  maximumBytes = 128 * 1024 * 1024,
): readonly (readonly ResourceChunkDescriptor[])[] {
  if (!Number.isSafeInteger(maximumBytes) || maximumBytes < 12 || maximumBytes > 512 * 1024 * 1024) {
    throw new Error("resource section byte bound is invalid")
  }
  if (chunks.length < 1 || chunks.length > MAX_GRAPH_CHUNKS) throw new Error("resource batch chunk bound is exceeded")
  const encoder = new TextEncoder()
  const sections: ResourceChunkDescriptor[][] = []
  const identities = new Set<string>()
  let section: ResourceChunkDescriptor[] = []
  let encodedBytes = 12
  let decodedBytes = 12
  for (const chunk of chunks) {
    if (identities.has(chunk.encodedSha256)) continue
    const encoded = 8 + encoder.encode(JSON.stringify(chunk)).byteLength + Number(chunk.encodedByteLength)
    const decoded = chunk.entries.reduce(
      (total, entry) => total + 8 + encoder.encode(entry.logicalPath).byteLength + Number(entry.byteLength),
      0,
    )
    if (12 + encoded > maximumBytes || 12 + decoded > maximumBytes) {
      throw new Error("resource chunk exceeds section byte bound")
    }
    if (section.length > 0 && (encodedBytes + encoded > maximumBytes || decodedBytes + decoded > maximumBytes)) {
      sections.push(Object.freeze(section) as ResourceChunkDescriptor[])
      section = []
      encodedBytes = 12
      decodedBytes = 12
    }
    section.push(chunk)
    identities.add(chunk.encodedSha256)
    encodedBytes += encoded
    decodedBytes += decoded
  }
  if (section.length > 0) sections.push(Object.freeze(section) as ResourceChunkDescriptor[])
  return Object.freeze(sections)
}

export function partitionResourceChunks(
  chunks: readonly EncodedResourceChunk[],
  maximumBytes = 128 * 1024 * 1024,
): readonly (readonly EncodedResourceChunk[])[] {
  const records = new Map<string, EncodedResourceChunk>()
  for (const chunk of chunks) {
    if (chunk.bytes.byteLength !== Number(chunk.descriptor.encodedByteLength)) {
      throw new Error("resource section encoded byte length differs")
    }
    records.set(chunk.descriptor.encodedSha256, chunk)
  }
  return Object.freeze(partitionResourceChunkDescriptors(chunks.map((chunk) => chunk.descriptor), maximumBytes)
    .map((section) => Object.freeze(section.map((descriptor) => records.get(descriptor.encodedSha256)!))))
}

export function encodeResourceBatch(chunks: readonly EncodedResourceChunk[]): Uint8Array {
  if (chunks.length < 1 || chunks.length > MAX_GRAPH_CHUNKS) throw new Error("resource batch chunk bound is exceeded")
  const encoder = new TextEncoder()
  const records = chunks.map((chunk) => ({ descriptor: encoder.encode(JSON.stringify(chunk.descriptor)), bytes: chunk.bytes }))
  const byteLength = 12 + records.reduce((total, record) => total + 8 + record.descriptor.byteLength + record.bytes.byteLength, 0)
  if (byteLength > 512 * 1024 * 1024) throw new Error("resource batch byte bound is exceeded")
  const output = new Uint8Array(byteLength)
  const view = new DataView(output.buffer)
  output.set(encoder.encode("PSGB"), 0)
  view.setUint32(4, 1, true)
  view.setUint32(8, records.length, true)
  let offset = 12
  for (const record of records) {
    view.setUint32(offset, record.descriptor.byteLength, true)
    offset += 4
    output.set(record.descriptor, offset)
    offset += record.descriptor.byteLength
    view.setUint32(offset, record.bytes.byteLength, true)
    offset += 4
    output.set(record.bytes, offset)
    offset += record.bytes.byteLength
  }
  return output
}

export function parseResourceSet(bytes: Uint8Array): ReadonlyMap<string, Uint8Array> {
  if (bytes.byteLength < 12 || bytes[0] !== 0x50 || bytes[1] !== 0x53 || bytes[2] !== 0x52 || bytes[3] !== 0x45) {
    throw new Error("resource set is malformed")
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  if (view.getUint32(4, true) !== 1) throw new Error("resource set version is invalid")
  const count = view.getUint32(8, true)
  if (count < 1 || count > MAX_GRAPH_ENTRIES) throw new Error("resource set entry bound is exceeded")
  let offset = 12
  const field = (): Uint8Array => {
    if (offset + 4 > bytes.byteLength) throw new Error("resource set is truncated")
    const length = view.getUint32(offset, true)
    offset += 4
    if (offset + length > bytes.byteLength) throw new Error("resource set is truncated")
    const value = bytes.subarray(offset, offset + length)
    offset += length
    return value
  }
  const decoder = new TextDecoder("utf-8", { fatal: true })
  const result = new Map<string, Uint8Array>()
  let prior = ""
  for (let index = 0; index < count; index += 1) {
    const path = field()
    const logicalPath = decoder.decode(path.buffer instanceof SharedArrayBuffer ? path.slice() : path)
    if (!logicalPath || logicalPath <= prior || logicalPath !== logicalPath.toLowerCase()) throw new Error("resource set identity is malformed")
    prior = logicalPath
    result.set(logicalPath, field())
  }
  if (offset !== bytes.byteLength) throw new Error("resource set has trailing bytes")
  return result
}
