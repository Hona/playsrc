import { mkdir, rm } from "node:fs/promises"
import path from "node:path"
import type { ObjectDescriptor } from "@playsrc/asset-store"
import type { LocalConfig } from "./config"
import { repositoryRoot } from "./config"
import { prepareTf2Release, releaseObjectPath, verifyFile } from "./tf2-release"

export const CLOUDFLARE_ASSET_BUCKET = "playsrc-production-assets"
export const CLOUDFLARE_ASSET_ORIGIN = "https://assets.playsrc.online"
export const WRANGLER_CONFIG = path.join(repositoryRoot, "apps", "web", "tf2", "wrangler.jsonc")
const MAX_WRANGLER_OBJECT_BYTES = 300_000_000
const WRANGLER_TIMEOUT_MILLISECONDS = 30 * 60 * 1_000

export class CloudflareError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "CloudflareError"
  }
}

type CommandResult = Readonly<{ code: number; stdout: string; stderr: string }>

export function isMissingR2Object(output: string): boolean {
  return /(?:10007|NoSuchKey|specified key does not exist|object not found)/iu.test(output)
}

export async function runWrangler(args: readonly string[]): Promise<CommandResult> {
  const controller = new AbortController()
  const timeout = setTimeout(() => controller.abort(), WRANGLER_TIMEOUT_MILLISECONDS)
  try {
    const child = Bun.spawn([process.execPath, "x", "wrangler", ...args], {
      cwd: repositoryRoot,
      env: process.env,
      stdout: "pipe",
      stderr: "pipe",
      signal: controller.signal,
    })
    const [stdout, stderr, code] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ])
    return Object.freeze({ code, stdout, stderr })
  } catch (error) {
    if (controller.signal.aborted) throw new CloudflareError("Wrangler exceeded its 30 minute operation bound")
    throw new CloudflareError(error instanceof Error ? error.message : "Wrangler could not start")
  } finally {
    clearTimeout(timeout)
  }
}

async function requireWrangler(args: readonly string[], operation: string): Promise<CommandResult> {
  const result = await runWrangler(args)
  if (result.code !== 0) {
    const detail = `${result.stderr}\n${result.stdout}`.trim()
    throw new CloudflareError(`${operation} failed${detail ? `: ${detail}` : ""}`)
  }
  return result
}

async function downloadRemoteObject(key: string, pathname: string): Promise<"Downloaded" | "Missing"> {
  await rm(pathname, { force: true })
  const result = await runWrangler([
    "r2",
    "object",
    "get",
    `${CLOUDFLARE_ASSET_BUCKET}/${key}`,
    `--file=${pathname}`,
    "--remote",
    `--config=${WRANGLER_CONFIG}`,
  ])
  if (result.code === 0) return "Downloaded"
  const output = `${result.stderr}\n${result.stdout}`
  if (isMissingR2Object(output)) return "Missing"
  throw new CloudflareError(`remote object read failed: ${output.trim()}`)
}

async function publishObject(
  config: LocalConfig,
  expected: ObjectDescriptor,
  temporaryDirectory: string,
): Promise<"Uploaded" | "AlreadyPresent"> {
  const source = releaseObjectPath(config, expected)
  await verifyFile(source, expected)
  const bytes = Number(expected.byteLength)
  if (bytes > MAX_WRANGLER_OBJECT_BYTES) {
    throw new CloudflareError(`object ${expected.sha256} exceeds Wrangler's 300000000 byte publication bound`)
  }
  const key = `objects/sha256/${expected.sha256}`
  const downloaded = path.join(temporaryDirectory, `${expected.sha256}.download`)
  try {
    if (await downloadRemoteObject(key, downloaded) === "Downloaded") {
      await verifyFile(downloaded, expected)
      return "AlreadyPresent"
    }
    await requireWrangler([
      "r2",
      "object",
      "put",
      `${CLOUDFLARE_ASSET_BUCKET}/${key}`,
      `--file=${source}`,
      `--content-type=${expected.mediaType}`,
      "--cache-control=public, max-age=31536000, immutable, no-transform",
      "--remote",
      `--config=${WRANGLER_CONFIG}`,
    ], `remote object ${expected.sha256} upload`)
    if (await downloadRemoteObject(key, downloaded) !== "Downloaded") {
      throw new CloudflareError(`uploaded object ${expected.sha256} is absent on readback`)
    }
    await verifyFile(downloaded, expected)
    return "Uploaded"
  } finally {
    await rm(downloaded, { force: true })
  }
}

export async function publishTf2Release(config: LocalConfig, target: string | undefined): Promise<void> {
  const infrastructure = await runWrangler([
    "r2",
    "bucket",
    "info",
    CLOUDFLARE_ASSET_BUCKET,
    "--json",
    `--config=${WRANGLER_CONFIG}`,
  ])
  if (infrastructure.code !== 0) {
    throw new CloudflareError("playsrc production R2 bucket is unavailable; apply infra/cloudflare first")
  }
  const artifact = await prepareTf2Release(config, target)
  const temporaryDirectory = path.join(config.sourceCacheDir, "cloudflare-publication")
  await mkdir(temporaryDirectory, { recursive: true })
  const outcomes: Record<string, "Uploaded" | "AlreadyPresent"> = {}
  for (const descriptor of Object.values(artifact.release.objects)) {
    outcomes[descriptor.sha256] = await publishObject(config, descriptor, temporaryDirectory)
  }
  console.log(JSON.stringify({ target: artifact.release.target, assetOrigin: CLOUDFLARE_ASSET_ORIGIN, outcomes }))
}
