import { link, mkdir, readFile, rename, rm, stat, writeFile } from "node:fs/promises"
import path from "node:path"

export type ObjectKind = "source-object" | "derived-object" | "source-root" | "map-root" | "game-root" | "application-root" | "catalog"
export type ObjectDescriptor = Readonly<{ kind: ObjectKind; mediaType: string; byteLength: string; sha256: string }>
export type StoredObject = Readonly<{ descriptor: ObjectDescriptor; outcome: "Stored" | "AlreadyPresent" }>
export type ChannelRecord = Readonly<{ channel: string; target: ObjectDescriptor }>

export class AssetStoreError extends Error {
  constructor(readonly code: "MalformedIdentity" | "MissingObject" | "IntegrityFailure" | "Cancelled" | "IoFailure", message: string) {
    super(message)
    this.name = "AssetStoreError"
  }
}

function throwIfCancelled(signal?: AbortSignal): void {
  if (signal?.aborted) throw new AssetStoreError("Cancelled", "object publication was cancelled")
}

const HASH = /^[0-9a-f]{64}$/
const CHANNEL = /^[a-z0-9](?:[a-z0-9._-]*[a-z0-9])?$/

function digest(bytes: Uint8Array): string {
  const hash = new Bun.CryptoHasher("sha256")
  hash.update(bytes)
  return hash.digest("hex")
}

export function objectPath(root: string, sha256: string): string {
  if (!HASH.test(sha256)) throw new AssetStoreError("MalformedIdentity", "object hash is not canonical")
  return path.join(root, "objects", "sha256", sha256.slice(0, 2), sha256)
}

export function descriptor(kind: ObjectKind, mediaType: string, bytes: Uint8Array): ObjectDescriptor {
  if (!mediaType || /[\r\n\0]/.test(mediaType)) throw new AssetStoreError("MalformedIdentity", "media type is malformed")
  return Object.freeze({ kind, mediaType, byteLength: String(bytes.byteLength), sha256: digest(bytes) })
}

async function verify(pathname: string, expected: ObjectDescriptor): Promise<Uint8Array> {
  try {
    const metadata = await stat(pathname)
    if (!metadata.isFile() || String(metadata.size) !== expected.byteLength) throw new AssetStoreError("IntegrityFailure", "object length differs")
    const bytes = await readFile(pathname)
    if (digest(bytes) !== expected.sha256) throw new AssetStoreError("IntegrityFailure", "object hash differs")
    return bytes
  } catch (error) {
    if (error instanceof AssetStoreError) throw error
    if ((error as NodeJS.ErrnoException).code === "ENOENT") throw new AssetStoreError("MissingObject", "object is absent")
    throw new AssetStoreError("IoFailure", "object read failed")
  }
}

export async function putObject(
  root: string,
  expected: ObjectDescriptor,
  bytes: Uint8Array,
  signal?: AbortSignal,
): Promise<StoredObject> {
  throwIfCancelled(signal)
  if (String(bytes.byteLength) !== expected.byteLength || digest(bytes) !== expected.sha256) throw new AssetStoreError("IntegrityFailure", "input differs from descriptor")
  const pathname = objectPath(root, expected.sha256)
  try {
    await verify(pathname, expected)
    return Object.freeze({ descriptor: expected, outcome: "AlreadyPresent" })
  } catch (error) {
    if (!(error instanceof AssetStoreError) || error.code !== "MissingObject") throw error
  }
  throwIfCancelled(signal)
  await mkdir(path.dirname(pathname), { recursive: true })
  const temporary = `${pathname}.${process.pid}.tmp`
  try {
    await rm(temporary, { force: true })
    await writeFile(temporary, bytes, { flag: "wx", signal })
    throwIfCancelled(signal)
    await link(temporary, pathname)
    await rm(temporary)
    await verify(pathname, expected)
    return Object.freeze({ descriptor: expected, outcome: "Stored" })
  } catch (error) {
    await rm(temporary, { force: true })
    if (signal?.aborted) throw new AssetStoreError("Cancelled", "object publication was cancelled")
    if ((error as NodeJS.ErrnoException).code === "EEXIST") {
      await verify(pathname, expected)
      return Object.freeze({ descriptor: expected, outcome: "AlreadyPresent" })
    }
    if (error instanceof AssetStoreError) throw error
    throw new AssetStoreError("IoFailure", "object installation failed")
  }
}

export async function readObject(root: string, expected: ObjectDescriptor): Promise<Uint8Array> {
  return verify(objectPath(root, expected.sha256), expected)
}

export function canonicalJson(value: unknown): Uint8Array {
  return new TextEncoder().encode(JSON.stringify(value))
}

export async function writeChannel(root: string, record: ChannelRecord): Promise<string> {
  if (!CHANNEL.test(record.channel) || !HASH.test(record.target.sha256)) throw new AssetStoreError("MalformedIdentity", "channel record is malformed")
  const bytes = canonicalJson(record)
  const revision = digest(bytes)
  const pathname = path.join(root, "channels", `${record.channel}.json`)
  await mkdir(path.dirname(pathname), { recursive: true })
  const temporary = `${pathname}.${process.pid}.tmp`
  try {
    await writeFile(temporary, bytes)
    await rename(temporary, pathname)
    return revision
  } catch {
    await rm(temporary, { force: true })
    throw new AssetStoreError("IoFailure", "channel write failed")
  }
}

export async function readChannel(root: string, channel: string): Promise<Readonly<{ bytes: Uint8Array; revision: string; record: ChannelRecord }>> {
  if (!CHANNEL.test(channel)) throw new AssetStoreError("MalformedIdentity", "channel name is malformed")
  try {
    const bytes = await readFile(path.join(root, "channels", `${channel}.json`))
    const record = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)) as ChannelRecord
    if (record.channel !== channel || !HASH.test(record.target?.sha256)) throw new Error("shape")
    return Object.freeze({ bytes, revision: digest(bytes), record: Object.freeze(record) })
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") throw new AssetStoreError("MissingObject", "channel is absent")
    if (error instanceof AssetStoreError) throw error
    throw new AssetStoreError("IntegrityFailure", "channel record is malformed")
  }
}
