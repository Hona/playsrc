import type { ObjectDescriptor } from "./index"

const HASH = /^[0-9a-f]{64}$/
const MAX_OBJECT_BYTES = 536_870_912
const KINDS = new Set([
  "source-object",
  "derived-object",
  "source-root",
  "map-root",
  "game-root",
  "application-root",
  "catalog",
])

export class BrowserAssetError extends Error {
  constructor(
    readonly code:
      | "MalformedIdentity"
      | "ResponseFailure"
      | "IntegrityFailure"
      | "PersistenceUnavailable"
      | "BoundExceeded"
      | "Cancelled",
    message: string,
  ) {
    super(message)
    this.name = "BrowserAssetError"
  }
}

export type DerivedRecord = Readonly<{
  key: string
  byteLength: number
  sha256: string
  bytes: ArrayBuffer
}>

export type DerivedObjectCache = Readonly<{
  read(key: string): Promise<Uint8Array | undefined>
  write(key: string, sha256: string, bytes: Uint8Array): Promise<void>
  remove(key: string): Promise<void>
  close(): void
}>

async function sha256(bytes: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", bytes)
  return Array.from(new Uint8Array(digest), (value) => value.toString(16).padStart(2, "0")).join("")
}

function length(descriptor: ObjectDescriptor): number {
  if (
    !KINDS.has(descriptor.kind)
    || !descriptor.mediaType
    || /[\r\n\0]/.test(descriptor.mediaType)
    || !HASH.test(descriptor.sha256)
    || !/^(0|[1-9]\d*)$/.test(descriptor.byteLength)
  ) {
    throw new BrowserAssetError("MalformedIdentity", "object descriptor is not canonical")
  }
  const value = Number(descriptor.byteLength)
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new BrowserAssetError("MalformedIdentity", "object byte length is invalid")
  }
  if (value > MAX_OBJECT_BYTES) throw new BrowserAssetError("BoundExceeded", "object exceeds browser byte limit")
  return value
}

function objectUrl(origin: string, hash: string): string {
  let value: URL
  try {
    value = new URL(origin)
  } catch {
    throw new BrowserAssetError("MalformedIdentity", "asset origin is not an absolute URL")
  }
  const loopback = value.hostname === "localhost" || value.hostname === "127.0.0.1" || value.hostname === "[::1]"
  if (
    (value.protocol !== "https:" && !(value.protocol === "http:" && loopback))
    || value.username
    || value.password
    || value.pathname !== "/"
    || value.search
    || value.hash
  ) {
    throw new BrowserAssetError("MalformedIdentity", "asset origin is not an accepted origin")
  }
  return `${value.origin}/objects/sha256/${hash}`
}

export async function fetchImmutableObject(
  origin: string,
  descriptor: ObjectDescriptor,
  signal?: AbortSignal,
  fetcher: typeof fetch = fetch,
): Promise<Uint8Array> {
  const expectedLength = length(descriptor)
  const url = objectUrl(origin, descriptor.sha256)
  if (signal?.aborted) throw new BrowserAssetError("Cancelled", "immutable object request was cancelled")
  let response: Response
  try {
    response = await fetcher(url, {
      method: "GET",
      cache: "force-cache",
      credentials: "omit",
      redirect: "error",
      signal,
    })
  } catch {
    if (signal?.aborted) throw new BrowserAssetError("Cancelled", "immutable object request was cancelled")
    throw new BrowserAssetError("ResponseFailure", "immutable object request failed")
  }
  if (
    response.status !== 200
    || response.redirected
    || response.url !== url
    || response.headers.get("content-length") !== descriptor.byteLength
    || response.headers.get("etag") !== `"${descriptor.sha256}"`
  ) {
    throw new BrowserAssetError("ResponseFailure", "immutable object response metadata differs")
  }
  let bytes: Uint8Array
  try {
    bytes = new Uint8Array(await response.arrayBuffer())
  } catch {
    if (signal?.aborted) throw new BrowserAssetError("Cancelled", "immutable object request was cancelled")
    throw new BrowserAssetError("ResponseFailure", "immutable object response body failed")
  }
  if (bytes.byteLength !== expectedLength || await sha256(bytes) !== descriptor.sha256) {
    throw new BrowserAssetError("IntegrityFailure", "immutable object bytes differ")
  }
  return bytes
}

