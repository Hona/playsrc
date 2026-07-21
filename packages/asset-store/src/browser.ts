import type { ObjectDescriptor } from "./index"

const HASH = /^[0-9a-f]{64}$/
const MAX_OBJECT_BYTES = 536_870_912
const CACHE_OPERATION_TIMEOUT_MILLISECONDS = 30_000
const MAX_CACHE_BYTES = 1024 * 1024 * 1024
const MAX_CACHE_RECORDS = 4_096
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
  bytes: Blob
  storedAt: number
}>

export type DerivedCacheMetadata = Readonly<{ key: string; byteLength: number; storedAt: number }>

function sameDerivedRecordMetadata(left: DerivedRecord, right: DerivedRecord): boolean {
  return left.key === right.key
    && left.byteLength === right.byteLength
    && left.sha256 === right.sha256
    && left.storedAt === right.storedAt
    && left.bytes.size === right.bytes.size
}

export function planDerivedCacheEviction(
  records: readonly DerivedCacheMetadata[],
  incoming: Readonly<{ key: string; byteLength: number }>,
  maximumBytes = MAX_CACHE_BYTES,
  maximumRecords = MAX_CACHE_RECORDS,
): readonly string[] {
  if (!HASH.test(incoming.key) || !Number.isSafeInteger(incoming.byteLength) || incoming.byteLength < 0 || incoming.byteLength > maximumBytes
    || !Number.isSafeInteger(maximumBytes) || maximumBytes < 1 || !Number.isSafeInteger(maximumRecords) || maximumRecords < 1) {
    throw new BrowserAssetError("BoundExceeded", "derived cache admission exceeds its bound")
  }
  const retained = records.filter((record) => record.key !== incoming.key)
  if (retained.some((record) => !HASH.test(record.key) || !Number.isSafeInteger(record.byteLength) || record.byteLength < 0
    || !Number.isSafeInteger(record.storedAt) || record.storedAt < 0)) {
    throw new BrowserAssetError("IntegrityFailure", "derived cache inventory is malformed")
  }
  let bytes = retained.reduce((total, record) => total + record.byteLength, incoming.byteLength)
  let count = retained.length + 1
  const evicted: string[] = []
  for (const record of [...retained].sort((left, right) => left.storedAt - right.storedAt || left.key.localeCompare(right.key))) {
    if (bytes <= maximumBytes && count <= maximumRecords) break
    bytes -= record.byteLength
    count -= 1
    evicted.push(record.key)
  }
  if (bytes > maximumBytes || count > maximumRecords) throw new BrowserAssetError("BoundExceeded", "derived cache cannot admit the object")
  return Object.freeze(evicted)
}

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
  onProgress?: (loadedBytes: number, totalBytes: number) => void,
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
    || response.headers.get("etag") === null
  ) {
    throw new BrowserAssetError("ResponseFailure", "immutable object response metadata differs")
  }
  if (signal?.aborted) throw new BrowserAssetError("Cancelled", "immutable object request was cancelled")
  const progress = (loadedBytes: number) => {
    try { onProgress?.(loadedBytes, expectedLength) } catch {}
  }
  progress(0)
  let bytes: Uint8Array
  try {
    if (!response.body) throw new Error("body")
    bytes = new Uint8Array(expectedLength)
    const reader = response.body.getReader()
    let offset = 0
    while (true) {
      const result = await reader.read()
      if (signal?.aborted) {
        await reader.cancel().catch(() => {})
        throw new BrowserAssetError("Cancelled", "immutable object request was cancelled")
      }
      if (result.done) break
      if (offset + result.value.byteLength > bytes.byteLength) throw new BrowserAssetError("IntegrityFailure", "immutable object response exceeds its descriptor")
      bytes.set(result.value, offset)
      offset += result.value.byteLength
      progress(offset)
    }
    if (offset !== bytes.byteLength) throw new BrowserAssetError("IntegrityFailure", "immutable object response is shorter than its descriptor")
  } catch (error) {
    if (signal?.aborted) throw new BrowserAssetError("Cancelled", "immutable object request was cancelled")
    if (error instanceof BrowserAssetError) throw error
    throw new BrowserAssetError("ResponseFailure", "immutable object response body failed")
  }
  if (signal?.aborted) throw new BrowserAssetError("Cancelled", "immutable object request was cancelled")
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
    Object.keys(record).sort().join("\0") !== "byteLength\0bytes\0key\0sha256\0storedAt"
    || record.key !== expectedKey
    || !Number.isSafeInteger(record.byteLength)
    || (record.byteLength as number) < 0
    || (record.byteLength as number) > MAX_OBJECT_BYTES
    || !HASH.test(record.sha256 as string)
    || !(record.bytes instanceof Blob)
    || record.bytes.size !== record.byteLength
    || !Number.isSafeInteger(record.storedAt)
    || (record.storedAt as number) < 0
  ) {
    throw new BrowserAssetError("IntegrityFailure", "derived cache record is malformed")
  }
  const bytes = new Uint8Array(await record.bytes.arrayBuffer())
  if (await sha256(bytes) !== record.sha256) {
    throw new BrowserAssetError("IntegrityFailure", "derived cache record hash differs")
  }
  return bytes
}

