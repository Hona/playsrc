import { copyFile, mkdir, readFile, rename, rm, stat, writeFile } from "node:fs/promises"
import path from "node:path"
import type { LocalConfig } from "./config"
import { repositoryRoot } from "./config"
import { rustEnvironment } from "./setup"
import type { ObjectDescriptor } from "@playsrc/asset-store"
import { parseResourceGraphBytes, type ResourceGraph } from "@playsrc/asset-store/graph"
import toolchains from "../toolchains.json"
import { TF2_CONTENT_BUILD } from "@playsrc/game-tf2-browser/content-build"
import { buildCacheDirectory, rustBuildIdentity } from "./build-identity"

export type SourceBundleArtifact = Readonly<{
  graphPath: string
  graphObjectDirectory: string
  ledgerPath: string
  graph: ResourceGraph
  report: Readonly<{
    target: string
    contentBuild: string
    providers: number
    requests: number
    authoritativeAbsences: number
    entries: number
    packedEntries: number
    derivedEntries: number
    graphEntries: number
    graphChunks: number
    graphEncodedBytes: number
    graphDescriptor: ObjectDescriptor
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
  packedEntries?: unknown
  derivedEntries?: unknown
  graphEntries?: unknown
  graphChunks?: unknown
  graphEncodedBytes?: unknown
  graphDescriptor?: unknown
  ledgerBytes?: unknown
  ledgerSha256?: unknown
  ledgerDescriptor?: unknown
}>

type SourceBundleCache = Readonly<{
  schema: "playsrc-resource-graph-cache-v1"
  generatorSha256: string
  report: SourceBundleReport
}>

const SHA256 = /^[0-9a-f]{64}$/

const descriptor = (
  value: unknown,
  kind: "derived-object" | "source-root",
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
    || report.contentBuild !== TF2_CONTENT_BUILD.contentBuild
    || !Number.isSafeInteger(report.providers)
    || (report.providers as number) < 2
    || (report.providers as number) > 65
    || !Number.isSafeInteger(report.requests)
    || (report.requests as number) < 1
    || (report.requests as number) > 16_384
    || !Number.isSafeInteger(report.authoritativeAbsences)
    || (report.authoritativeAbsences as number) < 0
    || (report.authoritativeAbsences as number) > (report.requests as number)
    || !Number.isSafeInteger(report.entries)
    || (report.entries as number) < 1
    || (report.entries as number) > (report.requests as number)
    || !Number.isSafeInteger(report.packedEntries)
    || (report.packedEntries as number) < 0
    || (report.packedEntries as number) > (report.requests as number)
    || (report.entries as number) + (report.packedEntries as number) + (report.authoritativeAbsences as number) !== report.requests
    || !Number.isSafeInteger(report.derivedEntries)
    || (report.derivedEntries as number) < 1
    || (report.derivedEntries as number) > 2_048
    || !Number.isSafeInteger(report.graphEntries)
    || report.graphEntries !== (report.entries as number) + (report.derivedEntries as number)
    || !Number.isSafeInteger(report.graphChunks)
    || (report.graphChunks as number) < 1
    || (report.graphChunks as number) > 1_024
    || !Number.isSafeInteger(report.graphEncodedBytes)
    || (report.graphEncodedBytes as number) < 1
    || (report.graphEncodedBytes as number) > 1024 * 1024 * 1024
    || !Number.isSafeInteger(report.ledgerBytes)
    || (report.ledgerBytes as number) < 2
    || (report.ledgerBytes as number) > 8 * 1024 * 1024
    || typeof report.ledgerSha256 !== "string"
    || !/^[0-9a-f]{64}$/.test(report.ledgerSha256)
  ) {
    throw new Error("source bundle report is malformed")
  }
  const graphRecord = report.graphDescriptor as Record<string, unknown> | undefined
  const graphDescriptor = descriptor(
    report.graphDescriptor,
    "source-root",
    "application/vnd.playsrc.resource-graph+json",
    graphRecord?.byteLength,
    graphRecord?.sha256,
  )
  const ledgerDescriptor = descriptor(
    report.ledgerDescriptor,
    "derived-object",
    "application/vnd.playsrc.source-dependency-ledger+json",
    report.ledgerBytes,
    report.ledgerSha256,
  )
  return Object.freeze({
    target,
    contentBuild: TF2_CONTENT_BUILD.contentBuild,
    providers: report.providers as number,
    requests: report.requests as number,
    authoritativeAbsences: report.authoritativeAbsences as number,
    entries: report.entries as number,
    packedEntries: report.packedEntries as number,
    derivedEntries: report.derivedEntries as number,
    graphEntries: report.graphEntries as number,
    graphChunks: report.graphChunks as number,
    graphEncodedBytes: report.graphEncodedBytes as number,
    graphDescriptor,
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
      || value.schema !== "playsrc-resource-graph-cache-v1"
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

async function readVerifiedGraph(
  paths: Readonly<{ graphPath: string; graphObjectDirectory: string; ledgerPath: string }>,
  report: SourceBundleArtifact["report"],
): Promise<Readonly<{ graph: ResourceGraph | null; error?: string }>> {
  try {
    const [graphBytes, ledgerBytes] = await Promise.all([
      readFile(paths.graphPath),
      readFile(paths.ledgerPath),
    ])
    if (graphBytes.byteLength !== Number(report.graphDescriptor.byteLength) || sha256(graphBytes) !== report.graphDescriptor.sha256
      || ledgerBytes.byteLength !== Number(report.ledgerDescriptor.byteLength) || sha256(ledgerBytes) !== report.ledgerDescriptor.sha256) {
      return Object.freeze({ graph: null, error: "root or ledger descriptor differs" })
    }
    const graph = parseResourceGraphBytes(graphBytes)
    if (graph.chunks.length !== report.graphChunks || graph.chunks.reduce((total, chunk) => total + Number(chunk.encodedByteLength), 0) !== report.graphEncodedBytes) return Object.freeze({ graph: null, error: "chunk totals differ" })
    const objects = await Promise.all(graph.chunks.map((chunk) => stat(path.join(paths.graphObjectDirectory, chunk.encodedSha256))))
    if (objects.some((metadata, index) => !metadata.isFile() || metadata.size !== Number(graph.chunks[index]!.encodedByteLength))) return Object.freeze({ graph: null, error: "chunk file size differs" })
    return Object.freeze({ graph })
  } catch (error) {
    return Object.freeze({ graph: null, error: error instanceof Error ? error.message : "graph verification failed" })
  }
}

async function immutableBundlePaths(
  directory: string,
  paths: Readonly<{ graphPath: string; graphObjectDirectory: string; ledgerPath: string }>,
  report: SourceBundleArtifact["report"],
): Promise<Readonly<{ graphPath: string; graphObjectDirectory: string; ledgerPath: string }>> {
  const snapshots = path.join(directory, "immutable-roots")
  await mkdir(snapshots, { recursive: true })
  const retain = async (source: string, identity: ObjectDescriptor): Promise<string> => {
    const destination = path.join(snapshots, identity.sha256)
    try {
      const existing = await readFile(destination)
      if (String(existing.byteLength) === identity.byteLength && sha256(existing) === identity.sha256) return destination
    } catch {}
    const bytes = await readFile(source)
    if (String(bytes.byteLength) !== identity.byteLength || sha256(bytes) !== identity.sha256) {
      throw new Error("source bundle root changed before its immutable snapshot")
    }
    const temporary = `${destination}.${process.pid}.tmp`
    try {
      await writeFile(temporary, bytes)
      await rename(temporary, destination)
    } finally {
      await rm(temporary, { force: true })
    }
    return destination
  }
  const [graphPath, ledgerPath] = await Promise.all([
    retain(paths.graphPath, report.graphDescriptor),
    retain(paths.ledgerPath, report.ledgerDescriptor),
  ])
  return Object.freeze({ graphPath, ledgerPath, graphObjectDirectory: paths.graphObjectDirectory })
}

type SourceBundleProducer = Readonly<{ executable: string; generatorSha256: string; environment: Record<string, string | undefined> }>

const producers = new Map<string, Promise<SourceBundleProducer>>()

export async function prepareSourceBundleProducer(config: LocalConfig): Promise<SourceBundleProducer> {
  const identity = await rustBuildIdentity()
  const prior = producers.get(identity)
  if (prior) return prior
  const operation = (async (): Promise<SourceBundleProducer> => {
  const executable = process.platform === "win32" ? "cargo.exe" : "cargo"
  const cargo = path.join(config.sourceCacheDir, "toolchains", "rust", "cargo", "bin", executable)
  const environment = { ...process.env, ...rustEnvironment(config.sourceCacheDir) }
  const filename = process.platform === "win32" ? "playsrc-source-bundle.exe" : "playsrc-source-bundle"
  const generatorPath = path.join(buildCacheDirectory(config.sourceCacheDir, identity), filename)
  let available = false
  try { available = (await stat(generatorPath)).isFile() } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error
  }
  if (!available) {
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
    const built = path.join(repositoryRoot, "target", "source-bundle", filename)
    await mkdir(path.dirname(generatorPath), { recursive: true })
    const temporary = `${generatorPath}.${process.pid}.${crypto.randomUUID()}.tmp`
    try {
      await copyFile(built, temporary)
      await rename(temporary, generatorPath)
    } finally {
      await rm(temporary, { force: true })
    }
  }
  const [generatorBytes,uiManifestBytes]=await Promise.all([
    readFile(generatorPath),
    readFile(path.join(repositoryRoot,"tools/source-bundle/tf2-ui.generated.json")),
  ])
  const generatorSha256=new Bun.CryptoHasher("sha256")
    .update(`playsrc-source-bundle-producer-v2\0${generatorBytes.byteLength}\0`)
    .update(generatorBytes)
    .update(uiManifestBytes)
    .digest("hex")
  return Object.freeze({ executable: generatorPath, generatorSha256, environment })
  })()
  producers.set(identity, operation)
  try { return await operation } catch (error) { producers.delete(identity); throw error }
}

export async function buildSourceBundle(config: LocalConfig, target: string): Promise<SourceBundleArtifact> {
  const { executable: generatorPath, generatorSha256, environment } = await prepareSourceBundleProducer(config)
  const directory = path.join(config.sourceCacheDir, "browser-bundles")
  const snapshotDirectory = path.join(directory, "generators", generatorSha256)
  const paths = Object.freeze({
    graphPath: path.join(snapshotDirectory, `${target}.graph.json`),
    graphObjectDirectory: path.join(directory, `${target}.graph`, "objects"),
    ledgerPath: path.join(snapshotDirectory, `${target}.dependencies.json`),
  })
  const cachePath = path.join(snapshotDirectory, `${target}.source-bundle-cache.json`)
  try {
    const cached = parseSourceBundleCache(await readFile(cachePath, "utf8"), target, generatorSha256)
    const cachedGraph = cached ? await readVerifiedGraph(paths, cached) : null
    if (cached && cachedGraph?.graph) {
      return Object.freeze({ ...await immutableBundlePaths(directory, paths, cached), graph: cachedGraph.graph, report: cached })
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
  const verified = await readVerifiedGraph(paths, report)
  if (!verified.graph) throw new Error(`resource graph snapshot differs from its report: ${verified.error}`)
  const graph = verified.graph
  const cache: SourceBundleCache = Object.freeze({
    schema: "playsrc-resource-graph-cache-v1",
    generatorSha256,
    report: JSON.parse(output) as SourceBundleReport,
  })
  await mkdir(snapshotDirectory, { recursive: true })
  const temporary = `${cachePath}.${process.pid}.tmp`
  try {
    await writeFile(temporary, `${JSON.stringify(cache)}\n`)
    await rm(cachePath, { force: true })
    await rename(temporary, cachePath)
  } finally {
    await rm(temporary, { force: true })
  }
  return Object.freeze({ ...await immutableBundlePaths(directory, paths, report), graph, report })
}
