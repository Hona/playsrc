import type { ObjectDescriptor } from "./index"

const HASH = /^[0-9a-f]{64}$/
const MAX_OBJECT_BYTES = 536_870_912
const CACHE_OPERATION_TIMEOUT_MILLISECONDS = 30_000
const MAX_CACHE_BYTES = 1024 * 1024 * 1024
const MAX_CACHE_RECORDS = 4_096
const VERIFIED_RECORD_VERSION = 1
const VERIFIED_BLOB_TYPE = "application/x-playsrc-verified"
const TRANSACTION_IDENTITY = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/
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
  verificationVersion?: number
  transactionIdentity?: string
}>

export type DerivedCacheMetadata = Readonly<{
  key: string
  byteLength: number
  storedAt: number
  sha256?: string
  verificationVersion?: number
  transactionIdentity?: string
}>
export type VerifiedDerivedObject = Readonly<{ bytes: Uint8Array; sha256: string }>

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
  read(key: string): Promise<VerifiedDerivedObject | undefined>
  write(key: string, expectedSha256: string | null, bytes: Uint8Array): Promise<string>
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
  cacheMode: RequestCache = "force-cache",
): Promise<Uint8Array> {
  const expectedLength = length(descriptor)
  const url = objectUrl(origin, descriptor.sha256)
  if (signal?.aborted) throw new BrowserAssetError("Cancelled", "immutable object request was cancelled")
  let response: Response
  try {
    response = await fetcher(url, {
      method: "GET",
      cache: cacheMode,
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

export type ImmutableObjectPriority = "critical" | "normal" | "background"

export type ImmutableObjectAcquisition = Readonly<{
  signal?: AbortSignal
  priority?: ImmutableObjectPriority
  onProgress?: (loadedBytes: number, totalBytes: number) => void
}>

export type ImmutableObjectAcquirer = (
  origin: string,
  descriptor: ObjectDescriptor,
  options?: ImmutableObjectAcquisition,
) => Promise<Uint8Array>

export type ImmutableObjectCacheEvent = Readonly<{
  kind: "hit" | "miss" | "corrupt" | "write"
  sha256: string
  byteLength: number
  milliseconds: number
  verification?: "verified-at-write" | "rehash"
  hashMilliseconds?: number
}>

const verifiedReadEvidence = new WeakMap<Uint8Array, Readonly<{ verification: "verified-at-write" | "rehash"; hashMilliseconds: number }>>()
const verifiedAdmissionEvidence = new WeakMap<Uint8Array, string>()

export function createImmutableObjectAcquirer(options: Readonly<{
  concurrency?: number
  fetcher?: typeof fetch
  cache?: () => Promise<DerivedObjectCache>
  onCacheEvent?: (event: ImmutableObjectCacheEvent) => void
}> = {}): ImmutableObjectAcquirer {
  const concurrency = options.concurrency ?? 8
  if (!Number.isSafeInteger(concurrency) || concurrency < 1 || concurrency > 32) {
    throw new BrowserAssetError("BoundExceeded", "immutable transfer concurrency exceeds its browser bound")
  }
  type Subscriber = {
    signal?: AbortSignal
    progress?: (loadedBytes: number, totalBytes: number) => void
    resolve(bytes: Uint8Array): void
    reject(error: unknown): void
    abort?(): void
  }
  type Transfer = {
    identity: string
    origin: string
    descriptor: ObjectDescriptor
    priority: number
    order: number
    controller: AbortController
    subscribers: Set<Subscriber>
    started: boolean
    loaded?: number
    total?: number
  }
  const priorities: Record<ImmutableObjectPriority, number> = { critical: 0, normal: 1, background: 2 }
  const transfers = new Map<string, Transfer>()
  const queue: Transfer[] = []
  let active = 0
  let order = 0

  const reportCache = (kind: ImmutableObjectCacheEvent["kind"], descriptor: ObjectDescriptor, started: number, bytes?: Uint8Array): void => {
    try {
      options.onCacheEvent?.(Object.freeze({
        kind,
        sha256: descriptor.sha256,
        byteLength: Number(descriptor.byteLength),
        milliseconds: performance.now() - started,
        ...(bytes ? verifiedReadEvidence.get(bytes) : undefined),
      }))
    } catch {}
  }

  const acquire = async (transfer: Transfer, progress: (loaded: number, total: number) => void): Promise<Uint8Array> => {
    if (!options.cache) return fetchImmutableObject(transfer.origin, transfer.descriptor, transfer.controller.signal, options.fetcher ?? fetch, progress)
    const cache = await options.cache()
    if (transfer.controller.signal.aborted) throw new BrowserAssetError("Cancelled", "immutable object request was cancelled")
    let started = performance.now()
    let retained: VerifiedDerivedObject | undefined
    let corrupted = false
    try {
      retained = await cache.read(transfer.descriptor.sha256)
      if (retained && (retained.sha256 !== transfer.descriptor.sha256 || retained.bytes.byteLength !== Number(transfer.descriptor.byteLength))) {
        throw new BrowserAssetError("IntegrityFailure", "cached immutable object differs from its descriptor")
      }
    } catch (error) {
      if (!(error instanceof BrowserAssetError) || error.code !== "IntegrityFailure") throw error
      reportCache("corrupt", transfer.descriptor, started)
      corrupted = true
      await cache.remove(transfer.descriptor.sha256)
      retained = undefined
    }
    if (transfer.controller.signal.aborted) throw new BrowserAssetError("Cancelled", "immutable object request was cancelled")
    if (retained) {
      reportCache("hit", transfer.descriptor, started, retained.bytes)
      progress(0, retained.bytes.byteLength)
      progress(retained.bytes.byteLength, retained.bytes.byteLength)
      return retained.bytes
    }
    reportCache("miss", transfer.descriptor, started)
    const bytes = await fetchImmutableObject(transfer.origin, transfer.descriptor, transfer.controller.signal, options.fetcher ?? fetch, progress, corrupted ? "no-store" : "force-cache")
    if (transfer.controller.signal.aborted) throw new BrowserAssetError("Cancelled", "immutable object request was cancelled")
    started = performance.now()
    verifiedAdmissionEvidence.set(bytes, transfer.descriptor.sha256)
    await cache.write(transfer.descriptor.sha256, transfer.descriptor.sha256, bytes)
    reportCache("write", transfer.descriptor, started)
    if (transfer.controller.signal.aborted) throw new BrowserAssetError("Cancelled", "immutable object request was cancelled")
    return bytes
  }

  const pump = (): void => {
    while (active < concurrency && queue.length > 0) {
      queue.sort((left, right) => left.priority - right.priority || left.order - right.order)
      const transfer = queue.shift()!
      if (transfer.subscribers.size === 0) continue
      transfer.started = true
      active += 1
      void acquire(transfer, (loaded, total) => {
        transfer.loaded = loaded
        transfer.total = total
        for (const subscriber of transfer.subscribers) {
          try { subscriber.progress?.(loaded, total) } catch {}
        }
      }).then((bytes) => {
        const subscribers = [...transfer.subscribers]
        transfer.subscribers.clear()
        if (transfers.get(transfer.identity) === transfer) transfers.delete(transfer.identity)
        active -= 1
        pump()
        for (const subscriber of subscribers) {
          if (subscriber.abort) subscriber.signal?.removeEventListener("abort", subscriber.abort)
          subscriber.resolve(bytes)
        }
      }, (error) => {
        const subscribers = [...transfer.subscribers]
        transfer.subscribers.clear()
        if (transfers.get(transfer.identity) === transfer) transfers.delete(transfer.identity)
        active -= 1
        pump()
        for (const subscriber of subscribers) {
          if (subscriber.abort) subscriber.signal?.removeEventListener("abort", subscriber.abort)
          subscriber.reject(error)
        }
      })
    }
  }

  return (origin, descriptor, acquisition = {}) => {
    try {
      length(descriptor)
      const identity = objectUrl(origin, descriptor.sha256)
      if (acquisition.signal?.aborted) {
        return Promise.reject(new BrowserAssetError("Cancelled", "immutable object request was cancelled"))
      }
      let transfer = transfers.get(identity)
      const priority = priorities[acquisition.priority ?? "normal"]
      if (priority === undefined) throw new BrowserAssetError("MalformedIdentity", "immutable transfer priority is invalid")
      if (transfer && (transfer.descriptor.byteLength !== descriptor.byteLength
        || transfer.descriptor.kind !== descriptor.kind || transfer.descriptor.mediaType !== descriptor.mediaType)) {
        throw new BrowserAssetError("IntegrityFailure", "shared immutable object descriptors differ")
      }
      if (!transfer) {
        transfer = { identity, origin, descriptor, priority, order: order++, controller: new AbortController(), subscribers: new Set(), started: false }
        transfers.set(identity, transfer)
        queue.push(transfer)
      } else if (!transfer.started) transfer.priority = Math.min(transfer.priority, priority)

      const current = transfer
      const result = new Promise<Uint8Array>((resolve, reject) => {
        const subscriber: Subscriber = { signal: acquisition.signal, progress: acquisition.onProgress, resolve, reject }
        if (subscriber.signal) {
          subscriber.abort = () => {
            current.subscribers.delete(subscriber)
            subscriber.signal!.removeEventListener("abort", subscriber.abort!)
            reject(new BrowserAssetError("Cancelled", "immutable object request was cancelled"))
            if (current.subscribers.size === 0) {
              if (transfers.get(current.identity) === current) transfers.delete(current.identity)
              current.controller.abort()
              if (!current.started) {
                const index = queue.indexOf(current)
                if (index !== -1) queue.splice(index, 1)
              }
            }
          }
          subscriber.signal.addEventListener("abort", subscriber.abort, { once: true })
        }
        current.subscribers.add(subscriber)
        if (current.loaded !== undefined && current.total !== undefined) {
          try { subscriber.progress?.(current.loaded, current.total) } catch {}
        }
      })
      pump()
      return result
    } catch (error) {
      return Promise.reject(error)
    }
  }
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
    !["byteLength\0bytes\0key\0sha256\0storedAt", "byteLength\0bytes\0key\0sha256\0storedAt\0transactionIdentity\0verificationVersion"].includes(Object.keys(record).sort().join("\0"))
    || record.key !== expectedKey
    || !Number.isSafeInteger(record.byteLength)
    || (record.byteLength as number) < 0
    || (record.byteLength as number) > MAX_OBJECT_BYTES
    || !HASH.test(record.sha256 as string)
    || !(record.bytes instanceof Blob)
    || record.bytes.size !== record.byteLength
    || !Number.isSafeInteger(record.storedAt)
    || (record.storedAt as number) < 0
    || (record.verificationVersion !== undefined && (record.verificationVersion !== VERIFIED_RECORD_VERSION
      || typeof record.transactionIdentity !== "string" || !TRANSACTION_IDENTITY.test(record.transactionIdentity)
      || record.bytes.type !== `${VERIFIED_BLOB_TYPE};identity=${record.transactionIdentity}`))
  ) {
    throw new BrowserAssetError("IntegrityFailure", "derived cache record is malformed")
  }
  const bytes = new Uint8Array(await record.bytes.arrayBuffer())
  if (await sha256(bytes) !== record.sha256) {
    throw new BrowserAssetError("IntegrityFailure", "derived cache record hash differs")
  }
  return bytes
}

function verifiedTransactionRecord(value: unknown, metadata: unknown, expectedKey: string): DerivedRecord | undefined {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return undefined
  const record = value as Record<string, unknown>
  if (record.verificationVersion === undefined && record.transactionIdentity === undefined) return undefined
  if (typeof metadata !== "object" || metadata === null || Array.isArray(metadata)) {
    throw new BrowserAssetError("IntegrityFailure", "derived cache verification metadata is missing")
  }
  const authority = metadata as Record<string, unknown>
  if (Object.keys(record).sort().join("\0") !== "byteLength\0bytes\0key\0sha256\0storedAt\0transactionIdentity\0verificationVersion"
    || Object.keys(authority).sort().join("\0") !== "byteLength\0key\0sha256\0storedAt\0transactionIdentity\0verificationVersion"
    || record.key !== expectedKey || authority.key !== expectedKey
    || !Number.isSafeInteger(record.byteLength) || (record.byteLength as number) < 0 || (record.byteLength as number) > MAX_OBJECT_BYTES
    || authority.byteLength !== record.byteLength || !HASH.test(record.sha256 as string) || authority.sha256 !== record.sha256
    || !(record.bytes instanceof Blob) || record.bytes.size !== record.byteLength
    || record.bytes.type !== `${VERIFIED_BLOB_TYPE};identity=${record.transactionIdentity}`
    || record.verificationVersion !== VERIFIED_RECORD_VERSION || authority.verificationVersion !== VERIFIED_RECORD_VERSION
    || typeof record.transactionIdentity !== "string" || !TRANSACTION_IDENTITY.test(record.transactionIdentity)
    || authority.transactionIdentity !== record.transactionIdentity
    || !Number.isSafeInteger(record.storedAt) || (record.storedAt as number) < 0
    || !Number.isSafeInteger(authority.storedAt) || (authority.storedAt as number) < (record.storedAt as number)) {
    throw new BrowserAssetError("IntegrityFailure", "derived cache verified transaction identity differs")
  }
  return record as DerivedRecord
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
  bounds: Readonly<{ maximumBytes?: number; maximumRecords?: number }> = {},
): Promise<DerivedObjectCache> {
  const maximumBytes = bounds.maximumBytes ?? MAX_CACHE_BYTES
  const maximumRecords = bounds.maximumRecords ?? MAX_CACHE_RECORDS
  if (!Number.isSafeInteger(maximumBytes) || maximumBytes < 1 || maximumBytes > MAX_CACHE_BYTES
    || !Number.isSafeInteger(maximumRecords) || maximumRecords < 1 || maximumRecords > MAX_CACHE_RECORDS) {
    throw new BrowserAssetError("BoundExceeded", "derived cache limits exceed their browser bounds")
  }
  if (!globalThis.indexedDB) {
    throw new BrowserAssetError("PersistenceUnavailable", "IndexedDB is unavailable")
  }
  const request = globalThis.indexedDB.open(databaseName, 2)
  request.onupgradeneeded = (event) => {
    const database = request.result
    const priorVersion = (event as IDBVersionChangeEvent).oldVersion ?? 0
    const objects = priorVersion === 0
      ? database.createObjectStore("objects", { keyPath: "key" })
      : request.transaction!.objectStore("objects")
    const metadata = database.createObjectStore("metadata", { keyPath: "key" })
    if (priorVersion === 0) return
    const cursor = objects.openCursor()
    cursor.onsuccess = () => {
      const current = cursor.result
      if (!current) return
      const record = current.value as DerivedRecord
      metadata.add({ key: record.key, byteLength: record.byteLength, storedAt: record.storedAt } satisfies DerivedCacheMetadata)
      current.continue()
    }
    cursor.onerror = () => request.transaction?.abort()
  }
  const database = await requestResult(request,"open")
  database.onversionchange = () => database.close()
  let latestReadRecency = 0
  const stores = (mode: IDBTransactionMode): [IDBObjectStore, IDBObjectStore, IDBTransaction] => {
    try {
      const transaction = database.transaction(["objects", "metadata"], mode)
      return [transaction.objectStore("objects"), transaction.objectStore("metadata"), transaction]
    } catch {
      throw new BrowserAssetError("PersistenceUnavailable", "IndexedDB object store is unavailable")
    }
  }
  const refreshVerifiedRecency = async (verified: DerivedRecord): Promise<void> => {
    const [, metadata, transaction] = stores("readwrite")
    const done = transactionDone(transaction, "read recency transaction")
    const current = await requestResult(metadata.get(verified.key), "read recency metadata request") as DerivedCacheMetadata | undefined
    if (!current) {
      await done
      return
    }
    if (current.key !== verified.key || current.byteLength !== verified.byteLength
      || !Number.isSafeInteger(current.storedAt) || current.storedAt < verified.storedAt
      || (verified.verificationVersion !== undefined && (current.verificationVersion !== verified.verificationVersion
        || current.transactionIdentity !== verified.transactionIdentity || current.sha256 !== verified.sha256))) {
      try { transaction.abort() } catch {}
      await done.catch(() => {})
      throw new BrowserAssetError("IntegrityFailure", "derived cache recency metadata is malformed")
    }
    const now = Date.now()
    const storedAt = Math.max(now, latestReadRecency + 1, current.storedAt + 1)
    if (!Number.isSafeInteger(now) || now < 0 || !Number.isSafeInteger(storedAt)) {
      try { transaction.abort() } catch {}
      await done.catch(() => {})
      throw new BrowserAssetError("BoundExceeded", "derived cache recency exceeds its bound")
    }
    await requestResult(metadata.put({ ...current, storedAt } satisfies DerivedCacheMetadata), "read recency metadata write request")
    await done
    latestReadRecency = Math.max(latestReadRecency, storedAt)
  }
  const cache: DerivedObjectCache = {
    async read(key: string): Promise<VerifiedDerivedObject | undefined> {
      if (!HASH.test(key)) throw new BrowserAssetError("MalformedIdentity", "derived key is not canonical")
      const [objects, metadata, transaction] = stores("readonly")
      const done = transactionDone(transaction,"read transaction")
      const [value, authority] = await Promise.all([
        requestResult(objects.get(key), "read request"),
        requestResult(metadata.get(key), "read metadata request"),
      ])
      await done
      if (value === undefined) {
        if (authority !== undefined) throw new BrowserAssetError("IntegrityFailure", "derived cache object is missing from its verified transaction")
        return undefined
      }
      const verified = verifiedTransactionRecord(value, authority, key)
      const verificationStarted = performance.now()
      const bytes = verified
        ? new Uint8Array(await bounded(verified.bytes.arrayBuffer(), "verified derived record bytes"))
        : await bounded(verifyDerivedRecord(value, key), "derived record verification")
      verifiedReadEvidence.set(bytes, {
        verification: verified ? "verified-at-write" : "rehash",
        hashMilliseconds: verified ? 0 : performance.now() - verificationStarted,
      })
      await refreshVerifiedRecency(value as DerivedRecord)
      return Object.freeze({ bytes, sha256: (value as DerivedRecord).sha256 })
    },
    async write(key: string, expectedSha256: string | null, bytes: Uint8Array): Promise<string> {
      if (!HASH.test(key) || (expectedSha256 !== null && !HASH.test(expectedSha256))) {
        throw new BrowserAssetError("MalformedIdentity", "derived identity is not canonical")
      }
      if (bytes.byteLength > MAX_OBJECT_BYTES || bytes.byteLength > maximumBytes) {
        throw new BrowserAssetError("BoundExceeded", "derived object exceeds browser byte limit")
      }
      const verified = expectedSha256 !== null && verifiedAdmissionEvidence.get(bytes) === expectedSha256
      verifiedAdmissionEvidence.delete(bytes)
      const actualSha256 = verified ? expectedSha256 : await bounded(sha256(bytes),"derived write hash")
      if (expectedSha256 !== null && actualSha256 !== expectedSha256) {
        throw new BrowserAssetError("IntegrityFailure", "derived bytes differ from their descriptor")
      }
      const [objects, metadata, transaction] = stores("readwrite")
      const done = transactionDone(transaction,"write transaction")
      const inventory = await requestResult(metadata.getAll(),"inventory request") as unknown as DerivedCacheMetadata[]
      const storedAt = Date.now()
      const transactionIdentity = crypto.randomUUID()
      const evicted = planDerivedCacheEviction(inventory.map((record) => ({
        key: record.key,
        byteLength: record.byteLength,
        storedAt: record.storedAt,
      })), { key, byteLength: bytes.byteLength }, maximumBytes, maximumRecords)
      for (const evictedKey of evicted) {
        objects.delete(evictedKey)
        metadata.delete(evictedKey)
      }
      let requestError:unknown
      void requestResult(objects.add({
        key,
        byteLength: bytes.byteLength,
        sha256: actualSha256,
        bytes: new Blob([bytes], { type: `${VERIFIED_BLOB_TYPE};identity=${transactionIdentity}` }),
        storedAt,
        verificationVersion: VERIFIED_RECORD_VERSION,
        transactionIdentity,
      } satisfies DerivedRecord),"write request").catch(error=>{requestError=error})
      void requestResult(metadata.add({
        key,
        byteLength: bytes.byteLength,
        storedAt,
        sha256: actualSha256,
        verificationVersion: VERIFIED_RECORD_VERSION,
        transactionIdentity,
      } satisfies DerivedCacheMetadata), "metadata write request").catch(error=>{requestError??=error})
      try {
        await done
      } catch (error) {
        const failure=requestError??error
        if(failure instanceof BrowserAssetError&&failure.message.includes("timed out"))throw failure
        const existing = await cache.read(key)
        if (!existing || existing.sha256 !== actualSha256) throw failure
      }
      return actualSha256
    },
    async remove(key: string): Promise<void> {
      if (!HASH.test(key)) throw new BrowserAssetError("MalformedIdentity", "derived key is not canonical")
      const [objects, metadata, transaction] = stores("readwrite")
      const done = transactionDone(transaction,"remove transaction")
      objects.delete(key)
      metadata.delete(key)
      await done
    },
    close(): void {
      database.close()
    },
  }
  return Object.freeze(cache)
}
