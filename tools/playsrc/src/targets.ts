import { acquireDownload, type DownloadSource, type ObjectIdentity } from "@playsrc/content"
import { mkdir, readFile, rename, rm, stat, writeFile } from "node:fs/promises"
import path from "node:path"
import maps from "../../../games/tf2/maps.json"
import type { LocalConfig } from "./config"

type MapTarget = Readonly<{
  logicalPath: string
  download?: DownloadSource
  installed?: Readonly<{ contentBuild: string; provider: string; byteLength: number; sha256: string }>
}>
type MapAcquisition = Readonly<{ logicalPath: string; decoded: ObjectIdentity }>

type InstalledMapStamp = Readonly<{
  schema: "playsrc-installed-map-stamp-v1"
  source: string
  sha256: string
  byteLength: number
  sourceIdentity: string
  cachedIdentity: string
}>

const fileIdentity = (value: Awaited<ReturnType<typeof stat>>): string =>
  `${value.dev}:${value.ino}:${value.size}:${value.mtimeMs}:${value.ctimeMs}`

export class TargetError extends Error {
  constructor(readonly code: "TargetMalformed" | "TargetMissing", message: string) {
    super(message)
    this.name = "TargetError"
  }
}

export function resolveMapTarget(identity: string | undefined): MapTarget {
  if (!identity || !/^[a-z0-9_]+$/.test(identity)) {
    throw new TargetError("TargetMalformed", "map target must be one canonical logical identity")
  }
  const target = (maps as Record<string, MapTarget>)[identity]
  if (!target) throw new TargetError("TargetMissing", `map target ${identity} is not declared`)
  return Object.freeze(target)
}

export async function acquireMap(
  config: LocalConfig,
  identity: string | undefined,
): Promise<MapAcquisition> {
  const target = resolveMapTarget(identity)
  if ((target.download === undefined) === (target.installed === undefined)) {
    throw new TargetError("TargetMalformed", `map target ${identity} must declare one source`)
  }
  if (target.download) {
    const acquired = await acquireDownload(config.sourceCacheDir, target.logicalPath, target.download)
    return Object.freeze({ logicalPath: acquired.logicalPath, decoded: acquired.decoded })
  }
  const installed = target.installed!
  const source = path.join(config.tf2Dir, target.logicalPath)
  const cachePath = path.join("objects", "sha256", installed.sha256.slice(0, 2), installed.sha256)
  const destination = path.join(config.sourceCacheDir, cachePath)
  const stampPath = path.join(config.sourceCacheDir, "prepared-content", "maps", `${installed.sha256}.json`)
  try {
    const [sourceMetadata, cachedMetadata, stampText] = await Promise.all([stat(source), stat(destination), readFile(stampPath, "utf8")])
    const stamp = JSON.parse(stampText) as InstalledMapStamp
    if (sourceMetadata.isFile() && cachedMetadata.isFile()
      && stamp.schema === "playsrc-installed-map-stamp-v1" && stamp.source === source
      && stamp.sha256 === installed.sha256 && stamp.byteLength === installed.byteLength
      && stamp.sourceIdentity === fileIdentity(sourceMetadata) && stamp.cachedIdentity === fileIdentity(cachedMetadata)) {
      return Object.freeze({ logicalPath: target.logicalPath, decoded: Object.freeze({ byteLength: installed.byteLength, sha256: installed.sha256, cachePath }) })
    }
  } catch (error) {
    if (!["ENOENT", "ENOTDIR"].includes((error as NodeJS.ErrnoException).code ?? "") && !(error instanceof SyntaxError)) throw error
  }
  const bytes = await readFile(source)
  const sha256 = new Bun.CryptoHasher("sha256").update(bytes).digest("hex")
  if (bytes.byteLength !== installed.byteLength || sha256 !== installed.sha256) {
    throw new TargetError("TargetMalformed", `installed map target ${identity} differs from its declaration`)
  }
  await mkdir(path.dirname(destination), { recursive: true })
  try {
    const cached = await stat(destination)
    if (!cached.isFile() || cached.size !== bytes.byteLength
      || new Bun.CryptoHasher("sha256").update(await readFile(destination)).digest("hex") !== sha256) {
      await writeFile(destination, bytes)
    }
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error
    const temporary = `${destination}.${process.pid}.${crypto.randomUUID()}.tmp`
    try {
      await writeFile(temporary, bytes)
      await rename(temporary, destination)
    } finally {
      await rm(temporary, { force: true })
    }
  }
  const [sourceMetadata, cachedMetadata] = await Promise.all([stat(source), stat(destination)])
  const stamp: InstalledMapStamp = Object.freeze({
    schema: "playsrc-installed-map-stamp-v1",
    source,
    sha256,
    byteLength: bytes.byteLength,
    sourceIdentity: fileIdentity(sourceMetadata),
    cachedIdentity: fileIdentity(cachedMetadata),
  })
  await mkdir(path.dirname(stampPath), { recursive: true })
  const temporaryStamp = `${stampPath}.${process.pid}.${crypto.randomUUID()}.tmp`
  try {
    await writeFile(temporaryStamp, `${JSON.stringify(stamp)}\n`)
    await rename(temporaryStamp, stampPath)
  } finally {
    await rm(temporaryStamp, { force: true })
  }
  return Object.freeze({
    logicalPath: target.logicalPath,
    decoded: Object.freeze({ byteLength: bytes.byteLength, sha256, cachePath }),
  })
}
