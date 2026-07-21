import { describe, expect, test } from "bun:test"
import { CloudflareError, createR2Adapter, publishImmutableObject, sortPublicationDescriptors, type RemoteObjectAdapter } from "../src/cloudflare"
import type { ObjectDescriptor } from "@playsrc/asset-store"

describe("Cloudflare publication output", () => {
  const bytes = new TextEncoder().encode("object")
  const descriptor: ObjectDescriptor = Object.freeze({
    kind: "derived-object",
    mediaType: "application/octet-stream",
    byteLength: "6",
    sha256: new Bun.CryptoHasher("sha256").update(bytes).digest("hex"),
  })

  function adapter(initial: Uint8Array | "Missing", create: "Created" | "PreconditionFailed" = "Created", persistCreate = true) {
    let remote = initial
    const calls: string[] = []
    const value: RemoteObjectAdapter = {
      async read() { calls.push("read"); return remote },
      async create(_key, _expected, input) { calls.push("create"); if (persistCreate) remote = input; return create },
      close() { calls.push("close") },
    }
    return { value, calls }
  }

  test("uploads only a missing object and verifies exact readback", async () => {
    const remote = adapter("Missing")
    expect(await publishImmutableObject(descriptor, bytes, remote.value)).toBe("Uploaded")
    expect(remote.calls).toEqual(["read", "create", "read"])
  })

  test("performs zero writes for a warm exact object", async () => {
    const remote = adapter(bytes)
    expect(await publishImmutableObject(descriptor, bytes, remote.value)).toBe("AlreadyPresent")
    expect(remote.calls).toEqual(["read"])
  })

  test("accepts a concurrent conditional-create winner only after readback", async () => {
    const remote = adapter("Missing", "PreconditionFailed")
    expect(await publishImmutableObject(descriptor, bytes, remote.value)).toBe("AlreadyPresent")
    expect(remote.calls).toEqual(["read", "create", "read"])
  })

  test("rejects conflicts, missing readback, and absent credentials", async () => {
    const corrupt = adapter(new TextEncoder().encode("Object"))
    await expect(publishImmutableObject(descriptor, bytes, corrupt.value)).rejects.toBeInstanceOf(CloudflareError)
    const missing = adapter("Missing", "Created", false)
    await expect(publishImmutableObject(descriptor, bytes, missing.value)).rejects.toBeInstanceOf(CloudflareError)
    expect(() => createR2Adapter({})).toThrow("R2 conditional publication credentials are unavailable")
  })

  test("orders leaves before roots and catalogs deterministically", () => {
    const value = (kind: ObjectDescriptor["kind"], digit: string): ObjectDescriptor => ({ kind, mediaType: "x", byteLength: "1", sha256: digit.repeat(64) })
    expect(sortPublicationDescriptors([
      value("catalog", "1"),
      value("source-root", "2"),
      value("derived-object", "4"),
      value("source-object", "3"),
    ]).map((item) => item.kind)).toEqual(["source-object", "derived-object", "source-root", "catalog"])
  })
})
