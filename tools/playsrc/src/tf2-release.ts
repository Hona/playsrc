import { createReadStream } from "node:fs"
import { mkdir, readFile, rename, rm, stat, writeFile } from "node:fs/promises"
import path from "node:path"
import { AssetStoreError, descriptor, objectPath, putObject, verifyObject, type ObjectDescriptor } from "@playsrc/asset-store"
import { canonicalGraphBytes, parseResourceCatalogBytes, parseResourceGraphBytes, resourceChunkObject, selectCatalogTarget } from "@playsrc/asset-store/graph"
import { parseTf2Release, TF2_RELEASE_SCHEMA, type Tf2Release } from "../../../apps/web/tf2/src/deployment"
import { TF2_TARGET_NAMES } from "@playsrc/game-tf2-browser/maps"
import type { LocalConfig } from "./config"
import { repositoryRoot } from "./config"
import { buildSourceBundle } from "./source-bundle"
import { acquireMap } from "./targets"
import { buildTf2Wasm } from "./tf2-wasm-build"
import { captureWasmBindings } from "./wasm-bindings"

const RELEASE_DIRECTORY = path.join(repositoryRoot, "apps", "web", "tf2", "releases")
const RELEASE_PATH = path.join(RELEASE_DIRECTORY, "current.json")
const CATALOG_PATH = path.join(RELEASE_DIRECTORY, "catalog.json")

export type Tf2ReleaseArtifact = Readonly<{
  release: Tf2Release
  files: ReadonlyMap<string, Readonly<{ descriptor: ObjectDescriptor; pathname: string }>>
}>

export class Tf2ReleaseError extends Error {
  constructor(message: string) { super(message); this.name = "Tf2ReleaseError" }
}

async function ensureLocalObject(root: string, expected: ObjectDescriptor, pathname: string): Promise<void> {
  try { await verifyObject(root, expected); return }
  catch (error) { if (!(error instanceof AssetStoreError) || error.code !== "MissingObject") throw error }
  try { await putObject(root, expected, await readFile(pathname)) }
  catch (error) {
    throw new Tf2ReleaseError(`local object ${expected.sha256} from ${path.basename(pathname)} differs: ${error instanceof Error ? error.message : String(error)}`)
  }
}

export async function prepareTf2Release(config: LocalConfig, target: string | undefined): Promise<Tf2ReleaseArtifact> {
  if (target !== undefined) throw new Tf2ReleaseError("the current TF2 release does not accept a target argument")
  const artifact = await buildTf2ReleaseCandidate(config, TF2_TARGET_NAMES)
  const catalog = artifact.files.get(artifact.release.objects.catalog.sha256)!
  await writeTf2Release(artifact.release, await readFile(catalog.pathname))
  return artifact
}

/** Prepare local candidate bytes without changing the approved release pointer. */
export async function buildTf2ReleaseCandidate(config: LocalConfig, names: readonly (typeof TF2_TARGET_NAMES)[number][]): Promise<Tf2ReleaseArtifact> {
  if (!names.length || names.some((name,index)=>!TF2_TARGET_NAMES.includes(name)||index>0&&TF2_TARGET_NAMES.indexOf(names[index-1]!)>=TF2_TARGET_NAMES.indexOf(name))) throw new Tf2ReleaseError("Candidate target scope is invalid")
  const wasm = buildTf2Wasm(config)
  const maps = await Promise.all(names.map(async (name) => Object.freeze({ name, map: await acquireMap(config, name) })))
  const prepared = [] as Array<Readonly<{ name: (typeof TF2_TARGET_NAMES)[number]; map: Awaited<ReturnType<typeof acquireMap>>; sourceBundle: Awaited<ReturnType<typeof buildSourceBundle>> }>>
  for (const { name, map } of maps) prepared.push(Object.freeze({ name, map, sourceBundle: await buildSourceBundle(config, name) }))
  const wasmPath = await wasm
  const wasmBytes = await readFile(wasmPath)
  const catalogBytes = canonicalGraphBytes({
    application: "tf2",
    entries: prepared.map(({ name, sourceBundle }) => ({ target: name, resources: sourceBundle.report.graphDescriptor }))
      .toSorted((left, right) => left.target.localeCompare(right.target)),
    schema: "playsrc-resource-catalog-v1",
  })
  const catalog = parseResourceCatalogBytes(catalogBytes)
  for (const { name, sourceBundle } of prepared) {
    const graph = parseResourceGraphBytes(await readFile(sourceBundle.graphPath))
    const selected = selectCatalogTarget(catalog, name)
    if (graph.target !== name || graph.contentBuild !== sourceBundle.report.contentBuild || !sameDescriptor(selected.resources, sourceBundle.report.graphDescriptor)) {
      throw new Tf2ReleaseError(`generated ${name} resource root differs from its catalog entry`)
    }
  }
  const catalogArtifactPath = path.join(config.sourceCacheDir, "browser-bundles", "catalog.json")
  await mkdir(path.dirname(catalogArtifactPath), { recursive: true })
  await writeFile(catalogArtifactPath, catalogBytes)
  const release = parseTf2Release({
    schema: TF2_RELEASE_SCHEMA,
    wasmBindings: await captureWasmBindings(path.dirname(wasmPath), descriptor("derived-object", "application/octet-stream", wasmBytes)),
    defaultTarget: "jump_beef",
    objects: {
      wasm: descriptor("derived-object", "application/octet-stream", wasmBytes),
      catalog: descriptor("catalog", "application/vnd.playsrc.asset-catalog+json", catalogBytes),
    },
    targets: prepared.map(({ name, map, sourceBundle }) => ({
      target: name,
      contentBuild: sourceBundle.report.contentBuild,
      objects: {
        bsp: { kind: "source-object", mediaType: "application/octet-stream", byteLength: String(map.decoded.byteLength), sha256: map.decoded.sha256 },
        resources: sourceBundle.report.graphDescriptor,
        dependencyLedger: sourceBundle.report.ledgerDescriptor,
      },
    })),
  })
  const files = new Map<string, Readonly<{ descriptor: ObjectDescriptor; pathname: string }>>()
  addFile(files, release.objects.wasm, wasmPath)
  addFile(files, release.objects.catalog, catalogArtifactPath)
  for (let index = 0; index < prepared.length; index += 1) {
    const { map, sourceBundle } = prepared[index]
    const targetRelease = release.targets[index]
    addFile(files, targetRelease.objects.bsp, path.join(config.sourceCacheDir, map.decoded.cachePath))
    addFile(files, targetRelease.objects.resources, sourceBundle.graphPath)
    addFile(files, targetRelease.objects.dependencyLedger, sourceBundle.ledgerPath)
    for (const chunk of sourceBundle.graph.chunks) {
      const chunkDescriptor = resourceChunkObject(chunk)
      addFile(files, chunkDescriptor, path.join(sourceBundle.graphObjectDirectory, chunkDescriptor.sha256))
    }
  }
  await Promise.all([...files.values()].map(({ descriptor: expected, pathname }) => ensureLocalObject(config.assetDir, expected, pathname)))
  return Object.freeze({ release, files })
}

