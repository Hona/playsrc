import { createHash } from "node:crypto"
import { mkdir, readFile, rename, rm, stat, writeFile } from "node:fs/promises"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"

type LocalConfig = {
  tf2Dir: string
  sourceCacheDir: string
  assetDir: string
}

type Manifest = {
  identity: string
  url: string
  byteLength: number
  sha256: string
  configuredTf2: {
    appBuildId: string
    patchVersion: string
    clientVersion: string
    serverVersion: string
    depotManifests: Record<string, string>
  }
  expected: Record<string, unknown>
  expectedNetworking: Record<string, unknown>
}

const scriptDir = dirname(fileURLToPath(import.meta.url))
const packageDir = resolve(scriptDir, "..")
const rootDir = resolve(packageDir, "../../..")
const configPath = join(rootDir, "playsrc.local.json")
const manifestPath = join(packageDir, "evidence/controlled-tf2-dem.json")
const cargoManifest = join(packageDir, "rust/Cargo.toml")
const networkingCargoManifest = join(rootDir, "packages/runtime/networking/rust/Cargo.toml")

const config = await readExactJson<LocalConfig>(configPath, ["assetDir", "sourceCacheDir", "tf2Dir"])
const manifest = await readExactJson<Manifest>(manifestPath)
validateManifest(manifest)
await validateConfiguredBuild(config, manifest)

const cacheDir = join(config.sourceCacheDir, "evidence/source-demo-network-replay")
const demoPath = join(cacheDir, `${manifest.sha256}.dem`)
await mkdir(cacheDir, { recursive: true })
if (!(await matchesIdentity(demoPath, manifest.byteLength, manifest.sha256))) {
  const temporaryPath = `${demoPath}.partial`
  await rm(temporaryPath, { force: true })
  const response = await fetch(manifest.url, { redirect: "follow" })
  if (!response.ok || !response.body) {
    throw new Error(`capture request failed: ${response.status} ${response.statusText}`)
  }
  const declaredLength = response.headers.get("content-length")
  if (declaredLength !== null && Number(declaredLength) !== manifest.byteLength) {
    throw new Error(`capture content-length ${declaredLength} != ${manifest.byteLength}`)
  }
  const bytes = await readExactBody(response.body, manifest.byteLength)
  const hash = sha256(bytes)
  if (hash !== manifest.sha256) {
    throw new Error(`capture sha256 ${hash} != ${manifest.sha256}`)
  }
  await writeFile(temporaryPath, bytes, { flag: "wx" })
  await rename(temporaryPath, demoPath)
}

const actual = await runJson([
    "cargo",
    "run",
    "--quiet",
    "--manifest-path",
    cargoManifest,
    "--example",
    "verify_controlled",
    "--",
    demoPath,
    manifest.identity,
    manifest.sha256,
])
compareRecord(actual, manifest.expected, "DEM")
const networking = await runJson([
  "cargo",
  "run",
  "--release",
  "--quiet",
  "--manifest-path",
  networkingCargoManifest,
  "--example",
  "verify_demo",
  "--",
  demoPath,
])
compareRecord(networking, manifest.expectedNetworking, "networking")
const report = {
  capture: manifest.identity,
  path: demoPath,
  byteLength: manifest.byteLength,
  sha256: manifest.sha256,
  configuredTf2: manifest.configuredTf2,
  parsed: actual,
  networking,
}
await writeFile(join(cacheDir, `${manifest.sha256}.report.json`), `${JSON.stringify(report, null, 2)}\n`)
process.stdout.write(`${JSON.stringify(report, null, 2)}\n`)

