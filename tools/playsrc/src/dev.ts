import { readFile } from "node:fs/promises"
import path from "node:path"
import { AssetStoreError, descriptor, putObject, verifyObject, type ObjectDescriptor } from "@playsrc/asset-store"
import { canonicalGraphBytes, parseResourceCatalog, resourceChunkObject } from "@playsrc/asset-store/graph"
import { startAssetService } from "@playsrc/assets-service"
import { createServer, type ViteDevServer } from "vite"
import type { LocalConfig } from "./config"
import { repositoryRoot } from "./config"
import { acquireMap } from "./targets"
import { buildTf2Wasm } from "./tf2-wasm-build"
import { buildSourceBundle } from "./source-bundle"
import { TF2_CONFIGURED_STARTUP } from "@playsrc/game-tf2-browser/startup-presentation"
import { TF2_JUMP_BEEF_MAP_PHOTO_LOCATIONS, TF2_PL_UPWARD_MAP_PHOTO_LOCATIONS, TF2_STAMP_BACKGROUND } from "@playsrc/game-tf2-browser/loading-presentation"
import { TF2_TARGET_NAMES, type Tf2TargetName } from "../../../apps/web/tf2/src/deployment"

const APPLICATION_URL = "http://127.0.0.1:4173/"
const ASSET_ORIGIN = "http://127.0.0.1:4174"
const READY_TIMEOUT_MS = 120_000

export class DevelopmentError extends Error {
  constructor(
    readonly code: "BuildFailed" | "ProcessFailure" | "ReadinessFailure" | "CleanupFailure",
    message: string,
  ) {
    super(message)
    this.name = "DevelopmentError"
  }
}

async function publicCommitIdentity(): Promise<string> {
  const child = Bun.spawn(["git", "rev-parse", "HEAD"], {
    cwd: repositoryRoot,
    stdout: "pipe",
    stderr: "ignore",
  })
  const value = (await new Response(child.stdout).text()).trim()
  if ((await child.exited) !== 0 || !/^[0-9a-f]{40}$/.test(value)) {
    throw new DevelopmentError("BuildFailed", "public application commit identity is unavailable")
  }
  return new Bun.CryptoHasher("sha256").update(value).digest("hex")
}

async function waitReady(url: string): Promise<void> {
  const deadline = Date.now() + READY_TIMEOUT_MS
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url, { cache: "no-store", redirect: "error" })
      if (response.status === 200) return
    } catch {}
    await Bun.sleep(100)
  }
  throw new DevelopmentError("ReadinessFailure", `${url} did not become ready within 120000 ms`)
}

async function publishFile(root: string, expected: ObjectDescriptor, pathname: string): Promise<void> {
  try {
    await verifyObject(root, expected)
    return
  } catch (error) {
    if (!(error instanceof AssetStoreError) || error.code !== "MissingObject") throw error
  }
  await putObject(root, expected, await readFile(pathname))
}

export type DevelopmentOwner = Readonly<{
  url: string
  startup: Readonly<{
    mapMilliseconds: number
    buildMilliseconds: number
    publicationMilliseconds: number
    viteCreationMilliseconds: number
    listenerMilliseconds: number
    totalMilliseconds: number
  }>
  close(): Promise<void>
  waitForInterrupt(): Promise<"SIGINT" | "SIGTERM">
}>