function addFile(files: Map<string, Readonly<{ descriptor: ObjectDescriptor; pathname: string }>>, expected: ObjectDescriptor, pathname: string): void {
  const existing = files.get(expected.sha256)
  if (existing && !sameDescriptor(existing.descriptor, expected)) throw new Tf2ReleaseError(`object ${expected.sha256} has conflicting descriptors`)
  if (!existing) files.set(expected.sha256, Object.freeze({ descriptor: expected, pathname }))
}

function sameDescriptor(left: ObjectDescriptor, right: ObjectDescriptor): boolean {
  return left.kind === right.kind && left.mediaType === right.mediaType && left.byteLength === right.byteLength && left.sha256 === right.sha256
}

export async function writeTf2Release(release: Tf2Release, catalogBytes: Uint8Array): Promise<void> {
  const checked = parseTf2Release(release)
  const parsedCatalog = parseResourceCatalogBytes(catalogBytes)
  if (parsedCatalog.application !== "tf2" || parsedCatalog.entries.length !== checked.targets.length) throw new Tf2ReleaseError("TF2 release catalog target table differs")
  for (const target of checked.targets) {
    if (!sameDescriptor(selectCatalogTarget(parsedCatalog, target.target).resources, target.objects.resources)) throw new Tf2ReleaseError(`TF2 release ${target.target} resource root differs from catalog`)
  }
  await mkdir(RELEASE_DIRECTORY, { recursive: true })
  const releaseTemporary = `${RELEASE_PATH}.${process.pid}.tmp`
  const catalogTemporary = `${CATALOG_PATH}.${process.pid}.tmp`
  try {
    await Promise.all([
      writeFile(releaseTemporary, `${JSON.stringify(checked, null, 2)}\n`, { flag: "wx" }),
      writeFile(catalogTemporary, catalogBytes, { flag: "wx" }),
    ])
    await rename(catalogTemporary, CATALOG_PATH)
    await rename(releaseTemporary, RELEASE_PATH)
  } finally {
    await Promise.all([rm(releaseTemporary, { force: true }), rm(catalogTemporary, { force: true })])
  }
}

export async function readTf2Release(target: string | undefined): Promise<Tf2Release> {
  if (target !== undefined) throw new Tf2ReleaseError("the current TF2 release does not accept a target argument")
  try { return parseTf2Release(JSON.parse(await readFile(RELEASE_PATH, "utf8"))) }
  catch (error) {
    if (error instanceof Tf2ReleaseError) throw error
    throw new Tf2ReleaseError(error instanceof Error ? error.message : "TF2 release descriptor could not be read")
  }
}

export function releaseObjectPath(config: LocalConfig, expected: ObjectDescriptor): string { return objectPath(config.assetDir, expected.sha256) }

export async function verifyFile(pathname: string, expected: ObjectDescriptor): Promise<void> {
  const metadata = await stat(pathname)
  if (!metadata.isFile() || String(metadata.size) !== expected.byteLength) throw new Tf2ReleaseError(`object ${expected.sha256} byte length differs`)
  const hash = new Bun.CryptoHasher("sha256")
  for await (const chunk of createReadStream(pathname, { highWaterMark: 1024 * 1024 })) hash.update(chunk as Uint8Array)
  if (hash.digest("hex") !== expected.sha256) throw new Tf2ReleaseError(`object ${expected.sha256} hash differs`)
}
