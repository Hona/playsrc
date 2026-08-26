import { describe, expect, jest, test } from "bun:test"
import { BrowserAssetError, fetchImmutableObject, openDerivedObjectCache, planDerivedCacheEviction, verifyDerivedRecord } from "../src/browser"
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
        records.set(key, record)
        return key
      }),
      put: (record: Record<string, unknown>) => request(() => {
        const key = record.key as string
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
      const upgrade = new FakeTransaction(stores, this.inventories)
      request.result = {
        createObjectStore: (storeName: string) => {
          if (storeName === "objects") return upgrade.objectStore("objects")
          const entries = new Map<string, Record<string, unknown>>()
          stores.set(storeName, entries)
          if (storeName === "metadata") this.metadata.set(name, entries)
          return upgrade.objectStore(storeName)
        },
        transaction: () => new FakeTransaction(stores, this.inventories) as unknown as IDBTransaction,
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
      expect(records.get(firstKey)?.storedAt).toBe(101)
      expect(records.get(untouchedKey)?.storedAt).toBe(100)
      expect(await cache.read(firstKey)).toEqual({ bytes: firstBytes, sha256: await digest(firstBytes) })
      expect(records.get(firstKey)?.storedAt).toBe(102)
      expect(planDerivedCacheEviction(
        [...records.values()].map((record) => ({ key: record.key as string, byteLength: record.byteLength as number, storedAt: record.storedAt as number })),
        { key: "4".repeat(64), byteLength: 1 },
        8,
        3,
      )).toEqual([untouchedKey])

      expect(await cache.read(missingKey)).toBeUndefined()
      expect(records.get(firstKey)?.storedAt).toBe(102)
      const first = records.get(firstKey)!
      records.set(firstKey, { ...first, bytes: new Blob([new TextEncoder().encode("111X")]) })
      await expect(cache.read(firstKey)).rejects.toMatchObject({ code: "IntegrityFailure" })
      expect(records.get(firstKey)?.storedAt).toBe(102)
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
      for (let index = 1; index <= 6; index += 1) {
        const expected = new Uint8Array(128).fill(index)
        expect(await cache.read(index.toString(16).repeat(64))).toEqual({ bytes: expected, sha256: await digest(expected) })
      }
      expect(fake.inventories.objects).toBe(0)
      expect(fake.inventories.metadata).toBeGreaterThan(0)
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
