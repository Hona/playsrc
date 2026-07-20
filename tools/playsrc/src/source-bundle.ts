import { mkdir, readFile, rename, rm, stat, writeFile } from "node:fs/promises"
import path from "node:path"
import type { LocalConfig } from "./config"
import { repositoryRoot } from "./config"
import { rustEnvironment } from "./setup"
import type { ObjectDescriptor } from "@playsrc/asset-store"
import toolchains from "../toolchains.json"

export type SourceBundleArtifact = Readonly<{
  bundlePath: string
  uiPath: string
  ledgerPath: string
  report: Readonly<{
    target: string
    contentBuild: "24207079"
    providers: number
    requests: number
    authoritativeAbsences: number
    entries: number
    bundleDescriptor: ObjectDescriptor
    uiEntries: number
    uiDescriptor: ObjectDescriptor
    ledgerDescriptor: ObjectDescriptor
  }>
}>

type SourceBundleReport = Readonly<{
  target?: unknown
  contentBuild?: unknown
  providers?: unknown
  requests?: unknown
  authoritativeAbsences?: unknown
  entries?: unknown
  bytes?: unknown
  sha256?: unknown
  bundleDescriptor?: unknown
  uiEntries?: unknown
  uiBytes?: unknown
  uiSha256?: unknown
  uiDescriptor?: unknown
  ledgerBytes?: unknown
  ledgerSha256?: unknown
  ledgerDescriptor?: unknown
}>

type SourceBundleCache = Readonly<{
  schema: "playsrc-source-bundle-cache-v1"
  generatorSha256: string
  report: SourceBundleReport
}>

const SHA256 = /^[0-9a-f]{64}$/

const descriptor = (
  value: unknown,
  kind: "derived-object",
  mediaType: string,
  byteLength: unknown,
  sha256: unknown,
): ObjectDescriptor => {
  if (
    typeof value !== "object"
    || value === null
    || Array.isArray(value)
    || Object.keys(value).sort().join("\0") !== "byteLength\0kind\0mediaType\0sha256"
  ) throw new Error("source bundle object descriptor is malformed")
  const record = value as Record<string, unknown>
  if (
    record.kind !== kind
    || record.mediaType !== mediaType
    || record.byteLength !== String(byteLength)
    || record.sha256 !== sha256
    || !/^(0|[1-9]\d*)$/.test(record.byteLength as string)
    || !/^[0-9a-f]{64}$/.test(record.sha256 as string)
  ) throw new Error("source bundle object descriptor differs from its report")
  return Object.freeze(record as ObjectDescriptor)
}

export function parseSourceBundleReport(output: string, target: string): SourceBundleArtifact["report"] {
  let report: SourceBundleReport
  try {
    report = JSON.parse(output) as SourceBundleReport
  } catch {
    throw new Error("source bundle report is not JSON")
  }
  if (
    report.target !== target
    || report.contentBuild !== "24207079"
    || !Number.isSafeInteger(report.providers)
    || (report.providers as number) < 2
    || (report.providers as number) > 65
    || !Number.isSafeInteger(report.requests)
    || (report.requests as number) < 1
    || (report.requests as number) > 4_096
    || !Number.isSafeInteger(report.authoritativeAbsences)
    || (report.authoritativeAbsences as number) < 0
    || (report.authoritativeAbsences as number) > (report.requests as number)
    || !Number.isSafeInteger(report.entries)
    || (report.entries as number) < 1
    || (report.entries as number) > (report.requests as number)
    || (report.entries as number) + (report.authoritativeAbsences as number) !== report.requests
    || !Number.isSafeInteger(report.bytes)
    || (report.bytes as number) < 12
    || (report.bytes as number) > 512 * 1024 * 1024
    || !Number.isSafeInteger(report.ledgerBytes)
    || (report.ledgerBytes as number) < 2
    || (report.ledgerBytes as number) > 8 * 1024 * 1024
    || typeof report.sha256 !== "string"
    || !/^[0-9a-f]{64}$/.test(report.sha256)
    || typeof report.ledgerSha256 !== "string"
    || !/^[0-9a-f]{64}$/.test(report.ledgerSha256)
    || !Number.isSafeInteger(report.uiEntries)
    || (report.uiEntries as number) < 1
    || (report.uiEntries as number) > 2_048
    || !Number.isSafeInteger(report.uiBytes)
    || (report.uiBytes as number) < 12
    || (report.uiBytes as number) > 512 * 1024 * 1024
    || typeof report.uiSha256 !== "string"
    || !/^[0-9a-f]{64}$/.test(report.uiSha256)
  ) {
    throw new Error("source bundle report is malformed")
  }
  const bundleDescriptor = descriptor(
    report.bundleDescriptor,
    "derived-object",
    "application/octet-stream",
    report.bytes,
    report.sha256,
  )
  const ledgerDescriptor = descriptor(
    report.ledgerDescriptor,
    "derived-object",
    "application/vnd.playsrc.source-dependency-ledger+json",
    report.ledgerBytes,
    report.ledgerSha256,
  )
  const uiDescriptor = descriptor(
    report.uiDescriptor,
    "derived-object",
    "application/octet-stream",
    report.uiBytes,
    report.uiSha256,
  )
  return Object.freeze({
    target,
    contentBuild: "24207079",
    providers: report.providers as number,
    requests: report.requests as number,
    authoritativeAbsences: report.authoritativeAbsences as number,
    entries: report.entries as number,
    bundleDescriptor,
    uiEntries: report.uiEntries as number,
    uiDescriptor,
    ledgerDescriptor,
  })
}

