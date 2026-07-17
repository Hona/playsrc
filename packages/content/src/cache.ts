import { link, mkdir, readFile, rm, stat, writeFile } from "node:fs/promises"
import path from "node:path"
import Bunzip from "seek-bzip"

export type DownloadSource = Readonly<{
  url: string
  compression: "bzip2"
  encodedByteLength: number
  encodedSha256: string
  decodedByteLength: number
  decodedSha256: string
}>

export type ObjectIdentity = Readonly<{
  byteLength: number
  sha256: string
  cachePath: string
}>

export type DownloadProvenance = Readonly<{
  logicalPath: string
  sourceUrl: string
  encoded: ObjectIdentity
  decoded: ObjectIdentity
}>

export type AcquireDownloadOptions = Readonly<{
  signal?: AbortSignal
  fetchSource?: typeof fetch
}>

export class ContentCacheError extends Error {
  constructor(
    readonly code:
      | "MalformedSource"
      | "DownloadFailed"
      | "IntegrityFailure"
      | "DecompressionFailed"
      | "Cancelled"
      | "IoFailure",
    message: string,
  ) {
    super(message)
    this.name = "ContentCacheError"
  }
}

const SHA256 = /^[0-9a-f]{64}$/
const MAX_SOURCE_BYTES = 512 * 1024 * 1024

function digest(bytes: Uint8Array): string {
  const hash = new Bun.CryptoHasher("sha256")
  hash.update(bytes)
  return hash.digest("hex")
}

function objectPath(root: string, sha256: string): string {
  return path.join(root, "objects", "sha256", sha256.slice(0, 2), sha256)
}

function relativeObjectPath(sha256: string): string {
  return `objects/sha256/${sha256.slice(0, 2)}/${sha256}`
}

function throwIfCancelled(signal?: AbortSignal): void {
  if (signal?.aborted) throw new ContentCacheError("Cancelled", "content acquisition was cancelled")
}

async function readObject(pathname: string, expectedLength: number, expectedHash: string) {
  try {
    const metadata = await stat(pathname)
    if (!metadata.isFile() || metadata.size !== expectedLength) {
      throw new ContentCacheError("IntegrityFailure", "cached object byte length differs")
    }
    const bytes = await readFile(pathname)
    if (digest(bytes) !== expectedHash) {
      throw new ContentCacheError("IntegrityFailure", "cached object SHA-256 differs")
    }
    return bytes
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return null
    if (error instanceof ContentCacheError) throw error
    throw new ContentCacheError("IoFailure", "cached object could not be read")
  }
}

async function installObject(
  pathname: string,
  bytes: Uint8Array,
  expectedLength: number,
  expectedHash: string,
): Promise<void> {
  await mkdir(path.dirname(pathname), { recursive: true })
  const temporary = `${pathname}.${process.pid}.tmp`
  try {
    await rm(temporary, { force: true })
    await writeFile(temporary, bytes, { flag: "wx" })
    await link(temporary, pathname)
    await rm(temporary)
  } catch (error) {
    await rm(temporary, { force: true })
    if ((error as NodeJS.ErrnoException).code === "EEXIST") {
      await readObject(pathname, expectedLength, expectedHash)
      return
    }
    throw new ContentCacheError("IoFailure", "cache object could not be installed")
  }
}

function validate(logicalPath: string, source: DownloadSource): void {
  let sourceUrl: URL | undefined
  try {
    sourceUrl = new URL(source.url)
  } catch {
    // The shared malformed-source result below owns URL syntax failures.
  }
  if (
    !/^maps\/[a-z0-9_./-]+\.bsp$/.test(logicalPath) ||
    logicalPath.includes("\\") ||
    logicalPath.split("/").some((component) => component === "" || component === "." || component === "..") ||
    sourceUrl?.protocol !== "https:" ||
    sourceUrl.username !== "" ||
    sourceUrl.password !== "" ||
    sourceUrl.hash !== "" ||
    source.compression !== "bzip2" ||
    !Number.isSafeInteger(source.encodedByteLength) ||
    !Number.isSafeInteger(source.decodedByteLength) ||
    source.encodedByteLength < 1 ||
    source.decodedByteLength < 1 ||
    source.encodedByteLength > MAX_SOURCE_BYTES ||
    source.decodedByteLength > MAX_SOURCE_BYTES ||
    !SHA256.test(source.encodedSha256) ||
    !SHA256.test(source.decodedSha256)
  ) {
    throw new ContentCacheError("MalformedSource", "download source is not canonical and bounded")
  }
}

