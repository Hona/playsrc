import { describe, expect, test } from "bun:test"
import { BrowserAssetError, fetchImmutableObject, verifyDerivedRecord } from "../src/browser"
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
    expect(await fetchImmutableObject("http://127.0.0.1:4321/", descriptor, undefined, fetcher)).toEqual(bytes)
    await expect(
      fetchImmutableObject("http://example.com/", descriptor, undefined, fetcher),
    ).rejects.toMatchObject({ code: "MalformedIdentity" })
    const controller = new AbortController()
    controller.abort()
    await expect(
      fetchImmutableObject("http://127.0.0.1:4321/", descriptor, controller.signal, fetcher),
    ).rejects.toMatchObject({ code: "Cancelled" })
  })

  test("rejects corrupt derived records without deleting or substituting bytes", async () => {
    const valid = { key: sha256, byteLength: bytes.byteLength, sha256, bytes: bytes.slice().buffer }
    expect(await verifyDerivedRecord(valid, sha256)).toEqual(bytes)
    const corrupt = { ...valid, bytes: new TextEncoder().encode("immutablE").buffer }
    await expect(verifyDerivedRecord(corrupt, sha256)).rejects.toBeInstanceOf(BrowserAssetError)
  })
})
