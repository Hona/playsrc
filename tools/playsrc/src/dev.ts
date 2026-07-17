import { readFile } from "node:fs/promises"
import path from "node:path"
import { descriptor, putObject } from "@playsrc/asset-store"
import { startAssetService } from "@playsrc/assets-service"
import { createServer, type ViteDevServer } from "vite"
import type { LocalConfig } from "./config"
import { repositoryRoot } from "./config"
import { acquireMap } from "./targets"
import { buildTf2Wasm } from "./verify-tf2-wasm"
import { buildSourceBundle } from "./source-bundle"

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
  if (await child.exited !== 0 || !/^[0-9a-f]{40}$/.test(value)) {
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

export type DevelopmentOwner = Readonly<{
  url: string
  close(): Promise<void>
  waitForInterrupt(): Promise<"SIGINT" | "SIGTERM">
}>

export async function startDevelopment(
  config: LocalConfig,
  target: string | undefined,
): Promise<DevelopmentOwner> {
  const map = await acquireMap(config, target)
  const wasmPath = await buildTf2Wasm(config)
  const bundlePath = await buildSourceBundle(config, target ?? "")
  const [bspBytes, wasmBytes, dependencyBytes, applicationBuild] = await Promise.all([
    readFile(path.join(config.sourceCacheDir, map.decoded.cachePath)),
    readFile(wasmPath),
    readFile(bundlePath),
    publicCommitIdentity(),
  ])
  const bsp = descriptor("source-object", "application/octet-stream", bspBytes)
  const wasm = descriptor("derived-object", "application/octet-stream", wasmBytes)
  const dependencies = descriptor("derived-object", "application/octet-stream", dependencyBytes)
  await putObject(config.assetDir, bsp, bspBytes)
  await putObject(config.assetDir, wasm, wasmBytes)
  await putObject(config.assetDir, dependencies, dependencyBytes)
  const browserConfiguration = JSON.stringify({
    application: "tf2",
    applicationBuild,
    target: "jump_beef",
    assetOrigin: APPLICATION_URL.slice(0, -1),
    allowedExternalOrigins: ["https://allowed-host"],
    bsp,
    wasm,
    dependencies,
  })
  const previousAssetOrigin = process.env.PLAYSRC_ASSET_ORIGIN
  const previousBrowserConfiguration = process.env.PLAYSRC_BROWSER_CONFIG
  process.env.PLAYSRC_ASSET_ORIGIN = ASSET_ORIGIN
  process.env.PLAYSRC_BROWSER_CONFIG = browserConfiguration
  let assets: ReturnType<typeof startAssetService> | undefined
  let application: ViteDevServer | undefined
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
    assets = startAssetService(config.assetDir, 4174)
    application = await createServer({
      configFile: path.join(repositoryRoot, "apps", "web", "tf2", "vite.config.ts"),
      root: path.join(repositoryRoot, "apps", "web", "tf2"),
      logLevel: "error",
    })
    await application.listen()
    await Promise.all([
      waitReady(`${ASSET_ORIGIN}/readyz`),
      waitReady(APPLICATION_URL),
    ])
  } catch (error) {
    await close().catch(() => {})
    if (error instanceof DevelopmentError) throw error
    throw new DevelopmentError("ProcessFailure", error instanceof Error ? error.message : "development startup failed")
  }
  return Object.freeze({
    url: APPLICATION_URL,
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
}

export async function runDevelopment(config: LocalConfig, target: string | undefined): Promise<void> {
  const owner = await startDevelopment(config, target)
  console.log(owner.url)
  await owner.waitForInterrupt()
  await owner.close()
}
