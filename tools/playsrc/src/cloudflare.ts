import { readFile } from "node:fs/promises"
import path from "node:path"
import { GetObjectCommand, PutObjectCommand, S3Client, S3ServiceException } from "@aws-sdk/client-s3"
import type { ObjectDescriptor } from "@playsrc/asset-store"
import type { LocalConfig } from "./config"
import { repositoryRoot } from "./config"
import { prepareTf2Release, releaseObjectPath, verifyFile } from "./tf2-release"

export const CLOUDFLARE_ASSET_BUCKET = "playsrc-production-assets"
export const CLOUDFLARE_ASSET_ORIGIN = "https://assets.playsrc.online"
export const WRANGLER_CONFIG = path.join(repositoryRoot, "apps", "web", "tf2", "wrangler.jsonc")
const MAX_PUBLICATION_OBJECT_BYTES = 64 * 1024 * 1024
const WRANGLER_TIMEOUT_MILLISECONDS = 30 * 60 * 1_000

export class CloudflareError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "CloudflareError"
  }
}

type CommandResult = Readonly<{ code: number; stdout: string; stderr: string }>

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

export type RemoteObjectAdapter = Readonly<{
  read(key: string): Promise<Uint8Array | "Missing">
  create(key: string, expected: ObjectDescriptor, bytes: Uint8Array): Promise<"Created" | "PreconditionFailed">
  close(): void
}>

function digest(bytes: Uint8Array): string {
  return new Bun.CryptoHasher("sha256").update(bytes).digest("hex")
}

function verifyBytes(bytes: Uint8Array, expected: ObjectDescriptor, location: string): void {
  if (String(bytes.byteLength) !== expected.byteLength || digest(bytes) !== expected.sha256) {
    throw new CloudflareError(`${location} object ${expected.sha256} differs`)
  }
}

function preconditionFailed(error: unknown): boolean {
  return error instanceof S3ServiceException && error.$metadata.httpStatusCode === 412
}

function missingObject(error: unknown): boolean {
  return error instanceof S3ServiceException && (error.name === "NoSuchKey" || error.$metadata.httpStatusCode === 404)
}

export function createR2Adapter(environment: NodeJS.ProcessEnv = process.env): RemoteObjectAdapter {
  const accountId = environment.CLOUDFLARE_ACCOUNT_ID
  const accessKeyId = environment.AWS_ACCESS_KEY_ID
  const secretAccessKey = environment.AWS_SECRET_ACCESS_KEY
  if (!accountId || !/^[0-9a-f]{32}$/.test(accountId) || !accessKeyId || !secretAccessKey) {
    throw new CloudflareError("R2 conditional publication credentials are unavailable")
  }
  const client = new S3Client({
    region: "auto",
    endpoint: `https://${accountId}.r2.cloudflarestorage.com`,
    credentials: { accessKeyId, secretAccessKey },
  })
  return Object.freeze({
    async read(key: string): Promise<Uint8Array | "Missing"> {
      try {
        const response = await client.send(new GetObjectCommand({ Bucket: CLOUDFLARE_ASSET_BUCKET, Key: key }))
        if (!response.Body) throw new CloudflareError(`remote object ${key} has no body`)
        return new Uint8Array(await response.Body.transformToByteArray())
      } catch (error) {
        if (missingObject(error)) return "Missing"
        if (error instanceof CloudflareError) throw error
        throw new CloudflareError(`remote object ${key} read failed`)
      }
    },
    async create(key: string, expected: ObjectDescriptor, bytes: Uint8Array): Promise<"Created" | "PreconditionFailed"> {
      const command = new PutObjectCommand({
        Bucket: CLOUDFLARE_ASSET_BUCKET,
        Key: key,
        Body: bytes,
        ContentLength: bytes.byteLength,
        ContentType: expected.mediaType,
        CacheControl: "public, max-age=31536000, immutable, no-transform",
      })
      command.middlewareStack.add((next) => async (args) => {
        const request = args.request as { headers: Record<string, string> }
        request.headers["if-none-match"] = "*"
        return next(args)
      }, { step: "build", name: "playsrcIfNoneMatch" })
      try {
        await client.send(command)
        return "Created"
      } catch (error) {
        if (preconditionFailed(error)) return "PreconditionFailed"
        const detail = error instanceof S3ServiceException
          ? `${error.name} HTTP ${error.$metadata.httpStatusCode ?? "unknown"}: ${error.message}`
          : error instanceof Error ? `${error.name}: ${error.message}` : "unknown error"
        throw new CloudflareError(`remote object ${key} conditional create failed: ${detail}`)
      }
    },
    close(): void { client.destroy() },
  })
}

