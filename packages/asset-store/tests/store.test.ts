import { describe, expect, test } from "bun:test"
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import { AssetStoreError, canonicalJson, descriptor, objectPath, putObject, readChannel, readObject, writeChannel } from "../src/index"

describe("local immutable asset store", () => {
  test("stores, reuses, verifies and rejects corrupt immutable objects", async () => {
    const root = await mkdtemp(path.join(os.tmpdir(), "playsrc-assets-"))
    try {
      const bytes = new TextEncoder().encode("immutable")
      const expected = descriptor("source-object", "application/octet-stream", bytes)
      expect((await putObject(root, expected, bytes)).outcome).toBe("Stored")
      expect((await putObject(root, expected, bytes)).outcome).toBe("AlreadyPresent")
      expect(await readObject(root, expected)).toEqual(bytes)
      await writeFile(objectPath(root, expected.sha256), "corrupt")
      await expect(readObject(root, expected)).rejects.toMatchObject({ code: "IntegrityFailure" })
      expect(await readFile(objectPath(root, expected.sha256), "utf8")).toBe("corrupt")
    } finally { await rm(root, { recursive: true, force: true }) }
  })

  test("atomically writes and reads exact channel bytes", async () => {
    const root = await mkdtemp(path.join(os.tmpdir(), "playsrc-assets-"))
    try {
      const target = descriptor("application-root", "application/json", canonicalJson({ map: "jump_beef" }))
      const revision = await writeChannel(root, { channel: "local", target })
      const result = await readChannel(root, "local")
      expect(result.revision).toBe(revision)
      expect(result.record.target).toEqual(target)
      await expect(readChannel(root, "../bad")).rejects.toBeInstanceOf(AssetStoreError)
    } finally { await rm(root, { recursive: true, force: true }) }
  })
})