function requestResult<T>(request: IDBRequest<T>,operation:string): Promise<T> {
  return new Promise((resolve, reject) => {
    let settled=false
    const finish=(action:()=>void)=>{if(settled)return;settled=true;clearTimeout(timeout);action()}
    const timeout=setTimeout(()=>finish(()=>reject(new BrowserAssetError("PersistenceUnavailable",`IndexedDB ${operation} timed out after ${CACHE_OPERATION_TIMEOUT_MILLISECONDS} ms`))),CACHE_OPERATION_TIMEOUT_MILLISECONDS)
    request.onsuccess = () => {
      if(settled){const value=request.result;if(value instanceof IDBDatabase)value.close();return}
      finish(()=>resolve(request.result))
    }
    request.onerror = () => finish(()=>reject(new BrowserAssetError("PersistenceUnavailable",`IndexedDB ${operation} failed${request.error?`: ${request.error.name}: ${request.error.message}`:""}`)))
  })
}

function transactionDone(transaction: IDBTransaction,operation:string): Promise<void> {
  return new Promise((resolve, reject) => {
    let settled=false
    const finish=(action:()=>void)=>{if(settled)return;settled=true;clearTimeout(timeout);action()}
    const timeout=setTimeout(()=>finish(()=>{try{transaction.abort()}catch{}reject(new BrowserAssetError("PersistenceUnavailable",`IndexedDB ${operation} timed out after ${CACHE_OPERATION_TIMEOUT_MILLISECONDS} ms`))}),CACHE_OPERATION_TIMEOUT_MILLISECONDS)
    transaction.oncomplete = () => finish(resolve)
    transaction.onabort = transaction.onerror = () => {
      const error=transaction.error
      finish(()=>reject(new BrowserAssetError("PersistenceUnavailable",`IndexedDB ${operation} failed${error?`: ${error.name}: ${error.message}`:""}`)))
    }
  })
}

function bounded<T>(promise:Promise<T>,operation:string):Promise<T>{
  return new Promise((resolve,reject)=>{let settled=false;const finish=(action:()=>void)=>{if(settled)return;settled=true;clearTimeout(timeout);action()},timeout=setTimeout(()=>finish(()=>reject(new BrowserAssetError("PersistenceUnavailable",`${operation} timed out after ${CACHE_OPERATION_TIMEOUT_MILLISECONDS} ms`))),CACHE_OPERATION_TIMEOUT_MILLISECONDS);promise.then(value=>finish(()=>resolve(value)),error=>finish(()=>reject(error)))})
}