async function readExactJson<T>(path: string, exactKeys?: string[]): Promise<T> {
  let value: unknown
  try {
    value = JSON.parse(await readFile(path, "utf8"))
  } catch (error) {
    throw new Error(`cannot read required JSON ${path}: ${String(error)}`)
  }
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${path} must contain one JSON object`)
  }
  if (exactKeys) {
    const actual = Object.keys(value).sort()
    if (JSON.stringify(actual) !== JSON.stringify(exactKeys)) {
      throw new Error(`${path} keys ${actual.join(",")} != ${exactKeys.join(",")}`)
    }
  }
  return value as T
}

function validateManifest(value: Manifest): void {
  if (!/^https:\/\//.test(value.url)) throw new Error("capture URL must use HTTPS")
  if (!/^[0-9a-f]{64}$/.test(value.sha256)) throw new Error("capture sha256 must be lowercase hex")
  if (!Number.isSafeInteger(value.byteLength) || value.byteLength <= 1_072 || value.byteLength > 64 * 1024 * 1024) {
    throw new Error("capture byteLength must be in 1073..67108864")
  }
}

async function validateConfiguredBuild(config: LocalConfig, manifest: Manifest): Promise<void> {
  for (const [field, value] of Object.entries(config)) {
    if (typeof value !== "string" || value.length === 0 || !value.startsWith("/")) {
      throw new Error(`playsrc.local.json ${field} must be an absolute non-empty path`)
    }
  }
  const steamInf = await readFile(join(config.tf2Dir, "steam.inf"), "utf8")
  for (const [field, expected] of [
    ["PatchVersion", manifest.configuredTf2.patchVersion],
    ["ClientVersion", manifest.configuredTf2.clientVersion],
    ["ServerVersion", manifest.configuredTf2.serverVersion],
  ] as const) {
    if (!steamInf.split(/\r?\n/).includes(`${field}=${expected}`)) {
      throw new Error(`configured steam.inf does not contain ${field}=${expected}`)
    }
  }
  const appManifest = await readFile(resolve(config.tf2Dir, "../steamapps/appmanifest_440.acf"), "utf8")
  requireAcfPair(appManifest, "buildid", manifest.configuredTf2.appBuildId)
  for (const [depot, expected] of Object.entries(manifest.configuredTf2.depotManifests)) {
    const block = appManifest.match(new RegExp(`"${depot}"\\s*\\{([\\s\\S]*?)\\}`))?.[1]
    if (!block) throw new Error(`configured appmanifest has no depot ${depot}`)
    requireAcfPair(block, "manifest", expected)
  }
}

function requireAcfPair(source: string, key: string, expected: string): void {
  if (!new RegExp(`"${key}"\\s+"${expected}"`).test(source)) {
    throw new Error(`configured appmanifest does not contain ${key}=${expected}`)
  }
}

async function matchesIdentity(path: string, bytes: number, expectedHash: string): Promise<boolean> {
  try {
    if ((await stat(path)).size !== bytes) return false
    return sha256(await readFile(path)) === expectedHash
  } catch {
    return false
  }
}

async function readExactBody(body: ReadableStream<Uint8Array>, expectedBytes: number): Promise<Uint8Array> {
  const reader = body.getReader()
  const chunks: Uint8Array[] = []
  let received = 0
  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    received += value.byteLength
    if (received > expectedBytes) {
      await reader.cancel("capture exceeds checked byteLength")
      throw new Error(`capture byte length exceeds ${expectedBytes}`)
    }
    chunks.push(value)
  }
  if (received !== expectedBytes) {
    throw new Error(`capture byte length ${received} != ${expectedBytes}`)
  }
  const output = new Uint8Array(received)
  let offset = 0
  for (const chunk of chunks) {
    output.set(chunk, offset)
    offset += chunk.byteLength
  }
  return output
}

function sha256(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex")
}

async function runJson(command: string[]): Promise<Record<string, unknown>> {
  const child = Bun.spawn(command, { cwd: rootDir, stdout: "pipe", stderr: "pipe" })
  const [status, stdout, stderr] = await Promise.all([
    child.exited,
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
  ])
  if (status !== 0) {
    throw new Error(`${command[0]} failed (${status}): ${stderr.trim()}`)
  }
  return JSON.parse(stdout) as Record<string, unknown>
}

function compareRecord(
  actual: Record<string, unknown>,
  expected: Record<string, unknown>,
  owner: string,
): void {
  for (const [field, value] of Object.entries(expected)) {
    if (JSON.stringify(actual[field]) !== JSON.stringify(value)) {
      throw new Error(`${owner} ${field} mismatch: ${JSON.stringify(actual[field])} != ${JSON.stringify(value)}`)
    }
  }
}
