import { describe, expect, jest, test } from "bun:test"
import { BrowserAssetError, createImmutableObjectAcquirer, fetchImmutableObject, openDerivedObjectCache, planDerivedCacheEviction, verifyDerivedRecord } from "../src/browser"
import type { ObjectDescriptor } from "../src/index"

const bytes = new TextEncoder().encode("immutable")
const sha256 = "3e58bada6a180c0d7f817bdae51fba96a461575b309bfbc17a6918d20c6617c7"
const descriptor: ObjectDescriptor = {
  kind: "source-object",
  mediaType: "application/octet-stream",
  byteLength: String(bytes.byteLength),
  sha256,
}

type FakeRequest<T> = {
  result: T
  error: Error | null
  onsuccess: ((event: Event) => void) | null
  onerror: ((event: Event) => void) | null
}

class FakeTransaction {
  oncomplete: ((event: Event) => void) | null = null
  onabort: ((event: Event) => void) | null = null
  onerror: ((event: Event) => void) | null = null
  error: Error | null = null
  #pending = 0
  #completionQueued = false
  #aborted = false

  constructor(
    readonly stores: Map<string, Map<string, Record<string, unknown>>>,
    readonly inventories: { objects: number; metadata: number },
    readonly writes: { objects: number; metadata: number },
  ) {}