async function readResponse(
  response: Response,
  expectedLength: number,
  signal?: AbortSignal,
): Promise<Buffer> {
  if (!response.body) throw new ContentCacheError("DownloadFailed", "download response has no body")
  const output = Buffer.allocUnsafe(expectedLength)
  const reader = response.body.getReader()
  let offset = 0
  try {
    while (true) {
      if (signal?.aborted) {
        await reader.cancel()
        throw new ContentCacheError("Cancelled", "content acquisition was cancelled")
      }
      const { done, value } = await reader.read()
      if (done) break
      if (offset + value.byteLength > expectedLength) {
        await reader.cancel()
        throw new ContentCacheError("IntegrityFailure", "download body exceeds its declared length")
      }
      output.set(value, offset)
      offset += value.byteLength
    }
  } catch (error) {
    if (error instanceof ContentCacheError) throw error
    if (signal?.aborted) throw new ContentCacheError("Cancelled", "content acquisition was cancelled")
    throw new ContentCacheError("DownloadFailed", "download body could not be read")
  }
  if (offset !== expectedLength) {
    throw new ContentCacheError("IntegrityFailure", "download body is shorter than its declared length")
  }
  return output
}

export async function acquireDownload(
  sourceCacheDir: string,
  logicalPath: string,
  source: DownloadSource,
  options: AcquireDownloadOptions = {},
): Promise<DownloadProvenance> {
  validate(logicalPath, source)
  throwIfCancelled(options.signal)
  const fetchSource = options.fetchSource ?? fetch
  const encodedPath = objectPath(sourceCacheDir, source.encodedSha256)
  let encoded = await readObject(encodedPath, source.encodedByteLength, source.encodedSha256)
  const encodedWasMissing = !encoded
  if (!encoded) {
    let response: Response
    try {
      response = await fetchSource(source.url, { redirect: "error", signal: options.signal })
    } catch {
      if (options.signal?.aborted) {
        throw new ContentCacheError("Cancelled", "content acquisition was cancelled")
      }
      throw new ContentCacheError("DownloadFailed", "download request failed")
    }
    if (!response.ok || response.url !== source.url) {
      throw new ContentCacheError("DownloadFailed", `download returned HTTP ${response.status}`)
    }
    const declaredLength = Number(response.headers.get("content-length"))
    if (declaredLength !== source.encodedByteLength) {
      throw new ContentCacheError("IntegrityFailure", "download Content-Length differs")
    }
    const bytes = await readResponse(response, source.encodedByteLength, options.signal)
    if (bytes.byteLength !== source.encodedByteLength || digest(bytes) !== source.encodedSha256) {
      throw new ContentCacheError("IntegrityFailure", "download bytes differ from the declared identity")
    }
    encoded = bytes
  }

  throwIfCancelled(options.signal)
  const decodedPath = objectPath(sourceCacheDir, source.decodedSha256)
  let decoded = await readObject(decodedPath, source.decodedByteLength, source.decodedSha256)
  const decodedWasMissing = !decoded
  if (!decoded) {
    try {
      decoded = Bunzip.decode(encoded, source.decodedByteLength)
    } catch {
      throw new ContentCacheError("DecompressionFailed", "bzip2 decoding failed")
    }
    if (decoded.byteLength !== source.decodedByteLength || digest(decoded) !== source.decodedSha256) {
      throw new ContentCacheError("IntegrityFailure", "decoded bytes differ from the declared identity")
    }
  }

  // Cancellation is observed before this atomic commit boundary. A signal
  // received after the check does not relabel successfully installed objects.
  throwIfCancelled(options.signal)
  if (encodedWasMissing) {
    await installObject(encodedPath, encoded, source.encodedByteLength, source.encodedSha256)
  }
  if (decodedWasMissing) {
    await installObject(decodedPath, decoded, source.decodedByteLength, source.decodedSha256)
  }

  return Object.freeze({
    logicalPath,
    sourceUrl: source.url,
    encoded: Object.freeze({
      byteLength: source.encodedByteLength,
      sha256: source.encodedSha256,
      cachePath: relativeObjectPath(source.encodedSha256),
    }),
    decoded: Object.freeze({
      byteLength: source.decodedByteLength,
      sha256: source.decodedSha256,
      cachePath: relativeObjectPath(source.decodedSha256),
    }),
  })
}
