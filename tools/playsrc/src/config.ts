import { constants } from "node:fs"
import { access, readFile, realpath, stat } from "node:fs/promises"
import path from "node:path"
import { fileURLToPath } from "node:url"

const CONFIG_KEYS = ["assetDir", "sourceCacheDir", "tf2Dir"] as const

export type LocalConfig = Readonly<{
  tf2Dir: string
  sourceCacheDir: string
  assetDir: string
}>

export class ConfigurationError extends Error {
  constructor(
    readonly code:
      | "ConfigurationMissing"
      | "ConfigurationMalformed"
      | "ConfiguredRootUnavailable",
    message: string,
  ) {
    super(message)
    this.name = "ConfigurationError"
  }
}

export const repositoryRoot = fileURLToPath(new URL("../../../", import.meta.url))

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const isNested = (parent: string, child: string): boolean => {
  const relative = path.relative(parent, child)
  return relative !== "" && !relative.startsWith("..") && !path.isAbsolute(relative)
}

export async function loadLocalConfig(root = repositoryRoot): Promise<LocalConfig> {
  const configPath = path.join(root, "playsrc.local.json")
  let bytes: Buffer

  try {
    bytes = await readFile(configPath)
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      throw new ConfigurationError(
        "ConfigurationMissing",
        "playsrc.local.json is missing from the repository root",
      )
    }
    throw new ConfigurationError(
      "ConfiguredRootUnavailable",
      "playsrc.local.json could not be read",
    )
  }

  let parsed: unknown
  try {
    const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes)
    parsed = JSON.parse(text)
  } catch {
    throw new ConfigurationError(
      "ConfigurationMalformed",
      "playsrc.local.json must be valid UTF-8 JSON",
    )
  }

  if (!isRecord(parsed) || Object.keys(parsed).sort().join("\0") !== CONFIG_KEYS.join("\0")) {
    throw new ConfigurationError(
      "ConfigurationMalformed",
      `playsrc.local.json must contain exactly ${CONFIG_KEYS.join(", ")}`,
    )
  }

  for (const key of CONFIG_KEYS) {
    const value = parsed[key]
    if (typeof value !== "string" || value.length === 0 || !path.isAbsolute(value)) {
      throw new ConfigurationError(
        "ConfigurationMalformed",
        `${key} must be a non-empty absolute path`,
      )
    }
  }

  const resolved = {} as Record<(typeof CONFIG_KEYS)[number], string>
  for (const key of CONFIG_KEYS) {
    const value = parsed[key] as string
    try {
      const rootStat = await stat(value)
      if (!rootStat.isDirectory()) throw new Error("not a directory")
      await access(value, key === "tf2Dir" ? constants.R_OK : constants.R_OK | constants.W_OK)
      resolved[key] = await realpath(value)
    } catch {
      throw new ConfigurationError(
        "ConfiguredRootUnavailable",
        `${key} must identify an accessible directory`,
      )
    }
  }

  for (let left = 0; left < CONFIG_KEYS.length; left += 1) {
    for (let right = left + 1; right < CONFIG_KEYS.length; right += 1) {
      const leftKey = CONFIG_KEYS[left]!
      const rightKey = CONFIG_KEYS[right]!
      const leftPath = resolved[leftKey]
      const rightPath = resolved[rightKey]
      if (leftPath === rightPath || isNested(leftPath, rightPath) || isNested(rightPath, leftPath)) {
        throw new ConfigurationError(
          "ConfigurationMalformed",
          `${leftKey} and ${rightKey} must be distinct, non-nested directories`,
        )
      }
    }
  }

  return Object.freeze({
    tf2Dir: resolved.tf2Dir,
    sourceCacheDir: resolved.sourceCacheDir,
    assetDir: resolved.assetDir,
  })
}
