import { mkdir, readFile, rename, rm, stat, writeFile } from "node:fs/promises"
import path from "node:path"
import { AssetStoreError, descriptor, objectPath, putObject, verifyObject, type ObjectDescriptor } from "@playsrc/asset-store"
import { canonicalGraphBytes, parseResourceCatalog, resourceChunkObject } from "@playsrc/asset-store/graph"
import { startAssetService } from "@playsrc/assets-service"
import { createServer, type ViteDevServer } from "vite"
import type { LocalConfig } from "./config"
import { repositoryRoot } from "./config"
import { acquireMap } from "./targets"
import { buildTf2Wasm } from "./tf2-wasm-build"
import { buildSourceBundle, prepareSourceBundleProducer } from "./source-bundle"
import { applicationBuildIdentity, invalidateRustBuildIdentity } from "./build-identity"
import { createDevelopmentBuildCoherence } from "./development-coherence"
import { TF2_CONFIGURED_STARTUP } from "@playsrc/game-tf2-browser/startup-presentation"
import { tf2MapLoading, TF2_STAMP_BACKGROUND } from "@playsrc/game-tf2-browser/loading-presentation"
import { TF2_DEVELOPMENT_TARGET_NAMES, type Tf2TargetName } from "@playsrc/game-tf2-browser/maps"

const APPLICATION_PORT = Number(process.env.PLAYSRC_DEV_PORT ?? "4173")
if (!Number.isSafeInteger(APPLICATION_PORT) || APPLICATION_PORT < 1024 || APPLICATION_PORT >= 65535) {
  throw new Error("PLAYSRC_DEV_PORT must be an integer from 1024 through 65534")
}
const APPLICATION_URL = `http://127.0.0.1:${APPLICATION_PORT}/`
const ASSET_ORIGIN = `http://127.0.0.1:${APPLICATION_PORT + 1}`
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

type PreparedObjectStamp = Readonly<{ schema: "playsrc-prepared-object-v1"; root: string; sha256: string; byteLength: string; file: string }>

const preparedFileIdentity = (value: Awaited<ReturnType<typeof stat>>): string =>
  `${value.dev}:${value.ino}:${value.size}:${value.mtimeMs}:${value.ctimeMs}`

async function publishFile(config: LocalConfig, expected: ObjectDescriptor, pathname: string): Promise<void> {
  const destination = objectPath(config.assetDir, expected.sha256)
  const stampPath = path.join(config.sourceCacheDir, "prepared-content", "objects", expected.sha256.slice(0, 2), `${expected.sha256}.json`)
  try {
    const [metadata, text] = await Promise.all([stat(destination), readFile(stampPath, "utf8")])
    const stamp = JSON.parse(text) as PreparedObjectStamp
    if (metadata.isFile() && stamp.schema === "playsrc-prepared-object-v1" && stamp.root === config.assetDir
      && stamp.sha256 === expected.sha256 && stamp.byteLength === expected.byteLength
      && stamp.file === preparedFileIdentity(metadata)) return
  } catch (error) {
    if (!["ENOENT", "ENOTDIR"].includes((error as NodeJS.ErrnoException).code ?? "") && !(error instanceof SyntaxError)) throw error
  }
  try {
    await verifyObject(config.assetDir, expected)
  } catch (error) {
    if (!(error instanceof AssetStoreError) || error.code !== "MissingObject") throw error
    try {
      await putObject(config.assetDir, expected, await readFile(pathname))
    } catch (failure) {
      throw new Error(`development asset ${pathname}: ${failure instanceof Error ? failure.message : String(failure)}`)
    }
  }
  const stamp: PreparedObjectStamp = Object.freeze({
    schema: "playsrc-prepared-object-v1",
    root: config.assetDir,
    sha256: expected.sha256,
    byteLength: expected.byteLength,
    file: preparedFileIdentity(await stat(destination)),
  })
  await mkdir(path.dirname(stampPath), { recursive: true })
  const temporary = `${stampPath}.${process.pid}.${crypto.randomUUID()}.tmp`
  try {
    await writeFile(temporary, `${JSON.stringify(stamp)}\n`)
    await rename(temporary, stampPath)
  } finally {
    await rm(temporary, { force: true })
  }
}