export async function startDevelopment(config: LocalConfig, target: string | undefined): Promise<DevelopmentOwner> {
  const started = performance.now()
  const targetIdentity = target ?? "jump_beef"
  if (!TF2_TARGET_NAMES.includes(targetIdentity as Tf2TargetName)) throw new DevelopmentError("BuildFailed", "development default target is undeclared")
  const maps = await Promise.all(TF2_TARGET_NAMES.map(async (name) => Object.freeze({ name, map: await acquireMap(config, name) })))
  const mapReady = performance.now()
  const concurrent = Promise.all([buildTf2Wasm(config), publicCommitIdentity(), import("../../../apps/web/tf2/vite.config")])
  const sourceBundles = [] as Array<Readonly<{ name: (typeof TF2_TARGET_NAMES)[number]; sourceBundle: Awaited<ReturnType<typeof buildSourceBundle>> }>>
  for (const name of TF2_TARGET_NAMES) sourceBundles.push(Object.freeze({ name, sourceBundle: await buildSourceBundle(config, name) }))
  const [wasmPath, applicationBuild, { tf2ViteConfiguration }] = await concurrent
  const buildReady = performance.now()
  const targets = maps.map(({ name, map }, index) => {
    const sourceBundle = sourceBundles[index].sourceBundle
    return Object.freeze({
      target: name,
      contentBuild: sourceBundle.report.contentBuild,
      objects: Object.freeze({
        bsp: Object.freeze({ kind: "source-object" as const, mediaType: "application/octet-stream", byteLength: String(map.decoded.byteLength), sha256: map.decoded.sha256 }),
        resources: sourceBundle.report.graphDescriptor,
        dependencyLedger: sourceBundle.report.ledgerDescriptor,
      }),
      loading: Object.freeze({
        mapPhotoLocations: name === "jump_beef" ? TF2_JUMP_BEEF_MAP_PHOTO_LOCATIONS : TF2_PL_UPWARD_MAP_PHOTO_LOCATIONS,
        stampBackground: TF2_STAMP_BACKGROUND,
      }),
    })
  })
  const catalogSource = { application: "tf2", entries: targets.map(({ target, objects }) => ({ target, resources: objects.resources })), schema: "playsrc-resource-catalog-v1" }
  const catalogBytes = canonicalGraphBytes(parseResourceCatalog(catalogSource))
  const catalog = descriptor("catalog", "application/vnd.playsrc.asset-catalog+json", catalogBytes)
  const wasmBytes = await readFile(wasmPath)
  const wasm = descriptor("derived-object", "application/octet-stream", wasmBytes)
  const browserConfiguration = JSON.stringify({
    application: "tf2",
    applicationBuild,
    defaultTarget: targetIdentity,
    renderLevel: 2,
    assetOrigin: APPLICATION_URL.slice(0, -1),
    allowedExternalOrigins: ["https://allowed-host"],
    wasm,
    catalog,
    targets,
    startup: TF2_CONFIGURED_STARTUP,
    presentation: {
      randomSeed: 0,
      activeHoliday: "none",
      activeWar: null,
      activeOperation: false,
      freeTrial: false,
    },
  })
  const previousAssetOrigin = process.env.PLAYSRC_ASSET_ORIGIN
  const previousBrowserConfiguration = process.env.PLAYSRC_BROWSER_CONFIG
  process.env.PLAYSRC_ASSET_ORIGIN = ASSET_ORIGIN
  process.env.PLAYSRC_BROWSER_CONFIG = browserConfiguration
  let assets: ReturnType<typeof startAssetService> | undefined
  let application: ViteDevServer | undefined
  let publicationMilliseconds = 0
  let viteCreationMilliseconds = 0
  let closed = false
  const restoreEnvironment = () => {
    if (previousAssetOrigin === undefined) delete process.env.PLAYSRC_ASSET_ORIGIN
    else process.env.PLAYSRC_ASSET_ORIGIN = previousAssetOrigin
    if (previousBrowserConfiguration === undefined) delete process.env.PLAYSRC_BROWSER_CONFIG
    else process.env.PLAYSRC_BROWSER_CONFIG = previousBrowserConfiguration
  }
  const close = async (): Promise<void> => {
    if (closed) return
    closed = true
    const failures: string[] = []
    try {
      await application?.ws.close()
      application?.httpServer?.closeAllConnections()
      await application?.close()
    } catch {
      failures.push("application")
    }
    try {
      assets?.stop(true)
    } catch {
      failures.push("assets")
    }
    restoreEnvironment()
    if (failures.length) {
      throw new DevelopmentError("CleanupFailure", `${failures.join(", ")} listener cleanup failed`)
    }
  }
  try {
    const publicationStarted = performance.now()
    await Promise.all([
      putObject(config.assetDir, wasm, wasmBytes),
      putObject(config.assetDir, catalog, catalogBytes),
      ...maps.flatMap(({ map }, index) => {
        const sourceBundle = sourceBundles[index].sourceBundle
        const configured = targets[index]
        return [
          publishFile(config.assetDir, configured.objects.bsp, path.join(config.sourceCacheDir, map.decoded.cachePath)),
          publishFile(config.assetDir, configured.objects.resources, sourceBundle.graphPath),
          publishFile(config.assetDir, configured.objects.dependencyLedger, sourceBundle.ledgerPath),
          ...sourceBundle.graph.chunks.map((chunk) => publishFile(config.assetDir, resourceChunkObject(chunk), path.join(sourceBundle.graphObjectDirectory, chunk.encodedSha256))),
        ]
      }),
    ])
    publicationMilliseconds = Math.round(performance.now() - publicationStarted)
    const viteStarted = performance.now()
    application = await createServer({
      ...tf2ViteConfiguration(ASSET_ORIGIN),
      configFile: false,
      root: path.join(repositoryRoot, "apps", "web", "tf2"),
      logLevel: "error",
    })
    viteCreationMilliseconds = Math.round(performance.now() - viteStarted)
    const listenerStarted = performance.now()
    assets = startAssetService(config.assetDir, 4174)
    await application.listen()
    await Promise.all([waitReady(`${ASSET_ORIGIN}/readyz`), waitReady(APPLICATION_URL)])
    const ready = performance.now()
    return Object.freeze({
      url: APPLICATION_URL,
      startup: Object.freeze({
        mapMilliseconds: Math.round(mapReady - started),
        buildMilliseconds: Math.round(buildReady - mapReady),
        publicationMilliseconds,
        viteCreationMilliseconds,
        listenerMilliseconds: Math.round(ready - listenerStarted),
        totalMilliseconds: Math.round(ready - started),
      }),
      close,
      waitForInterrupt(): Promise<"SIGINT" | "SIGTERM"> {
        if (closed) return Promise.reject(new DevelopmentError("ProcessFailure", "development owner is closed"))
        return new Promise((resolve) => {
          const finish = (signal: "SIGINT" | "SIGTERM") => {
            process.off("SIGINT", interrupt)
            process.off("SIGTERM", terminate)
            resolve(signal)
          }
          const interrupt = () => finish("SIGINT")
          const terminate = () => finish("SIGTERM")
          process.once("SIGINT", interrupt)
          process.once("SIGTERM", terminate)
        })
      },
    })
  } catch (error) {
    await close().catch(() => {})
    if (error instanceof DevelopmentError) throw error
    throw new DevelopmentError("ProcessFailure", error instanceof Error ? error.message : "development startup failed")
  }
}

export async function runDevelopment(config: LocalConfig, target: string | undefined): Promise<void> {
  const started = performance.now()
  const owner = await startDevelopment(config, target)
  console.error(`playsrc dev ready target=${target ?? ""} milliseconds=${Math.round(performance.now() - started)} processMilliseconds=${Math.round(process.uptime() * 1_000)} stages=${JSON.stringify(owner.startup)}`)
  console.log(owner.url)
  await owner.waitForInterrupt()
  await owner.close()
}
