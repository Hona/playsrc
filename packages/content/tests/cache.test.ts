import { afterEach, describe, expect, test } from "bun:test"
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import { acquireDownload, ContentCacheError, type DownloadSource } from "../src/cache"

const roots: string[] = []
const encoded = Buffer.from(
  "QlpoOTFBWSZTWSLUmeAAAAJBgAAQKARYICAAIYjJoQwIzZH1vi7kinChIEWpM8A=",
  "base64",
)
const source: DownloadSource = {
  url: "https://content.test/maps/example.bsp.bz2",
  compression: "bzip2",
  encodedByteLength: 47,
  encodedSha256: "97716a6c61e0fde29a5f30d365c9babc7c201a7215c39544535638809b608b6d",
  decodedByteLength: 8,
  decodedSha256: "847b83cd44274345a7d4c47b9a7f6a46a2f1e3fe52b596c69fccfcd57f064cf4",
}

afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { force: true, recursive: true })))
})

async function root(): Promise<string> {
  const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-content-"))
  roots.push(directory)
  return directory
}

function response(bytes = encoded, status = 200): Response {
  return new Response(bytes, {
    status,
    headers: { "content-length": String(bytes.byteLength) },
  })
}

describe("download source cache", () => {
  test("downloads, verifies, decompresses, and reuses immutable objects", async () => {
    const directory = await root()
    let requests = 0
    const fetchSource = (async (url: string | URL | Request) => {
      requests += 1
      const result = response()
      Object.defineProperty(result, "url", { value: String(url) })
      return result
    }) as typeof fetch

    const first = await acquireDownload(directory, "maps/example.bsp", source, fetchSource)
    const second = await acquireDownload(directory, "maps/example.bsp", source, fetchSource)
    expect(first).toEqual(second)
    expect(requests).toBe(1)
    expect(await readFile(path.join(directory, first.decoded.cachePath), "utf8")).toBe("playsrc\n")
  })

  test("fails without fallback on HTTP, length, hash, and cache corruption", async () => {
    const cases = [
      [response(encoded, 404), "DownloadFailed"],
      [response(encoded.subarray(0, 20)), "IntegrityFailure"],
      [response(Buffer.alloc(encoded.length)), "IntegrityFailure"],
    ] as const
    for (const [result, code] of cases) {
      const directory = await root()
      Object.defineProperty(result, "url", { value: source.url })
      try {
        await acquireDownload(directory, "maps/example.bsp", source, (async () => result) as typeof fetch)
        throw new Error("acquisition unexpectedly succeeded")
      } catch (error) {
        expect(error).toBeInstanceOf(ContentCacheError)
        expect((error as ContentCacheError).code).toBe(code)
      }
    }

    const directory = await root()
    const provenance = await acquireDownload(directory, "maps/example.bsp", source, (async () => {
      const result = response()
      Object.defineProperty(result, "url", { value: source.url })
      return result
    }) as typeof fetch)
    await writeFile(path.join(directory, provenance.decoded.cachePath), "corrupt!")
    await expect(
      acquireDownload(directory, "maps/example.bsp", source, (async () => {
        throw new Error("must not fetch")
      }) as typeof fetch),
    ).rejects.toMatchObject({ code: "IntegrityFailure" })
  })

  test("rejects malformed logical paths and source identities before fetch", async () => {
    let requested = false
    try {
      await acquireDownload(
        await root(),
        "../example.bsp",
        source,
        (async () => {
          requested = true
          return response()
        }) as typeof fetch,
      )
      throw new Error("acquisition unexpectedly succeeded")
    } catch (error) {
      expect(error).toMatchObject({ code: "MalformedSource" })
      expect(requested).toBe(false)
    }
  })
})