export function parseSourceBundleCache(
  output: string,
  target: string,
  generatorSha256: string,
): SourceBundleArtifact["report"] | null {
  try {
    const value = JSON.parse(output) as SourceBundleCache
    if (
      typeof value !== "object"
      || value === null
      || Array.isArray(value)
      || Object.keys(value).sort().join("\0") !== "generatorSha256\0report\0schema"
      || value.schema !== "playsrc-source-bundle-cache-v1"
      || value.generatorSha256 !== generatorSha256
      || !SHA256.test(value.generatorSha256)
    ) return null
    return parseSourceBundleReport(JSON.stringify(value.report), target)
  } catch {
    return null
  }
}

function sha256(bytes: Uint8Array): string {
  return new Bun.CryptoHasher("sha256").update(bytes).digest("hex")
}

async function artifactsHaveDeclaredSizes(
  paths: Readonly<{ bundlePath: string; uiPath: string; ledgerPath: string }>,
  report: SourceBundleArtifact["report"],
): Promise<boolean> {
  try {
    const [bundle, ui, ledger] = await Promise.all([
      stat(paths.bundlePath),
      stat(paths.uiPath),
      stat(paths.ledgerPath),
    ])
    return bundle.isFile()
      && ui.isFile()
      && ledger.isFile()
      && bundle.size === Number(report.bundleDescriptor.byteLength)
      && ui.size === Number(report.uiDescriptor.byteLength)
      && ledger.size === Number(report.ledgerDescriptor.byteLength)
  } catch {
    return false
  }
}

export async function buildSourceBundle(config: LocalConfig, target: string): Promise<SourceBundleArtifact> {
  const executable = process.platform === "win32" ? "cargo.exe" : "cargo"
  const cargo = path.join(config.sourceCacheDir, "toolchains", "rust", "cargo", "bin", executable)
  const environment = { ...process.env, ...rustEnvironment(config.sourceCacheDir) }
  const build = Bun.spawn([
    cargo,
    `+${toolchains.rust.toolchain}`,
    "build",
    "--profile",
    "source-bundle",
    "-p",
    "playsrc-source-bundle",
  ], {
    cwd: repositoryRoot,
    env: environment,
    stdout: "ignore",
    stderr: "inherit",
  })
  if (await build.exited !== 0) throw new Error("source bundle build failed")

  const generatorPath = path.join(
    repositoryRoot,
    "target",
    "source-bundle",
    process.platform === "win32" ? "playsrc-source-bundle.exe" : "playsrc-source-bundle",
  )
  const generatorSha256 = sha256(await readFile(generatorPath))
  const directory = path.join(config.sourceCacheDir, "browser-bundles")
  const paths = Object.freeze({
    bundlePath: path.join(directory, `${target}.psdb`),
    uiPath: path.join(directory, `${target}.ui.puib`),
    ledgerPath: path.join(directory, `${target}.dependencies.json`),
  })
  const cachePath = path.join(directory, `${target}.source-bundle-cache.json`)
  try {
    const cached = parseSourceBundleCache(await readFile(cachePath, "utf8"), target, generatorSha256)
    if (cached && await artifactsHaveDeclaredSizes(paths, cached)) {
      return Object.freeze({ ...paths, report: cached })
    }
  } catch {}

  const child = Bun.spawn([generatorPath, target], {
    cwd: repositoryRoot,
    env: environment,
    stdout: "pipe",
    stderr: "inherit",
  })
  const output = await new Response(child.stdout).text()
  if (await child.exited !== 0) throw new Error("source bundle build failed")
  const report = parseSourceBundleReport(output, target)
  if (!await artifactsHaveDeclaredSizes(paths, report)) throw new Error("source bundle artifacts differ from their report")
  const cache: SourceBundleCache = Object.freeze({
    schema: "playsrc-source-bundle-cache-v1",
    generatorSha256,
    report: JSON.parse(output) as SourceBundleReport,
  })
  await mkdir(directory, { recursive: true })
  const temporary = `${cachePath}.${process.pid}.tmp`
  try {
    await writeFile(temporary, `${JSON.stringify(cache)}\n`)
    await rm(cachePath, { force: true })
    await rename(temporary, cachePath)
  } finally {
    await rm(temporary, { force: true })
  }
  return Object.freeze({ ...paths, report })
}
