import { acquireDownload, type DownloadProvenance, type DownloadSource } from "@playsrc/content"
import maps from "../../../games/tf2/maps.json"
import type { LocalConfig } from "./config"

type MapTarget = Readonly<{
  logicalPath: string
  download: DownloadSource
}>

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
): Promise<DownloadProvenance> {
  const target = resolveMapTarget(identity)
  return acquireDownload(config.sourceCacheDir, target.logicalPath, target.download)
}