  objectStore(name = "objects"): IDBObjectStore {
    const records = this.stores.get(name)
    if (!records) throw new Error(`Object store ${name} is unavailable`)
    const request = <T>(operation: () => T): IDBRequest<T> => {
      const value: FakeRequest<T> = { result: undefined as T, error: null, onsuccess: null, onerror: null }
      this.#pending += 1
      queueMicrotask(() => {
        if (this.#aborted) return
        try {
          value.result = operation()
          value.onsuccess?.(new Event("success"))
        } catch (error) {
          value.error = error as Error
          this.error = error as Error
          value.onerror?.(new Event("error"))
          this.onerror?.(new Event("error"))
        } finally {
          this.#pending -= 1
          this.#queueCompletion()
        }
      })
      return value as unknown as IDBRequest<T>
    }
    return {
      get: (key: string) => request(() => records.get(key)),
      getAll: () => request(() => {
        if (name === "objects") this.inventories.objects += 1
        else if (name === "metadata") this.inventories.metadata += 1
        return [...records.values()]
      }),
      add: (record: Record<string, unknown>) => request(() => {
        const key = record.key as string
        if (records.has(key)) throw new Error("ConstraintError")
        if (name === "objects") this.writes.objects += 1
        else if (name === "metadata") this.writes.metadata += 1
        records.set(key, record)
        return key
      }),
      put: (record: Record<string, unknown>) => request(() => {
        const key = record.key as string
        if (name === "objects") this.writes.objects += 1
        else if (name === "metadata") this.writes.metadata += 1
        records.set(key, record)
        return key
      }),
      delete: (key: string) => request(() => { records.delete(key); return undefined }),
      openCursor: () => {
        const values = [...records.values()]
        let index = 0
        const cursor: FakeRequest<IDBCursorWithValue | null> = { result: null, error: null, onsuccess: null, onerror: null }
        const advance = () => {
          this.#pending += 1
          queueMicrotask(() => {
            if (this.#aborted) return
            try {
              cursor.result = index < values.length
                ? { value: values[index]!, continue: () => { index += 1; advance() } } as IDBCursorWithValue
                : null
              cursor.onsuccess?.(new Event("success"))
            } catch (error) {
              cursor.error = error as Error
              this.error = error as Error
              cursor.onerror?.(new Event("error"))
              this.onerror?.(new Event("error"))
            } finally {
              this.#pending -= 1
              this.#queueCompletion()
            }
          })
        }
        advance()
        return cursor as unknown as IDBRequest<IDBCursorWithValue | null>
      },
    } as unknown as IDBObjectStore
  }

  completeWhenIdle(): void {
    this.#queueCompletion()
  }

  abort(): void {
    if (this.#aborted) return
    this.#aborted = true
    this.onabort?.(new Event("abort"))
  }

  #queueCompletion(): void {
    if (this.#pending !== 0 || this.#completionQueued || this.#aborted) return
    this.#completionQueued = true
    queueMicrotask(() => {
      this.#completionQueued = false
      if (this.#pending === 0 && !this.#aborted) this.oncomplete?.(new Event("complete"))
    })
  }
}

class FakeIndexedDb {
  readonly databases = new Map<string, Map<string, Record<string, unknown>>>()
  readonly metadata = new Map<string, Map<string, Record<string, unknown>>>()
  readonly versions = new Map<string, number>()
  readonly inventories = { objects: 0, metadata: 0 }
  readonly writes = { objects: 0, metadata: 0 }

  open(name: string, version = 1): IDBOpenDBRequest {
    const request: FakeRequest<IDBDatabase> & {
      onupgradeneeded: ((event: IDBVersionChangeEvent) => void) | null
      transaction: IDBTransaction | null
    } = {
      result: undefined as unknown as IDBDatabase,
      error: null,
      onsuccess: null,
      onerror: null,
      onupgradeneeded: null,
      transaction: null,
    }
    queueMicrotask(() => {
      const priorVersion = this.versions.get(name) ?? (this.databases.has(name) ? 1 : 0)
      const records = this.databases.get(name) ?? new Map<string, Record<string, unknown>>()
      this.databases.set(name, records)
      const stores = new Map<string, Map<string, Record<string, unknown>>>([["objects", records]])
      const metadata = this.metadata.get(name)
      if (metadata) stores.set("metadata", metadata)
      const upgrade = new FakeTransaction(stores, this.inventories, this.writes)
      request.result = {
        createObjectStore: (storeName: string) => {
          if (storeName === "objects") return upgrade.objectStore("objects")
          const entries = new Map<string, Record<string, unknown>>()
          stores.set(storeName, entries)
          if (storeName === "metadata") this.metadata.set(name, entries)
          return upgrade.objectStore(storeName)
        },
        transaction: () => new FakeTransaction(stores, this.inventories, this.writes) as unknown as IDBTransaction,
        close: () => {},
        onversionchange: null,
      } as unknown as IDBDatabase
      if (version > priorVersion) {
        request.transaction = upgrade as unknown as IDBTransaction
        upgrade.oncomplete = () => {
          this.versions.set(name, version)
          request.transaction = null
          request.onsuccess?.(new Event("success"))
        }
        const event = new Event("upgradeneeded")
        Object.defineProperty(event, "oldVersion", { value: priorVersion })
        request.onupgradeneeded?.(event as IDBVersionChangeEvent)
        upgrade.completeWhenIdle()
        return
      }
      request.onsuccess?.(new Event("success"))
    })
    return request as unknown as IDBOpenDBRequest
  }
}

async function digest(value: Uint8Array): Promise<string> {
  return Array.from(new Uint8Array(await crypto.subtle.digest("SHA-256", value)), (byte) => byte.toString(16).padStart(2, "0")).join("")
}

describe("browser asset adapters", () => {
  test("reuses one authenticated persistent immutable authority across browser generations without native duplicate caching", async () => {
    const indexedDbDescriptor = Object.getOwnPropertyDescriptor(globalThis, "indexedDB")
    const fake = new FakeIndexedDb()
    Object.defineProperty(globalThis, "indexedDB", { configurable: true, value: fake })
    const url = `http://127.0.0.1:4321/objects/sha256/${sha256}`
    let requests = 0
    const modes: Array<RequestCache | undefined> = []
    const events: string[] = []
    const fetcher = (async (_input: RequestInfo | URL, init?: RequestInit) => {
      requests += 1
      modes.push(init?.cache)
      const response = new Response(bytes, { headers: { "content-length": String(bytes.byteLength), etag: `"${sha256}"` } })
      Object.defineProperty(response, "url", { value: url })
      return response
    }) as typeof fetch
    try {
      const coldCache = await openDerivedObjectCache("persistent-immutable-generations")
      const cold = createImmutableObjectAcquirer({ fetcher, cache: async () => coldCache, onCacheEvent: (event) => events.push(event.kind) })
      expect(await cold("http://127.0.0.1:4321/", descriptor)).toEqual(bytes)
      expect(requests).toBe(1)
      expect(modes).toEqual(["no-store"])
      const objectWrites = fake.writes.objects
      coldCache.close()

      const warmCache = await openDerivedObjectCache("persistent-immutable-generations")
      const warm = createImmutableObjectAcquirer({ fetcher, cache: async () => warmCache, onCacheEvent: (event) => events.push(event.kind) })
      const progress: number[] = []
      expect(await warm("http://127.0.0.1:4321/", descriptor, { onProgress: (loaded) => progress.push(loaded) })).toEqual(bytes)
      expect(requests).toBe(1)
      expect(fake.writes.objects).toBe(objectWrites)
      expect(progress).toEqual([0, bytes.byteLength])
      expect(events).toEqual(["miss", "write", "hit"])
      warmCache.close()
    } finally {
      if (indexedDbDescriptor) Object.defineProperty(globalThis, "indexedDB", indexedDbDescriptor)
      else delete (globalThis as { indexedDB?: IDBFactory }).indexedDB
    }
  })

  test("evicts corrupt immutable bytes before one freshly authenticated replacement", async () => {
    const indexedDbDescriptor = Object.getOwnPropertyDescriptor(globalThis, "indexedDB")
    const fake = new FakeIndexedDb()
    Object.defineProperty(globalThis, "indexedDB", { configurable: true, value: fake })
    const url = `http://127.0.0.1:4321/objects/sha256/${sha256}`
    let requests = 0
    const events: string[] = []
    const fetcher = (async () => {
      requests += 1
      const response = new Response(bytes, { headers: { "content-length": String(bytes.byteLength), etag: `"${sha256}"` } })
      Object.defineProperty(response, "url", { value: url })
      return response
    }) as typeof fetch
    try {
      const cache = await openDerivedObjectCache("corrupt-immutable-recovery")
      await cache.write(sha256, sha256, bytes)
      const records = fake.databases.get("corrupt-immutable-recovery")!
      records.set(sha256, { ...records.get(sha256)!, bytes: new Blob([new TextEncoder().encode("immutablE")]) })
      const acquire = createImmutableObjectAcquirer({ fetcher, cache: async () => cache, onCacheEvent: (event) => events.push(event.kind) })
      expect(await acquire("http://127.0.0.1:4321/", descriptor)).toEqual(bytes)
      expect(requests).toBe(1)
      expect(events).toEqual(["corrupt", "miss", "write"])
      expect(await cache.read(sha256)).toEqual({ bytes, sha256 })
      cache.close()
    } finally {
      if (indexedDbDescriptor) Object.defineProperty(globalThis, "indexedDB", indexedDbDescriptor)
      else delete (globalThis as { indexedDB?: IDBFactory }).indexedDB
    }
  })

  test("authenticates immutable bytes once at admission and trusts their atomic origin-owned Blob identity across generations", async () => {
    const indexedDbDescriptor = Object.getOwnPropertyDescriptor(globalThis, "indexedDB")
    const fake = new FakeIndexedDb()
    Object.defineProperty(globalThis, "indexedDB", { configurable: true, value: fake })
    const hashing = jest.spyOn(crypto.subtle, "digest")
    try {
      const first = await openDerivedObjectCache("verified-at-write-generations")
      await first.write(sha256, sha256, bytes)
      expect(hashing).toHaveBeenCalledTimes(1)
      const record = fake.databases.get("verified-at-write-generations")!.get(sha256)!
      const metadata = fake.metadata.get("verified-at-write-generations")!.get(sha256)!
      expect(record.transactionIdentity).toBe(metadata.transactionIdentity)
      expect((record.bytes as Blob).type).toBe(`application/x-playsrc-verified;identity=${record.transactionIdentity}`)
      first.close()

      const second = await openDerivedObjectCache("verified-at-write-generations")
      expect(await second.read(sha256)).toEqual({ bytes, sha256 })
      expect(await second.read(sha256)).toEqual({ bytes, sha256 })
      expect(hashing).toHaveBeenCalledTimes(1)
      second.close()
    } finally {
      hashing.mockRestore()
      if (indexedDbDescriptor) Object.defineProperty(globalThis, "indexedDB", indexedDbDescriptor)
      else delete (globalThis as { indexedDB?: IDBFactory }).indexedDB
    }
  })

  test("carries authenticated immutable transfer ownership directly into its atomic admission without hashing the same bytes twice", async () => {
    const indexedDbDescriptor = Object.getOwnPropertyDescriptor(globalThis, "indexedDB")
    const fake = new FakeIndexedDb()
    Object.defineProperty(globalThis, "indexedDB", { configurable: true, value: fake })
    const hashing = jest.spyOn(crypto.subtle, "digest")
    const url = `http://127.0.0.1:4321/objects/sha256/${sha256}`
    try {
      const cache = await openDerivedObjectCache("single-authenticated-immutable-admission")
      const acquire = createImmutableObjectAcquirer({
        cache: async () => cache,
        fetcher: (async () => {
          const response = new Response(bytes, { headers: { "content-length": String(bytes.byteLength), etag: `"${sha256}"` } })
          Object.defineProperty(response, "url", { value: url })
          return response
        }) as typeof fetch,
      })
      expect(await acquire("http://127.0.0.1:4321/", descriptor)).toEqual(bytes)
      expect(hashing).toHaveBeenCalledTimes(1)
      expect(await cache.read(sha256)).toEqual({ bytes, sha256 })
      expect(hashing).toHaveBeenCalledTimes(1)
      await expect(cache.write("0".repeat(64), "0".repeat(64), bytes)).rejects.toMatchObject({ code: "IntegrityFailure" })
      expect(hashing).toHaveBeenCalledTimes(2)
      cache.close()
    } finally {
      hashing.mockRestore()
      if (indexedDbDescriptor) Object.defineProperty(globalThis, "indexedDB", indexedDbDescriptor)
      else delete (globalThis as { indexedDB?: IDBFactory }).indexedDB
    }
  })

  test("rejects torn writes, metadata rollback, identity substitution, downgraded verification, and replacement Blobs", async () => {
    const indexedDbDescriptor = Object.getOwnPropertyDescriptor(globalThis, "indexedDB")
    const fake = new FakeIndexedDb()
    Object.defineProperty(globalThis, "indexedDB", { configurable: true, value: fake })
    try {
      const cache = await openDerivedObjectCache("verified-transaction-corruption")
      await cache.write(sha256, sha256, bytes)
      const objects = fake.databases.get("verified-transaction-corruption")!
      const inventory = fake.metadata.get("verified-transaction-corruption")!
      const record = objects.get(sha256)!
      const metadata = inventory.get(sha256)!
      const rejected = async (object: Record<string, unknown> | undefined, authority: Record<string, unknown> | undefined) => {
        if (object) objects.set(sha256, object)
        else objects.delete(sha256)
        if (authority) inventory.set(sha256, authority)
        else inventory.delete(sha256)
        await expect(cache.read(sha256)).rejects.toMatchObject({ code: "IntegrityFailure" })
      }

      await rejected(undefined, metadata)
      await rejected(record, undefined)
      await rejected(record, { ...metadata, storedAt: (record.storedAt as number) - 1 })
      await rejected(record, { ...metadata, transactionIdentity: crypto.randomUUID() })
      await rejected(record, { ...metadata, sha256: "0".repeat(64) })
      await rejected(record, { ...metadata, verificationVersion: 0 })
      await rejected({ ...record, verificationVersion: 0 }, metadata)
      await rejected({ ...record, bytes: new Blob([new TextEncoder().encode("immutablE")], { type: "application/octet-stream" }) }, metadata)
      await rejected({ ...record, bytes: new Blob([bytes], { type: `application/x-playsrc-verified;identity=${crypto.randomUUID()}` }) }, metadata)

      objects.set(sha256, record)
      inventory.set(sha256, metadata)
      expect(await cache.read(sha256)).toEqual({ bytes, sha256 })
      await expect(cache.write("0".repeat(64), "0".repeat(64), bytes)).rejects.toMatchObject({ code: "IntegrityFailure" })
      cache.close()
    } finally {
      if (indexedDbDescriptor) Object.defineProperty(globalThis, "indexedDB", indexedDbDescriptor)
      else delete (globalThis as { indexedDB?: IDBFactory }).indexedDB
    }
  })

  test("admits immutable objects under one bounded LRU inventory without rewriting retained Blobs", async () => {
    const indexedDbDescriptor = Object.getOwnPropertyDescriptor(globalThis, "indexedDB")
    const fake = new FakeIndexedDb()
    Object.defineProperty(globalThis, "indexedDB", { configurable: true, value: fake })
    const now = jest.spyOn(Date, "now")
    try {
      const cache = await openDerivedObjectCache("bounded-immutable-eviction", { maximumBytes: 12, maximumRecords: 2 })
      const first = new TextEncoder().encode("first"), second = new TextEncoder().encode("second"), third = new TextEncoder().encode("third")
      const firstHash = await digest(first), secondHash = await digest(second), thirdHash = await digest(third)
      now.mockReturnValue(10)
      await cache.write(firstHash, firstHash, first)
      now.mockReturnValue(20)
      await cache.write(secondHash, secondHash, second)
      const retainedBlob = fake.databases.get("bounded-immutable-eviction")!.get(firstHash)!.bytes
      now.mockReturnValue(30)
      expect((await cache.read(firstHash))?.bytes).toEqual(first)
      now.mockReturnValue(40)
      await cache.write(thirdHash, thirdHash, third)
      expect(await cache.read(secondHash)).toBeUndefined()
      expect((await cache.read(firstHash))?.bytes).toEqual(first)
      expect((await cache.read(thirdHash))?.bytes).toEqual(third)
      expect(fake.databases.get("bounded-immutable-eviction")!.get(firstHash)!.bytes).toBe(retainedBlob)
      expect(fake.inventories.objects).toBe(0)
      cache.close()
    } finally {
      now.mockRestore()
      if (indexedDbDescriptor) Object.defineProperty(globalThis, "indexedDB", indexedDbDescriptor)
      else delete (globalThis as { indexedDB?: IDBFactory }).indexedDB
    }
  })

  test("shares one persistent immutable admission while independently cancelling concurrent subscribers", async () => {
    const indexedDbDescriptor = Object.getOwnPropertyDescriptor(globalThis, "indexedDB")
    const fake = new FakeIndexedDb()
    Object.defineProperty(globalThis, "indexedDB", { configurable: true, value: fake })
    const url = `http://127.0.0.1:4321/objects/sha256/${sha256}`
    let requests = 0
    let release!: () => void
    const blocked = new Promise<void>((resolve) => { release = resolve })
    const fetcher = (async () => {
      requests += 1
      await blocked
      const response = new Response(bytes, { headers: { "content-length": String(bytes.byteLength), etag: `"${sha256}"` } })
      Object.defineProperty(response, "url", { value: url })
      return response
    }) as typeof fetch
    try {
      const cache = await openDerivedObjectCache("concurrent-persistent-immutable")
      const acquire = createImmutableObjectAcquirer({ fetcher, cache: async () => cache })
      const controller = new AbortController()
      const cancelled = acquire("http://127.0.0.1:4321/", descriptor, { signal: controller.signal })
      const retained = acquire("http://127.0.0.1:4321/", descriptor)
      controller.abort()
      await expect(cancelled).rejects.toMatchObject({ code: "Cancelled" })
      release()
      expect(await retained).toEqual(bytes)
      expect(requests).toBe(1)
      expect(fake.writes.objects).toBe(1)
      expect(await cache.read(sha256)).toEqual({ bytes, sha256 })
      cache.close()
    } finally {
      if (indexedDbDescriptor) Object.defineProperty(globalThis, "indexedDB", indexedDbDescriptor)
      else delete (globalThis as { indexedDB?: IDBFactory }).indexedDB
    }
  })

  test("shares one authenticated immutable transfer, verification, and progress between simultaneous consumers", async () => {
    const url = `http://127.0.0.1:4321/objects/sha256/${sha256}`
    let requests = 0
    let release!: () => void
    const blocked = new Promise<void>((resolve) => { release = resolve })
    const fetcher = (async () => {
      requests += 1
      await blocked
      const response = new Response(bytes, { headers: { "content-length": String(bytes.byteLength), etag: `"${sha256}"` } })
      Object.defineProperty(response, "url", { value: url })
      return response
    }) as typeof fetch
    const acquire = createImmutableObjectAcquirer({ concurrency: 2, fetcher })
    const firstProgress: number[] = [], secondProgress: number[] = []
    const first = acquire("http://127.0.0.1:4321/", descriptor, { onProgress: (loaded) => firstProgress.push(loaded) })
    const second = acquire("http://127.0.0.1:4321/", descriptor, { onProgress: (loaded) => secondProgress.push(loaded) })
    await Promise.resolve()
    expect(requests).toBe(1)
    release()
    const results = await Promise.all([first, second])
    expect(results[0]).toBe(results[1])
    expect(firstProgress).toEqual([0, bytes.byteLength])
    expect(secondProgress).toEqual([0, bytes.byteLength])
    expect(requests).toBe(1)
  })

  test("bounds immutable transfer concurrency and starts queued critical objects before background chunks", async () => {
    const values = await Promise.all(["first", "background", "critical"].map(async (text) => {
      const value = new TextEncoder().encode(text)
      return { value, descriptor: { ...descriptor, byteLength: String(value.byteLength), sha256: await digest(value) } }
    }))
    let active = 0, maximum = 0
    const started: string[] = []
    const releases: Array<() => void> = []
    const fetcher = (async (input: RequestInfo | URL) => {
      const url = String(input)
      const entry = values.find((candidate) => url.endsWith(candidate.descriptor.sha256))!
      started.push(new TextDecoder().decode(entry.value))
      maximum = Math.max(maximum, ++active)
      await new Promise<void>((resolve) => releases.push(resolve))
      active -= 1
      const response = new Response(entry.value, { headers: { "content-length": String(entry.value.byteLength), etag: `"${entry.descriptor.sha256}"` } })
      Object.defineProperty(response, "url", { value: url })
      return response
    }) as typeof fetch
    const acquire = createImmutableObjectAcquirer({ concurrency: 1, fetcher })
    const first = acquire("http://127.0.0.1:4321/", values[0]!.descriptor)
    const background = acquire("http://127.0.0.1:4321/", values[1]!.descriptor, { priority: "background" })
    const critical = acquire("http://127.0.0.1:4321/", values[2]!.descriptor, { priority: "critical" })
    expect(started).toEqual(["first"])
    releases.shift()!()
    await first
    expect(started).toEqual(["first", "critical"])
    releases.shift()!()
    await critical
    expect(started).toEqual(["first", "critical", "background"])
    releases.shift()!()
    await background
    expect(maximum).toBe(1)
  })

  test("cancels one immutable subscriber without interrupting another authenticated consumer", async () => {
    const url = `http://127.0.0.1:4321/objects/sha256/${sha256}`
    let release!: () => void
    const blocked = new Promise<void>((resolve) => { release = resolve })
    let transferSignal: AbortSignal | undefined
    const fetcher = (async (_input: RequestInfo | URL, init?: RequestInit) => {
      transferSignal = init?.signal ?? undefined
      await blocked
      const response = new Response(bytes, { headers: { "content-length": String(bytes.byteLength), etag: `"${sha256}"` } })
      Object.defineProperty(response, "url", { value: url })
      return response
    }) as typeof fetch
    const acquire = createImmutableObjectAcquirer({ concurrency: 2, fetcher })
    const controller = new AbortController()
    const cancelled = acquire("http://127.0.0.1:4321/", descriptor, { signal: controller.signal })
    const retained = acquire("http://127.0.0.1:4321/", descriptor)
    controller.abort()
    await expect(cancelled).rejects.toMatchObject({ code: "Cancelled" })
    expect(transferSignal?.aborted).toBe(false)
    release()
    expect(await retained).toEqual(bytes)
  })

  test("accepts only the exact immutable response and bytes", async () => {
    const url = `http://127.0.0.1:4321/objects/sha256/${sha256}`
    const fetcher = (async () => {
      const response = new Response(bytes, {
        headers: { "content-length": String(bytes.byteLength), etag: `"${sha256}"` },
      })
      Object.defineProperty(response, "url", { value: url })
      return response
    }) as typeof fetch
    const progress: readonly number[][] = []
    expect(await fetchImmutableObject("http://127.0.0.1:4321/", descriptor, undefined, fetcher, (loaded, total) => (progress as number[][]).push([loaded, total]))).toEqual(bytes)
    expect(progress).toEqual([[0, bytes.byteLength], [bytes.byteLength, bytes.byteLength]])
    const providerEtagFetcher = (async () => {
      const response = new Response(bytes, {
        headers: { "content-length": String(bytes.byteLength), etag: '"provider-validator"' },
      })
      Object.defineProperty(response, "url", { value: url })
      return response
    }) as typeof fetch
    expect(await fetchImmutableObject("http://127.0.0.1:4321/", descriptor, undefined, providerEtagFetcher)).toEqual(bytes)
    await expect(
      fetchImmutableObject("http://example.com/", descriptor, undefined, fetcher),
    ).rejects.toMatchObject({ code: "MalformedIdentity" })
    const controller = new AbortController()
    controller.abort()
    await expect(
      fetchImmutableObject("http://127.0.0.1:4321/", descriptor, controller.signal, fetcher),
    ).rejects.toMatchObject({ code: "Cancelled" })

    const corruptFetcher = (async () => {
      const corrupt = new TextEncoder().encode("immutablE")
      const response = new Response(corrupt, {
        headers: { "content-length": String(corrupt.byteLength), etag: `"${sha256}"` },
      })
      Object.defineProperty(response, "url", { value: url })
      return response
    }) as typeof fetch
    await expect(fetchImmutableObject(
      "http://127.0.0.1:4321/",
      descriptor,
      undefined,
      corruptFetcher,
    )).rejects.toMatchObject({ code: "IntegrityFailure" })

    const missingEtagFetcher = (async () => {
      const response = new Response(bytes, { headers: { "content-length": String(bytes.byteLength) } })
      Object.defineProperty(response, "url", { value: url })
      return response
    }) as typeof fetch
    await expect(fetchImmutableObject(
      "http://127.0.0.1:4321/",
      descriptor,
      undefined,
      missingEtagFetcher,
    )).rejects.toMatchObject({ code: "ResponseFailure" })
  })

  test("cancels an active immutable body without returning partial bytes", async () => {
    const url = `http://127.0.0.1:4321/objects/sha256/${sha256}`
    const controller = new AbortController()
    let chunk = 0
    const body = new ReadableStream<Uint8Array>({
      pull(stream) {
        if (chunk++ === 0) {
          stream.enqueue(bytes.subarray(0, 2))
          return
        }
        controller.abort()
        stream.enqueue(bytes.subarray(2))
        stream.close()
      },
    })
    const fetcher = (async () => {
      const response = new Response(body, {
        headers: { "content-length": String(bytes.byteLength), etag: `"${sha256}"` },
      })
      Object.defineProperty(response, "url", { value: url })
      return response
    }) as typeof fetch
    await expect(fetchImmutableObject(
      "http://127.0.0.1:4321/",
      descriptor,
      controller.signal,
      fetcher,
    )).rejects.toMatchObject({ code: "Cancelled" })
  })

  test("rejects corrupt derived records without deleting or substituting bytes", async () => {
    const valid = { key: sha256, byteLength: bytes.byteLength, sha256, bytes: new Blob([bytes]), storedAt: 1 }
    expect(await verifyDerivedRecord(valid, sha256)).toEqual(bytes)
    const corrupt = { ...valid, bytes: new Blob([new TextEncoder().encode("immutablE")]) }
    await expect(verifyDerivedRecord(corrupt, sha256)).rejects.toBeInstanceOf(BrowserAssetError)
  })

  test("evicts oldest cache records within byte and count bounds", () => {
    const records = [
      { key: "1".repeat(64), byteLength: 40, storedAt: 1 },
      { key: "2".repeat(64), byteLength: 30, storedAt: 2 },
      { key: "3".repeat(64), byteLength: 20, storedAt: 3 },
    ]
    expect(planDerivedCacheEviction(records, { key: "4".repeat(64), byteLength: 50 }, 100, 4)).toEqual(["1".repeat(64)])
    expect(planDerivedCacheEviction(records, { key: "4".repeat(64), byteLength: 1 }, 100, 3)).toEqual(["1".repeat(64)])
    expect(() => planDerivedCacheEviction(records, { key: "4".repeat(64), byteLength: 101 }, 100, 4)).toThrow(BrowserAssetError)
  })

  test("refreshes verified read recency before the next bounded admission", async () => {
    const indexedDbDescriptor = Object.getOwnPropertyDescriptor(globalThis, "indexedDB")
    const fake = new FakeIndexedDb()
    Object.defineProperty(globalThis, "indexedDB", { configurable: true, value: fake })
    const now = jest.spyOn(Date, "now").mockReturnValue(100)
    const firstKey = "1".repeat(64), untouchedKey = "2".repeat(64), missingKey = "3".repeat(64)
    const firstBytes = new TextEncoder().encode("1111"), untouchedBytes = new TextEncoder().encode("2222")
    try {
      const cache = await openDerivedObjectCache("verified-read-recency")
      await cache.write(firstKey, await digest(firstBytes), firstBytes)
      await cache.write(untouchedKey, await digest(untouchedBytes), untouchedBytes)
      expect(await cache.read(firstKey)).toEqual({ bytes: firstBytes, sha256: await digest(firstBytes) })
      const records = fake.databases.get("verified-read-recency")!
      const metadata = fake.metadata.get("verified-read-recency")!
      expect(metadata.get(firstKey)?.storedAt).toBe(101)
      expect(records.get(firstKey)?.storedAt).toBe(100)
      expect(records.get(untouchedKey)?.storedAt).toBe(100)
      expect(await cache.read(firstKey)).toEqual({ bytes: firstBytes, sha256: await digest(firstBytes) })
      expect(metadata.get(firstKey)?.storedAt).toBe(102)
      expect(planDerivedCacheEviction(
        [...metadata.values()].map((record) => ({ key: record.key as string, byteLength: record.byteLength as number, storedAt: record.storedAt as number })),
        { key: "4".repeat(64), byteLength: 1 },
        8,
        3,
      )).toEqual([untouchedKey])

      expect(await cache.read(missingKey)).toBeUndefined()
      expect(metadata.get(firstKey)?.storedAt).toBe(102)
      const first = records.get(firstKey)!
      records.set(firstKey, { ...first, bytes: new Blob([new TextEncoder().encode("111X")]) })
      await expect(cache.read(firstKey)).rejects.toMatchObject({ code: "IntegrityFailure" })
      expect(metadata.get(firstKey)?.storedAt).toBe(102)
      cache.close()
    } finally {
      now.mockRestore()
      if (indexedDbDescriptor) Object.defineProperty(globalThis, "indexedDB", indexedDbDescriptor)
      else delete (globalThis as { indexedDB?: IDBFactory }).indexedDB
    }
  })

  test("never clones Blob-backed object inventories while admitting or refreshing exact records", async () => {
    const indexedDbDescriptor = Object.getOwnPropertyDescriptor(globalThis, "indexedDB")
    const fake = new FakeIndexedDb()
    Object.defineProperty(globalThis, "indexedDB", { configurable: true, value: fake })
    try {
      const cache = await openDerivedObjectCache("metadata-only-inventories")
      for (let index = 1; index <= 6; index += 1) {
        const key = index.toString(16).repeat(64)
        const value = new Uint8Array(128).fill(index)
        await cache.write(key, await digest(value), value)
      }
      const admissionInventories = fake.inventories.metadata
      const admissionObjectWrites = fake.writes.objects
      for (let index = 1; index <= 6; index += 1) {
        const expected = new Uint8Array(128).fill(index)
        expect(await cache.read(index.toString(16).repeat(64))).toEqual({ bytes: expected, sha256: await digest(expected) })
      }
      expect(fake.inventories.objects).toBe(0)
      expect(fake.inventories.metadata).toBe(admissionInventories)
      expect(fake.writes.objects).toBe(admissionObjectWrites)
      expect(fake.writes.metadata).toBe(admissionObjectWrites + 6)
      cache.close()
    } finally {
      if (indexedDbDescriptor) Object.defineProperty(globalThis, "indexedDB", indexedDbDescriptor)
      else delete (globalThis as { indexedDB?: IDBFactory }).indexedDB
    }
  })

  test("upgrades existing Blob-backed cache entries without invalidating their exact bytes", async () => {
    const indexedDbDescriptor = Object.getOwnPropertyDescriptor(globalThis, "indexedDB")
    const fake = new FakeIndexedDb()
    const key = "a".repeat(64)
    const value = new TextEncoder().encode("retained exact cache bytes")
    const blob = new Blob([value])
    fake.databases.set("preserved-version-one", new Map([[key, {
      key,
      byteLength: value.byteLength,
      sha256: await digest(value),
      bytes: blob,
      storedAt: 17,
    }]]))
    fake.versions.set("preserved-version-one", 1)
    Object.defineProperty(globalThis, "indexedDB", { configurable: true, value: fake })
    try {
      const cache = await openDerivedObjectCache("preserved-version-one")
      expect(fake.versions.get("preserved-version-one")).toBe(2)
      expect(fake.metadata.get("preserved-version-one")?.get(key)).toEqual({ key, byteLength: value.byteLength, storedAt: 17 })
      expect(await cache.read(key)).toEqual({ bytes: value, sha256: await digest(value) })
      expect(fake.databases.get("preserved-version-one")?.get(key)?.bytes).toBe(blob)
      expect(fake.inventories.objects).toBe(0)
      cache.close()
    } finally {
      if (indexedDbDescriptor) Object.defineProperty(globalThis, "indexedDB", indexedDbDescriptor)
      else delete (globalThis as { indexedDB?: IDBFactory }).indexedDB
    }
  })

  test("rejects mismatched recency metadata without rewriting its verified Blob", async () => {
    const indexedDbDescriptor = Object.getOwnPropertyDescriptor(globalThis, "indexedDB")
    const fake = new FakeIndexedDb()
    Object.defineProperty(globalThis, "indexedDB", { configurable: true, value: fake })
    try {
      const cache = await openDerivedObjectCache("malformed-recency-metadata")
      await cache.write(sha256, sha256, bytes)
      const stored = fake.databases.get("malformed-recency-metadata")!.get(sha256)!
      const writes = fake.writes.objects
      fake.metadata.get("malformed-recency-metadata")!.set(sha256, {
        key: sha256,
        byteLength: bytes.byteLength + 1,
        storedAt: stored.storedAt,
      })
      await expect(cache.read(sha256)).rejects.toMatchObject({
        code: "IntegrityFailure",
        message: "derived cache verified transaction identity differs",
      })
      expect(fake.writes.objects).toBe(writes)
      expect(fake.databases.get("malformed-recency-metadata")!.get(sha256)).toBe(stored)
      cache.close()
    } finally {
      if (indexedDbDescriptor) Object.defineProperty(globalThis, "indexedDB", indexedDbDescriptor)
      else delete (globalThis as { indexedDB?: IDBFactory }).indexedDB
    }
  })

  test("terminates an IndexedDB open request that never settles", async () => {
    const descriptor=Object.getOwnPropertyDescriptor(globalThis,"indexedDB")
    const request={} as IDBOpenDBRequest
    Object.defineProperty(globalThis,"indexedDB",{configurable:true,value:{open:()=>request}})
    jest.useFakeTimers()
    try{
      const pending=openDerivedObjectCache("never-settles")
      jest.advanceTimersByTime(30_000)
      await expect(pending).rejects.toMatchObject({code:"PersistenceUnavailable",message:"IndexedDB open timed out after 30000 ms"})
    }finally{
      jest.useRealTimers()
      if(descriptor)Object.defineProperty(globalThis,"indexedDB",descriptor)
      else delete (globalThis as {indexedDB?:IDBFactory}).indexedDB
    }
  })
})
