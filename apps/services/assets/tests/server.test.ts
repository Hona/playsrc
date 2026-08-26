import { describe, expect, test } from "bun:test"
import { mkdtemp, rm } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import { descriptor, putObject, writeChannel } from "@playsrc/asset-store"
import { assetResponse } from "../src/server"

describe("asset service", () => {
  test("serves exact whole, HEAD, conditional, and ranged immutable bytes", async () => {
    const root = await mkdtemp(path.join(os.tmpdir(), "playsrc-service-"))
    try {
      const bytes = new TextEncoder().encode("0123456789")
      const object = descriptor("source-object", "application/octet-stream", bytes)
      await putObject(root, object, bytes)
      const url = `http://localhost/objects/sha256/${object.sha256}`
      const whole = await assetResponse(new Request(url), root)
      expect(whole.status).toBe(200); expect(await whole.text()).toBe("0123456789")
      expect(whole.headers.get("content-digest")).toBe(whole.headers.get("repr-digest"))
      const partial = await assetResponse(new Request(url, { headers: { range: "bytes=2-5" } }), root)
      expect(partial.status).toBe(206); expect(await partial.text()).toBe("2345"); expect(partial.headers.get("content-range")).toBe("bytes 2-5/10")
      expect(partial.headers.get("content-digest")).not.toBe(partial.headers.get("repr-digest"))
      const head = await assetResponse(new Request(url, { method: "HEAD" }), root)
      expect(head.headers.get("content-length")).toBe("10"); expect(await head.text()).toBe("")
      const cached = await assetResponse(new Request(url, { headers: { "if-none-match": `"${object.sha256}"` } }), root)
      expect(cached.status).toBe(304)
    } finally { await rm(root, { recursive: true, force: true }) }
  })

  test("serves exact channel records", async () => {
    const root = await mkdtemp(path.join(os.tmpdir(), "playsrc-service-"))
    try {
      const target = descriptor("application-root", "application/json", new TextEncoder().encode("{}"))
      const revision = await writeChannel(root, { channel: "local", target })
      const response = await assetResponse(new Request("http://localhost/channels/local"), root)
      expect(response.status).toBe(200); expect(response.headers.get("etag")).toBe(`"${revision}"`)
    } finally { await rm(root, { recursive: true, force: true }) }
  })
})