export async function publishImmutableObject(
  expected: ObjectDescriptor,
  bytes: Uint8Array,
  adapter: RemoteObjectAdapter,
): Promise<"Uploaded" | "AlreadyPresent"> {
  if (bytes.byteLength > MAX_PUBLICATION_OBJECT_BYTES) throw new CloudflareError(`object ${expected.sha256} exceeds the 67108864-byte publication bound`)
  verifyBytes(bytes, expected, "local")
  const key = `objects/sha256/${expected.sha256}`
  const existing = await adapter.read(key)
  if (existing !== "Missing") {
    verifyBytes(existing, expected, "remote")
    return "AlreadyPresent"
  }
  const created = await adapter.create(key, expected, bytes)
  const readback = await adapter.read(key)
  if (readback === "Missing") throw new CloudflareError(`remote object ${expected.sha256} is absent after ${created}`)
  verifyBytes(readback, expected, "remote")
  return created === "Created" ? "Uploaded" : "AlreadyPresent"
}

export function sortPublicationDescriptors(descriptors: readonly ObjectDescriptor[]): readonly ObjectDescriptor[] {
  const rank = (kind: ObjectDescriptor["kind"]) => kind === "catalog" ? 2 : kind.endsWith("root") ? 1 : 0
  return Object.freeze([...descriptors].sort((left, right) => rank(left.kind) - rank(right.kind) || left.sha256.localeCompare(right.sha256)))
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
  const descriptors = sortPublicationDescriptors(Array.from(artifact.files.values(), ({ descriptor }) => descriptor))
  const adapter = createR2Adapter()
  const objects: { sha256: string; byteLength: string; kind: ObjectDescriptor["kind"]; outcome: "Uploaded" | "AlreadyPresent" }[] = []
  const totalBytes = descriptors.reduce((total, descriptor) => total + Number(descriptor.byteLength), 0)
  const started = Date.now()
  let verifiedBytes = 0, uploaded = 0, alreadyPresent = 0
  const progress = (state = "publishing") => {
    const percent = totalBytes === 0 ? 100 : Math.floor(verifiedBytes / totalBytes * 1000) / 10
    console.error(`[publish] ${state} ${percent.toFixed(1)}% | verified ${(verifiedBytes / 1_048_576).toFixed(2)}/${(totalBytes / 1_048_576).toFixed(2)} MiB | objects ${objects.length}/${descriptors.length} | uploaded ${uploaded} | already present ${alreadyPresent} | ${((Date.now() - started) / 1000).toFixed(0)}s`)
  }
  progress()
  const timer = setInterval(progress, 1000)
  try {
    for (const descriptor of descriptors) {
      const source = releaseObjectPath(config, descriptor)
      await verifyFile(source, descriptor)
      const outcome = await publishImmutableObject(descriptor, await readFile(source), adapter)
      objects.push({ sha256: descriptor.sha256, byteLength: descriptor.byteLength, kind: descriptor.kind, outcome })
      // Count completed remote readback, not bytes merely sent or found by name.
      verifiedBytes += Number(descriptor.byteLength)
      if (outcome === "Uploaded") uploaded++
      else alreadyPresent++
    }
  } finally {
    clearInterval(timer)
    adapter.close()
    progress(objects.length === descriptors.length ? "complete" : "failed")
  }
  console.log(JSON.stringify({
    schema: "playsrc-r2-publication-v1",
    defaultTarget: artifact.release.defaultTarget,
    targets: artifact.release.targets.map((target) => target.target),
    assetOrigin: CLOUDFLARE_ASSET_ORIGIN,
    objects,
    totals: {
      objects: objects.length,
      bytes: objects.reduce((total, object) => total + Number(object.byteLength), 0),
      uploaded: objects.filter((object) => object.outcome === "Uploaded").length,
      alreadyPresent: objects.filter((object) => object.outcome === "AlreadyPresent").length,
    },
  }))
}