export async function openDerivedObjectCache(
  databaseName = "playsrc-derived-v3",
): Promise<DerivedObjectCache> {
  if (!globalThis.indexedDB) {
    throw new BrowserAssetError("PersistenceUnavailable", "IndexedDB is unavailable")
  }
  const request = globalThis.indexedDB.open(databaseName, 1)
  request.onupgradeneeded = () => request.result.createObjectStore("objects", { keyPath: "key" })
  const database = await requestResult(request,"open")
  database.onversionchange = () => database.close()
  const store = (mode: IDBTransactionMode): [IDBObjectStore, IDBTransaction] => {
    try {
      const transaction = database.transaction("objects", mode)
      return [transaction.objectStore("objects"), transaction]
    } catch {
      throw new BrowserAssetError("PersistenceUnavailable", "IndexedDB object store is unavailable")
    }
  }
  const refreshVerifiedRecency = async (verified: DerivedRecord): Promise<void> => {
    const [objects, transaction] = store("readwrite")
    const done = transactionDone(transaction, "read recency transaction")
    const [current, inventory] = await Promise.all([
      requestResult(objects.get(verified.key), "read recency request") as Promise<DerivedRecord | undefined>,
      requestResult(objects.getAll(), "read recency inventory request") as Promise<DerivedRecord[]>,
    ])
    if (!current || !sameDerivedRecordMetadata(current, verified)) {
      await done
      return
    }
    if (inventory.some((record) => !Number.isSafeInteger(record.storedAt) || record.storedAt < 0)) {
      try { transaction.abort() } catch {}
      await done.catch(() => {})
      throw new BrowserAssetError("IntegrityFailure", "derived cache recency inventory is malformed")
    }
    const now = Date.now()
    const maximum = inventory.reduce((value, record) => Math.max(value, record.storedAt), 0)
    const storedAt = Math.max(now, maximum + 1)
    if (!Number.isSafeInteger(now) || now < 0 || !Number.isSafeInteger(storedAt)) {
      try { transaction.abort() } catch {}
      await done.catch(() => {})
      throw new BrowserAssetError("BoundExceeded", "derived cache recency exceeds its bound")
    }
    await requestResult(objects.put({ ...current, storedAt } satisfies DerivedRecord), "read recency write request")
    await done
  }
  const cache: DerivedObjectCache = {
    async read(key: string): Promise<Uint8Array | undefined> {
      if (!HASH.test(key)) throw new BrowserAssetError("MalformedIdentity", "derived key is not canonical")
      const [objects, transaction] = store("readonly")
      const done = transactionDone(transaction,"read transaction")
      const value = await requestResult(objects.get(key),"read request")
      await done
      if (value === undefined) return undefined
      const bytes = await bounded(verifyDerivedRecord(value, key),"derived record verification")
      await refreshVerifiedRecency(value as DerivedRecord)
      return bytes
    },
    async write(key: string, expectedSha256: string, bytes: Uint8Array): Promise<void> {
      if (!HASH.test(key) || !HASH.test(expectedSha256)) {
        throw new BrowserAssetError("MalformedIdentity", "derived identity is not canonical")
      }
      if (bytes.byteLength > MAX_OBJECT_BYTES) {
        throw new BrowserAssetError("BoundExceeded", "derived object exceeds browser byte limit")
      }
      if (await bounded(sha256(bytes),"derived write hash") !== expectedSha256) {
        throw new BrowserAssetError("IntegrityFailure", "derived bytes differ from their descriptor")
      }
      const [objects, transaction] = store("readwrite")
      const done = transactionDone(transaction,"write transaction")
      const inventory = await requestResult(objects.getAll(),"inventory request") as unknown as DerivedRecord[]
      const storedAt = Date.now()
      const evicted = planDerivedCacheEviction(inventory.map((record) => ({
        key: record.key,
        byteLength: record.byteLength,
        storedAt: record.storedAt,
      })), { key, byteLength: bytes.byteLength })
      for (const evictedKey of evicted) objects.delete(evictedKey)
      let requestError:unknown
      void requestResult(objects.add({
        key,
        byteLength: bytes.byteLength,
        sha256: expectedSha256,
        bytes: new Blob([bytes],{type:"application/octet-stream"}),
        storedAt,
      } satisfies DerivedRecord),"write request").catch(error=>{requestError=error})
      try {
        await done
      } catch (error) {
        const failure=requestError??error
        if(failure instanceof BrowserAssetError&&failure.message.includes("timed out"))throw failure
        const existing = await cache.read(key)
        if (!existing || await bounded(sha256(existing),"existing derived hash") !== expectedSha256) throw failure
      }
    },
    async remove(key: string): Promise<void> {
      if (!HASH.test(key)) throw new BrowserAssetError("MalformedIdentity", "derived key is not canonical")
      const [objects, transaction] = store("readwrite")
      const done = transactionDone(transaction,"remove transaction")
      objects.delete(key)
      await done
    },
    close(): void {
      database.close()
    },
  }
  return Object.freeze(cache)
}
