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
