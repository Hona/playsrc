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

  constructor(readonly records: Map<string, Record<string, unknown>>) {}

  objectStore(): IDBObjectStore {
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
      get: (key: string) => request(() => this.records.get(key)),
      getAll: () => request(() => [...this.records.values()]),
      add: (record: Record<string, unknown>) => request(() => {
        const key = record.key as string
        if (this.records.has(key)) throw new Error("ConstraintError")
        this.records.set(key, record)
        return key
      }),
      put: (record: Record<string, unknown>) => request(() => {
        const key = record.key as string
        this.records.set(key, record)
        return key
      }),
      delete: (key: string) => request(() => { this.records.delete(key); return undefined }),
    } as unknown as IDBObjectStore
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

  open(name: string): IDBOpenDBRequest {
    const request: FakeRequest<IDBDatabase> & { onupgradeneeded: ((event: IDBVersionChangeEvent) => void) | null } = {
      result: undefined as unknown as IDBDatabase,
      error: null,
      onsuccess: null,
      onerror: null,
      onupgradeneeded: null,
    }
    queueMicrotask(() => {
      const created = !this.databases.has(name)
      const records = this.databases.get(name) ?? new Map<string, Record<string, unknown>>()
      this.databases.set(name, records)
      request.result = {
        createObjectStore: () => ({} as IDBObjectStore),
        transaction: () => new FakeTransaction(records) as unknown as IDBTransaction,
        close: () => {},
        onversionchange: null,
      } as unknown as IDBDatabase
      if (created) request.onupgradeneeded?.(new Event("upgradeneeded") as IDBVersionChangeEvent)
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
      expect(await cache.read(firstKey)).toEqual(firstBytes)
      const records = fake.databases.get("verified-read-recency")!
      expect(records.get(firstKey)?.storedAt).toBe(101)
      expect(records.get(untouchedKey)?.storedAt).toBe(100)
      expect(await cache.read(firstKey)).toEqual(firstBytes)
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