export type DevelopmentOwner = Readonly<{
  url: string
  startup: Readonly<{
    mapMilliseconds: number
    buildMilliseconds: number
    wasmMilliseconds: number
    sourceProducerMilliseconds: number
    sourceBundles: readonly Readonly<{ target: string; prepareMilliseconds: number; publishMilliseconds: number }>[]
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
  if (!TF2_DEVELOPMENT_TARGET_NAMES.includes(targetIdentity as Tf2TargetName)) throw new DevelopmentError("BuildFailed", "development default target is undeclared")
  const maps = [Object.freeze({ name: targetIdentity as Tf2TargetName, map: await acquireMap(config, targetIdentity) })]
  const mapReady = performance.now()
  let wasmMilliseconds = 0
  let sourceProducerMilliseconds = 0
  const [wasmPath, initialApplicationBuild, { tf2ViteConfiguration }, sourceBundles] = await Promise.all([
    (async () => {
      const began = performance.now()
      const artifact = await buildTf2Wasm(config)
      wasmMilliseconds = Math.round(performance.now() - began)
      return artifact
    })(),
    applicationBuildIdentity(),
    import("../../../apps/web/tf2/vite.config"),
    (async () => {
      const began = performance.now()
      await prepareSourceBundleProducer(config)
      sourceProducerMilliseconds = Math.round(performance.now() - began)
      return Promise.all(maps.map(async ({ name }) => {
        const prepareStarted = performance.now()
        const sourceBundle = await buildSourceBundle(config, name)
        const prepareMilliseconds = Math.round(performance.now() - prepareStarted)
        const publishStarted = performance.now()
        await Promise.all([
          publishFile(config, sourceBundle.report.graphDescriptor, sourceBundle.graphPath),
          publishFile(config, sourceBundle.report.ledgerDescriptor, sourceBundle.ledgerPath),
          ...sourceBundle.graph.chunks.map((chunk) => publishFile(config, resourceChunkObject(chunk), path.join(sourceBundle.graphObjectDirectory, chunk.encodedSha256))),
        ])
        return Object.freeze({ name, sourceBundle, prepareMilliseconds, publishMilliseconds: Math.round(performance.now() - publishStarted) })
      }))
    })(),
  ])
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
        mapPhotoLocations: tf2MapLoading(name).photoLocations,
        mapPhoto: tf2MapLoading(name).photo,
        stampBackground: TF2_STAMP_BACKGROUND,
      }),
    })
  })
  const createCatalog = (catalogTargets = targets) => {
    const bytes = canonicalGraphBytes(parseResourceCatalog({
      application: "tf2",
      entries: catalogTargets.map(({ target, objects }) => ({ target, resources: objects.resources }))
        .toSorted((left, right) => left.target.localeCompare(right.target)),
      schema: "playsrc-resource-catalog-v1",
    }))
    return Object.freeze({ bytes, descriptor: descriptor("catalog", "application/vnd.playsrc.asset-catalog+json", bytes) })
  }
  let catalog = createCatalog()
  const wasmBytes = await readFile(wasmPath)
  let wasm = descriptor("derived-object", "application/octet-stream", wasmBytes)
  let applicationBuild = initialApplicationBuild
  const browserConfiguration = () => JSON.stringify({
    application: "tf2",
    applicationBuild,
    defaultTarget: targetIdentity,
    renderLevel: 0,
    assetOrigin: APPLICATION_URL.slice(0, -1),
    allowedExternalOrigins: ["https://allowed-host"],
    wasm,
    catalog: catalog.descriptor,
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
  process.env.PLAYSRC_BROWSER_CONFIG = browserConfiguration()
  let assets: ReturnType<typeof startAssetService> | undefined
  let application: ViteDevServer | undefined
  let publicationMilliseconds = 0
  let viteCreationMilliseconds = 0
  let closed = false
  const coherence = createDevelopmentBuildCoherence(applicationBuild, applicationBuildIdentity, async (identity) => {
    const replacementStarted = performance.now()
    invalidateRustBuildIdentity()
    const [replacementWasmPath] = await Promise.all([
      buildTf2Wasm(config),
      prepareSourceBundleProducer(config),
    ])
    const replacementWasmBytes = await readFile(replacementWasmPath)
    const replacementWasm = descriptor("derived-object", "application/octet-stream", replacementWasmBytes)
    const replacements = await Promise.all(targets.map(async (previous) => {
      const sourceBundle = await buildSourceBundle(config, previous.target)
      await Promise.all([
        publishFile(config, sourceBundle.report.graphDescriptor, sourceBundle.graphPath),
        publishFile(config, sourceBundle.report.ledgerDescriptor, sourceBundle.ledgerPath),
        ...sourceBundle.graph.chunks.map((chunk) => publishFile(config, resourceChunkObject(chunk), path.join(sourceBundle.graphObjectDirectory, chunk.encodedSha256))),
      ])
      return Object.freeze({
        ...previous,
        contentBuild: sourceBundle.report.contentBuild,
        objects: Object.freeze({
          ...previous.objects,
          resources: sourceBundle.report.graphDescriptor,
          dependencyLedger: sourceBundle.report.ledgerDescriptor,
        }),
      })
    }))
    await putObject(config.assetDir, replacementWasm, replacementWasmBytes)
    const replacementCatalog = createCatalog(replacements)
    await putObject(config.assetDir, replacementCatalog.descriptor, replacementCatalog.bytes)
    return () => {
      targets.splice(0, targets.length, ...replacements)
      catalog = replacementCatalog
      wasm = replacementWasm
      applicationBuild = identity
      process.env.PLAYSRC_BROWSER_CONFIG = browserConfiguration()
      if (application) {
        application.moduleGraph.invalidateAll()
      }
      application?.ws.send({ type: "full-reload" })
      console.error(`playsrc dev build replaced applicationBuild=${identity} wasm=${wasm.sha256} milliseconds=${Math.round(performance.now() - replacementStarted)}`)
    }
  })
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
      putObject(config.assetDir, catalog.descriptor, catalog.bytes),
      ...maps.map(({ map }, index) => publishFile(
        config,
        targets[index]!.objects.bsp,
        path.join(config.sourceCacheDir, map.decoded.cachePath),
      )),
    ])
    publicationMilliseconds = Math.round(performance.now() - publicationStarted)
    const viteStarted = performance.now()
    const vite = tf2ViteConfiguration(ASSET_ORIGIN, false, coherence.ensure)
    application = await createServer({
      ...vite,
      plugins: [
        ...(vite.plugins ?? []),
        {
          name: "playsrc-profile-development-owner",
          configureServer(server) {
            let preparation: Promise<void> = Promise.resolve()
            server.middlewares.use("/__playsrc/prepare-target", (request, response, next) => {
              const name = request.url?.replace(/^\//u, "").split("?")[0]
              if (request.method !== "POST" || !TF2_DEVELOPMENT_TARGET_NAMES.includes(name as Tf2TargetName)) {
                next()
                return
              }
              const perform = async () => {
                const started = performance.now()
                if (!targets.some((candidate) => candidate.target === name)) {
                  const mapStarted = performance.now()
                  const map = await acquireMap(config, name)
                  const mapMilliseconds = Math.round(performance.now() - mapStarted)
                  const bundleStarted = performance.now()
                  const sourceBundle = await buildSourceBundle(config, name!)
                  const bundleMilliseconds = Math.round(performance.now() - bundleStarted)
                  const publishStarted = performance.now()
                  const target = Object.freeze({
                    target: name as Tf2TargetName,
                    contentBuild: sourceBundle.report.contentBuild,
                    objects: Object.freeze({
                      bsp: Object.freeze({ kind: "source-object" as const, mediaType: "application/octet-stream", byteLength: String(map.decoded.byteLength), sha256: map.decoded.sha256 }),
                      resources: sourceBundle.report.graphDescriptor,
                      dependencyLedger: sourceBundle.report.ledgerDescriptor,
                    }),
                    loading: Object.freeze({
                      mapPhotoLocations: tf2MapLoading(name).photoLocations,
                      mapPhoto: tf2MapLoading(name).photo,
                      stampBackground: TF2_STAMP_BACKGROUND,
                    }),
                  })
                  await Promise.all([
                    publishFile(config, target.objects.bsp, path.join(config.sourceCacheDir, map.decoded.cachePath)),
                    publishFile(config, sourceBundle.report.graphDescriptor, sourceBundle.graphPath),
                    publishFile(config, sourceBundle.report.ledgerDescriptor, sourceBundle.ledgerPath),
                    ...sourceBundle.graph.chunks.map((chunk) => publishFile(config, resourceChunkObject(chunk), path.join(sourceBundle.graphObjectDirectory, chunk.encodedSha256))),
                  ])
                  targets.push(target)
                  targets.sort((left, right) => TF2_DEVELOPMENT_TARGET_NAMES.indexOf(left.target) - TF2_DEVELOPMENT_TARGET_NAMES.indexOf(right.target))
                  catalog = createCatalog()
                  await putObject(config.assetDir, catalog.descriptor, catalog.bytes)
                  process.env.PLAYSRC_BROWSER_CONFIG = browserConfiguration()
                  server.moduleGraph.invalidateAll()
                  console.error(`playsrc dev target prepared target=${name} milliseconds=${Math.round(performance.now() - started)} mapMilliseconds=${mapMilliseconds} bundleMilliseconds=${bundleMilliseconds} publicationMilliseconds=${Math.round(performance.now() - publishStarted)}`)
                }
                response.statusCode = 200
                response.setHeader("content-type", "application/json; charset=utf-8")
                response.setHeader("cache-control", "no-store")
                response.end(browserConfiguration())
              }
              preparation = preparation.catch(() => undefined).then(perform)
              void preparation.catch((error) => {
                response.statusCode = 500
                response.setHeader("content-type", "application/problem+json")
                response.end(JSON.stringify({ title: error instanceof Error ? error.message : "Target preparation failed", status: 500 }))
              })
            })
            server.middlewares.use("/__playsrc/profile-owner", (_request, response) => {
              const identity = process.env.PLAYSRC_PROFILE_SOURCE_IDENTITY
              const token = process.env.PLAYSRC_PROFILE_OWNER_TOKEN
              if (!identity || !token) {
                response.statusCode = 404
                response.end()
                return
              }
              response.statusCode = 200
              response.setHeader("content-type", "application/json; charset=utf-8")
              response.setHeader("cache-control", "no-store")
              response.end(JSON.stringify({ schema: "playsrc-profile-owner-v1", repository: repositoryRoot, identity, token, target: targetIdentity }))
            })
          },
        },
      ],
      configFile: false,
      root: path.join(repositoryRoot, "apps", "web", "tf2"),
      logLevel: "error",
    })
    viteCreationMilliseconds = Math.round(performance.now() - viteStarted)
    const listenerStarted = performance.now()
    assets = startAssetService(config.assetDir, APPLICATION_PORT + 1)
    await application.listen()
    await Promise.all([waitReady(`${ASSET_ORIGIN}/readyz`), waitReady(APPLICATION_URL)])
    const ready = performance.now()
    return Object.freeze({
      url: APPLICATION_URL,
      startup: Object.freeze({
        mapMilliseconds: Math.round(mapReady - started),
        buildMilliseconds: Math.round(buildReady - mapReady),
        wasmMilliseconds,
        sourceProducerMilliseconds,
        sourceBundles: Object.freeze(sourceBundles.map(({ name, prepareMilliseconds, publishMilliseconds }) =>
          Object.freeze({ target: name, prepareMilliseconds, publishMilliseconds }))),
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