export async function verifyDerivedRecord(
  value: unknown,
  expectedKey: string,
): Promise<Uint8Array> {
  if (!HASH.test(expectedKey) || typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new BrowserAssetError("IntegrityFailure", "derived cache record is malformed")
  }
  const record = value as Record<string, unknown>
  if (
    Object.keys(record).sort().join("\0") !== "byteLength\0bytes\0key\0sha256"
    || record.key !== expectedKey
    || !Number.isSafeInteger(record.byteLength)
    || (record.byteLength as number) < 0
    || (record.byteLength as number) > MAX_OBJECT_BYTES
    || !HASH.test(record.sha256 as string)
    || !(record.bytes instanceof ArrayBuffer)
    || record.bytes.byteLength !== record.byteLength
  ) {
    throw new BrowserAssetError("IntegrityFailure", "derived cache record is malformed")
  }
  const bytes = new Uint8Array(record.bytes)
  if (await sha256(bytes) !== record.sha256) {
    throw new BrowserAssetError("IntegrityFailure", "derived cache record hash differs")
  }
  return bytes.slice()
}

function requestResult<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result)
    request.onerror = () => reject(new BrowserAssetError("PersistenceUnavailable", "IndexedDB request failed"))
  })
}

function transactionDone(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve()
    transaction.onabort = transaction.onerror = () => reject(
      new BrowserAssetError("PersistenceUnavailable", "IndexedDB transaction failed"),
    )
  })
}

export async function openDerivedObjectCache(
  databaseName = "playsrc-derived-v1",
): Promise<DerivedObjectCache> {
  if (!globalThis.indexedDB) {
    throw new BrowserAssetError("PersistenceUnavailable", "IndexedDB is unavailable")
  }
  const request = globalThis.indexedDB.open(databaseName, 1)
  request.onupgradeneeded = () => request.result.createObjectStore("objects", { keyPath: "key" })
  const database = await requestResult(request)
  database.onversionchange = () => database.close()
  const store = (mode: IDBTransactionMode): [IDBObjectStore, IDBTransaction] => {
    try {
      const transaction = database.transaction("objects", mode)
      return [transaction.objectStore("objects"), transaction]
    } catch {
      throw new BrowserAssetError("PersistenceUnavailable", "IndexedDB object store is unavailable")
    }
  }
  const cache: DerivedObjectCache = {
    async read(key: string): Promise<Uint8Array | undefined> {
      if (!HASH.test(key)) throw new BrowserAssetError("MalformedIdentity", "derived key is not canonical")
      const [objects, transaction] = store("readonly")
      const done = transactionDone(transaction)
      const value = await requestResult(objects.get(key))
      await done
      return value === undefined ? undefined : verifyDerivedRecord(value, key)
    },
    async write(key: string, expectedSha256: string, bytes: Uint8Array): Promise<void> {
      if (!HASH.test(key) || !HASH.test(expectedSha256)) {
        throw new BrowserAssetError("MalformedIdentity", "derived identity is not canonical")
      }
      if (bytes.byteLength > MAX_OBJECT_BYTES) {
        throw new BrowserAssetError("BoundExceeded", "derived object exceeds browser byte limit")
      }
      if (await sha256(bytes) !== expectedSha256) {
        throw new BrowserAssetError("IntegrityFailure", "derived bytes differ from their descriptor")
      }
      const [objects, transaction] = store("readwrite")
      const done = transactionDone(transaction)
      objects.add({
        key,
        byteLength: bytes.byteLength,
        sha256: expectedSha256,
        bytes: bytes.slice().buffer,
      } satisfies DerivedRecord)
      try {
        await done
      } catch (error) {
        const existing = await cache.read(key)
        if (!existing || await sha256(existing) !== expectedSha256) throw error
      }
    },
    async remove(key: string): Promise<void> {
      if (!HASH.test(key)) throw new BrowserAssetError("MalformedIdentity", "derived key is not canonical")
      const [objects, transaction] = store("readwrite")
      const done = transactionDone(transaction)
      objects.delete(key)
      await done
    },
    close(): void {
      database.close()
    },
  }
  return Object.freeze(cache)
}
