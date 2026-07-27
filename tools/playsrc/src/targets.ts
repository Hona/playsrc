import { acquireDownload, type DownloadSource, type ObjectIdentity } from "@playsrc/content"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import maps from "../../../games/tf2/maps.json"
import type { LocalConfig } from "./config"

type MapTarget = Readonly<{
  logicalPath: string
  download?: DownloadSource
  installed?: Readonly<{ contentBuild: string; provider: string; byteLength: number; sha256: string }>
}>
type MapAcquisition = Readonly<{ logicalPath: string; decoded: ObjectIdentity }>

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
  const bytes = await readFile(path.join(config.tf2Dir, target.logicalPath))
  const sha256 = new Bun.CryptoHasher("sha256").update(bytes).digest("hex")
  if (bytes.byteLength !== installed.byteLength || sha256 !== installed.sha256) {
    throw new TargetError("TargetMalformed", `installed map target ${identity} differs from its declaration`)
  }
  const cachePath = path.join("objects", "sha256", sha256.slice(0, 2), sha256)
  const destination = path.join(config.sourceCacheDir, cachePath)
  await mkdir(path.dirname(destination), { recursive: true })
  await writeFile(destination, bytes)
  return Object.freeze({
    logicalPath: target.logicalPath,
    decoded: Object.freeze({ byteLength: bytes.byteLength, sha256, cachePath }),
  })
}
