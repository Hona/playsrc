import { createReadStream } from "node:fs"
import { mkdir, readFile, rename, rm, stat, writeFile } from "node:fs/promises"
import path from "node:path"
import { AssetStoreError, descriptor, objectPath, putObject, verifyObject, type ObjectDescriptor } from "@playsrc/asset-store"
import { canonicalGraphBytes, parseResourceCatalog, resourceChunkObject, selectCatalogTarget } from "@playsrc/asset-store/graph"
import { parseTf2Release, TF2_RELEASE_SCHEMA, type Tf2Release } from "../../../apps/web/tf2/src/deployment"
import type { LocalConfig } from "./config"
import { repositoryRoot } from "./config"
import { buildSourceBundle } from "./source-bundle"
import { acquireMap } from "./targets"
import { buildTf2Wasm } from "./tf2-wasm-build"

const RELEASE_PATH = path.join(repositoryRoot, "apps", "web", "tf2", "releases", "jump_beef.json")
const CATALOG_PATH = path.join(repositoryRoot, "apps", "web", "tf2", "releases", "catalog.json")

export type Tf2ReleaseArtifact = Readonly<{
  release: Tf2Release
  files: ReadonlyMap<string, Readonly<{ descriptor: ObjectDescriptor; pathname: string }>>
}>

export class Tf2ReleaseError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "Tf2ReleaseError"
  }
}

async function ensureLocalObject(root: string, expected: ObjectDescriptor, pathname: string): Promise<void> {
  try {
    await verifyObject(root, expected)
    return
  } catch (error) {
    if (!(error instanceof AssetStoreError) || error.code !== "MissingObject") throw error
  }
  await putObject(root, expected, await readFile(pathname))
}

export async function prepareTf2Release(config: LocalConfig, target: string | undefined): Promise<Tf2ReleaseArtifact> {
  if (target !== "jump_beef") throw new Tf2ReleaseError("release target must be jump_beef")
  const map = await acquireMap(config, target)
  const [wasmPath, sourceBundle] = await Promise.all([
    buildTf2Wasm(config),
    buildSourceBundle(config, target),
  ])
  const wasmBytes = await readFile(wasmPath)
  const catalog = parseResourceCatalog(JSON.parse(await readFile(CATALOG_PATH, "utf8")))
  const catalogBytes = canonicalGraphBytes(catalog)
  const catalogArtifactPath = path.join(config.sourceCacheDir, "browser-bundles", "catalog.json")
  await mkdir(path.dirname(catalogArtifactPath), { recursive: true })
  await writeFile(catalogArtifactPath, catalogBytes)
  const catalogResources = selectCatalogTarget(catalog, target).resources
  const generatedResources = sourceBundle.report.graphDescriptor
  if (catalogResources.kind !== generatedResources.kind
    || catalogResources.mediaType !== generatedResources.mediaType
    || catalogResources.byteLength !== generatedResources.byteLength
    || catalogResources.sha256 !== generatedResources.sha256) {
    throw new Tf2ReleaseError("catalog resource root differs from generated target")
  }
  const release = parseTf2Release({
    schema: TF2_RELEASE_SCHEMA,
    target,
    contentBuild: sourceBundle.report.contentBuild,
    objects: {
      bsp: {
        kind: "source-object",
        mediaType: "application/octet-stream",
        byteLength: String(map.decoded.byteLength),
        sha256: map.decoded.sha256,
      },
      wasm: descriptor("derived-object", "application/octet-stream", wasmBytes),
      catalog: descriptor("catalog", "application/vnd.playsrc.asset-catalog+json", catalogBytes),
      dependencyLedger: sourceBundle.report.ledgerDescriptor,
    },
  })
  const files = new Map<string, Readonly<{ descriptor: ObjectDescriptor; pathname: string }>>([
    [release.objects.bsp.sha256, { descriptor: release.objects.bsp, pathname: path.join(config.sourceCacheDir, map.decoded.cachePath) }],
    [release.objects.wasm.sha256, { descriptor: release.objects.wasm, pathname: wasmPath }],
    [release.objects.catalog.sha256, { descriptor: release.objects.catalog, pathname: catalogArtifactPath }],
    [sourceBundle.report.graphDescriptor.sha256, { descriptor: sourceBundle.report.graphDescriptor, pathname: sourceBundle.graphPath }],
    [release.objects.dependencyLedger.sha256, { descriptor: release.objects.dependencyLedger, pathname: sourceBundle.ledgerPath }],
  ])
  for (const chunk of sourceBundle.graph.chunks) {
    const chunkDescriptor = resourceChunkObject(chunk)
    files.set(chunkDescriptor.sha256, { descriptor: chunkDescriptor, pathname: path.join(sourceBundle.graphObjectDirectory, chunkDescriptor.sha256) })
  }
  await Promise.all(
    Array.from(files.values(), ({ descriptor, pathname }) => ensureLocalObject(config.assetDir, descriptor, pathname)),
  )
  await writeTf2Release(release)
  return Object.freeze({ release, files })
}

export async function writeTf2Release(release: Tf2Release): Promise<void> {
  const checked = parseTf2Release(release)
  await mkdir(path.dirname(RELEASE_PATH), { recursive: true })
  const temporary = `${RELEASE_PATH}.${process.pid}.tmp`
  try {
    await writeFile(temporary, `${JSON.stringify(checked, null, 2)}\n`, { flag: "wx" })
    await rm(RELEASE_PATH, { force: true })
    await rename(temporary, RELEASE_PATH)
  } finally {
    await rm(temporary, { force: true })
  }
}

export async function readTf2Release(target: string | undefined): Promise<Tf2Release> {
  if (target !== "jump_beef") throw new Tf2ReleaseError("release target must be jump_beef")
  try {
    return parseTf2Release(JSON.parse(await readFile(RELEASE_PATH, "utf8")))
  } catch (error) {
    if (error instanceof Tf2ReleaseError) throw error
    throw new Tf2ReleaseError(error instanceof Error ? error.message : "TF2 release descriptor could not be read")
  }
}

export function releaseObjectPath(config: LocalConfig, descriptor: ObjectDescriptor): string {
  return objectPath(config.assetDir, descriptor.sha256)
}

export async function verifyFile(pathname: string, expected: ObjectDescriptor): Promise<void> {
  const metadata = await stat(pathname)
  if (!metadata.isFile() || String(metadata.size) !== expected.byteLength) {
    throw new Tf2ReleaseError(`object ${expected.sha256} byte length differs`)
  }
  const hash = new Bun.CryptoHasher("sha256")
  for await (const chunk of createReadStream(pathname, { highWaterMark: 1024 * 1024 })) hash.update(chunk as Uint8Array)
  if (hash.digest("hex") !== expected.sha256) throw new Tf2ReleaseError(`object ${expected.sha256} hash differs`)
}
