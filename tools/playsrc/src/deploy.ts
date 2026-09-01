import { copyFile, readdir, readFile, rm, stat, writeFile } from "node:fs/promises"
import path from "node:path"
import { createDeployedBrowserConfiguration, parseTf2Release, TF2_APPLICATION_ORIGIN, type Tf2Release } from "../../../apps/web/tf2/src/deployment"
import { applyCloudflareInfrastructure, validateCloudflareInfrastructure } from "./cloudflare-infra"
import { CLOUDFLARE_ASSET_ORIGIN, runWrangler, WRANGLER_CONFIG } from "./cloudflare"
import { repositoryRoot } from "./config"
import { applicationBuildIdentity } from "./build-identity"
import { readTf2Release } from "./tf2-release"
import type { ObjectDescriptor } from "@playsrc/asset-store"
import type { BrowserConfiguration } from "../../../apps/web/tf2/src/config"
import { assertStaticBundleGeneration, STATIC_GENERATION_BUNDLE_PREFIXES } from "../../../apps/web/tf2/generation-plugin"
import { assertWasmBindings, captureWasmBindings } from "./wasm-bindings"
import { staticStartupPackage } from "../profile/static-startup-package"
import { assertStaticStartupReceipt } from "../profile/static-startup-gate"
import { isDeepStrictEqual } from "node:util"

const APP_DIRECTORY = path.join(repositoryRoot, "apps", "web", "tf2")
const DIST_DIRECTORY = path.join(APP_DIRECTORY, "dist", "cloudflare")
const READY_TIMEOUT_MILLISECONDS = 10 * 60 * 1_000

export class DeploymentError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "DeploymentError"
  }
}

export async function buildStaticSite(target: string | undefined, options: { approved?: boolean; candidate?: Tf2Release } = {}): Promise<string> {
  if (options.approved && options.candidate) throw new DeploymentError("An approved release cannot substitute a candidate descriptor")
  const sourceRelease = options.candidate ? parseTf2Release(options.candidate) : await readTf2Release(target)
  const applicationBuild = await applicationBuildIdentity()
  const bindingsDirectory = path.join(repositoryRoot, "games/tf2/browser/src/wasm-generated")
  const compiledWasm = await readFile(path.join(bindingsDirectory, "tf2_wasm_bg.wasm"))
  const compiledDescriptor: ObjectDescriptor = {
    kind: "derived-object", mediaType: "application/octet-stream", byteLength: String(compiledWasm.byteLength),
    sha256: new Bun.CryptoHasher("sha256").update(compiledWasm).digest("hex"),
  }
  if (options.approved) await assertWasmBindings(bindingsDirectory, sourceRelease.wasmBindings)
  const release = options.approved ? sourceRelease : parseTf2Release({ ...sourceRelease,
    objects: { ...sourceRelease.objects, wasm: compiledDescriptor },
    wasmBindings: await captureWasmBindings(bindingsDirectory, compiledDescriptor),
  })
  const configuration = createDeployedBrowserConfiguration(release, applicationBuild)
  await rm(DIST_DIRECTORY, { recursive: true, force: true })
  const child = Bun.spawn([process.execPath, "run", "build"], {
    cwd: APP_DIRECTORY,
    env: { ...process.env, PLAYSRC_APPLICATION_BUILD: undefined, PLAYSRC_BROWSER_CONFIG: JSON.stringify(configuration) },
    stdout: "inherit",
    stderr: "inherit",
  })
  if (await child.exited !== 0) throw new DeploymentError("TF2 static application build failed")
  await assertWasmBindings(bindingsDirectory, release.wasmBindings)
  if (!options.approved) await captureWasmBindings(bindingsDirectory, compiledDescriptor)
  await Promise.all([
    copyFile(path.join(repositoryRoot, "apps", "web", "index.html"), path.join(DIST_DIRECTORY, "index.html")),
    copyFile(path.join(repositoryRoot, "apps", "web", "404.html"), path.join(DIST_DIRECTORY, "404.html")),
    copyFile(path.join(repositoryRoot, "apps", "web", "_headers"), path.join(DIST_DIRECTORY, "_headers")),
    writeFile(path.join(DIST_DIRECTORY, "tf2", "playsrc-config.json"), `${JSON.stringify(configuration)}\n`),
    writeFile(path.join(DIST_DIRECTORY, "release.json"), `${JSON.stringify({
      schema: "playsrc-cloudflare-deployment-v1",
      application: "tf2",
      defaultTarget: release.defaultTarget,
      applicationBuild,
      applicationOrigin: TF2_APPLICATION_ORIGIN,
      assetOrigin: CLOUDFLARE_ASSET_ORIGIN,
      release,
    }, null, 2)}\n`),
  ])
  await verifyStaticTree(configuration)
  return applicationBuild
}

async function verifyStaticTree(configuration: BrowserConfiguration): Promise<void> {
  for (const relative of ["index.html", "404.html", "_headers", "release.json", "tf2/index.html", "tf2/playsrc-config.json"]) {
    const metadata = await stat(path.join(DIST_DIRECTORY, relative))
    if (!metadata.isFile() || metadata.size < 1) throw new DeploymentError(`static deployment file ${relative} is unavailable`)
  }
  const entries = await readdir(path.join(DIST_DIRECTORY, "tf2", "assets"))
  if (!entries.some((entry) => entry.endsWith(".js")) || !entries.some((entry) => entry.endsWith(".css"))) {
    throw new DeploymentError("TF2 static deployment assets are incomplete")
  }
  for (const prefix of STATIC_GENERATION_BUNDLE_PREFIXES) {
    const matches = entries.filter((entry) => entry.startsWith(prefix) && entry.endsWith(".js"))
    if (matches.length !== 1) {
      throw new DeploymentError(`TF2 ${prefix.slice(0, -1)} bundle application generation differs`)
    }
    assertStaticBundleGeneration(await readFile(path.join(DIST_DIRECTORY, "tf2", "assets", matches[0]!), "utf8"), configuration)
  }
}

export async function readRemoteReleaseObject(descriptor: ObjectDescriptor, fetcher: typeof fetch = fetch): Promise<Uint8Array> {
  const response = await fetcher(`${CLOUDFLARE_ASSET_ORIGIN}/objects/sha256/${descriptor.sha256}`, {
    method: "GET", headers: { origin: TF2_APPLICATION_ORIGIN }, redirect: "error", signal: AbortSignal.timeout(120_000),
  })
  if (response.status !== 200 || response.headers.get("content-length") !== descriptor.byteLength) throw new DeploymentError(`remote object ${descriptor.sha256} response differs (HTTP ${response.status})`)
  const bytes = new Uint8Array(await response.arrayBuffer())
  if (String(bytes.byteLength) !== descriptor.byteLength || new Bun.CryptoHasher("sha256").update(bytes).digest("hex") !== descriptor.sha256) throw new DeploymentError(`remote object ${descriptor.sha256} bytes differ`)
  return bytes
}

async function waitForDeployment(release: Tf2Release, applicationBuild: string): Promise<void> {
  const configuration = createDeployedBrowserConfiguration(release, applicationBuild)
  const deadline = Date.now() + READY_TIMEOUT_MILLISECONDS
  let last = "deployment did not respond"
  while (Date.now() < deadline) {
    try {
      const [root, tf2, configurationResponse] = await Promise.all([
        fetch(`${TF2_APPLICATION_ORIGIN}/`, { cache: "no-store", redirect: "error" }),
        fetch(`${TF2_APPLICATION_ORIGIN}/tf2`, { cache: "no-store", redirect: "error" }),
        fetch(`${TF2_APPLICATION_ORIGIN}/tf2/playsrc-config.json`, { cache: "no-store", redirect: "error" }),
      ])
      if (root.status !== 200 || tf2.status !== 200 || configurationResponse.status !== 200) {
        last = `route statuses were ${root.status}, ${tf2.status}, ${configurationResponse.status}`
      } else {
        if (JSON.stringify(await configurationResponse.json()) === JSON.stringify(configuration)) return
        last = "deployed browser configuration differs"
      }
    } catch (error) {
      last = error instanceof Error ? error.message : "deployment probe failed"
    }
    await Bun.sleep(2_000)
  }
  throw new DeploymentError(`production did not become ready within 600000 ms: ${last}`)
}

export async function verifyCloudflareDeployment(target: string | undefined): Promise<void> {
  await validateCloudflareInfrastructure()
  await buildStaticSite(target)
  const result = await runWrangler(["deploy", "--dry-run", `--config=${WRANGLER_CONFIG}`])
  if (result.code !== 0) throw new DeploymentError(`Wrangler dry run failed: ${result.stderr.trim()}`)
}

/** Deployment consumes accepted bytes. Compiling here invalidates cross-host receipts. */
export async function verifyPreparedRelease(target: string | undefined): Promise<Awaited<ReturnType<typeof staticStartupPackage>>> {
  const packaged = await staticStartupPackage(DIST_DIRECTORY)
  const release = await readTf2Release(target)
  const applicationBuild = await applicationBuildIdentity()
  assertPreparedReleaseIdentity(packaged, release, applicationBuild)
  await verifyStaticTree(packaged.configuration)
  assertReleaseStartupAcceptance({ packageSha256: packaged.sha256, wasmSha256: packaged.configuration.wasm.sha256 })
  return packaged
}

export function assertPreparedReleaseIdentity(packaged: Pick<Awaited<ReturnType<typeof staticStartupPackage>>, "release" | "configuration">, release: Tf2Release, applicationBuild: string): void {
  if (!isDeepStrictEqual(packaged.release, release)) throw new DeploymentError("Prepared package release descriptor differs from checked source")
  if (!isDeepStrictEqual(packaged.configuration, createDeployedBrowserConfiguration(release, applicationBuild))) throw new DeploymentError("Prepared package configuration/source build differs from checked source")
}

export async function deployCloudflare(target: string | undefined): Promise<void> {
  const packaged = await verifyPreparedRelease(target)
  const applicationBuild = packaged.configuration.applicationBuild
  // Complete immutable publication/readback is owned by the publisher, not repeated during delivery.
  if ((await staticStartupPackage(DIST_DIRECTORY)).sha256 !== packaged.sha256) throw new DeploymentError("Static package changed after startup acceptance")
  await applyCloudflareInfrastructure()
  if ((await staticStartupPackage(DIST_DIRECTORY)).sha256 !== packaged.sha256) throw new DeploymentError("Static package changed before deployment")
  const result = await runWrangler(["deploy", `--config=${WRANGLER_CONFIG}`])
  if (result.code !== 0) throw new DeploymentError(`Wrangler deployment failed: ${result.stderr.trim()}`)
  await waitForDeployment(packaged.release, applicationBuild)
  console.log(JSON.stringify({ target, applicationBuild, url: `${TF2_APPLICATION_ORIGIN}/tf2` }))
}

export function assertReleaseStartupAcceptance(expected: { packageSha256: string; wasmSha256: string }, environment: NodeJS.ProcessEnv = process.env): void {
  if (environment.PLAYSRC_RELEASE_VERSION === "0.0.12" && environment.PLAYSRC_WAIVE_V0012_STARTUP_RECEIPT === "true") {
    console.warn("User-authorized v0.0.12 startup receipt waiver: exact-package headed startup is not certified; asset, binding and package checks remain mandatory")
    return
  }
  let startup: unknown
  try { startup = JSON.parse(environment.PLAYSRC_STATIC_STARTUP_RECEIPT ?? "") }
  catch { throw new DeploymentError("Approved release requires its exact headed static-package startup receipt") }
  const receipt = startup as { packageSha256?: unknown; wasmSha256?: unknown } | null
  const hash = (value: unknown) => typeof value === "string" && /^[0-9a-f]{64}$/.test(value) ? value : "absent-or-invalid"
  if (receipt?.packageSha256 !== expected.packageSha256 || receipt?.wasmSha256 !== expected.wasmSha256) {
    throw new DeploymentError(`Startup receipt identity mismatch: accepted package=${hash(receipt?.packageSha256)} observed package=${hash(expected.packageSha256)}; accepted WASM=${hash(receipt?.wasmSha256)} observed WASM=${hash(expected.wasmSha256)}`)
  }
  assertStaticStartupReceipt(startup, expected)
}

export function assertReleaseWasmInterface(compiled: Uint8Array, approved: Uint8Array): void {
  const contracts = [compiled, approved].map(bytes => {
    const module = new WebAssembly.Module(bytes)
    return {
      imports: WebAssembly.Module.imports(module),
      exports: WebAssembly.Module.exports(module).sort((a, b) => a.name < b.name ? -1 : a.name > b.name ? 1 : 0),
    }
  })
  if (JSON.stringify(contracts[0]) !== JSON.stringify(contracts[1])) {
    throw new DeploymentError("Approved WASM import/export contract differs from browser bindings")
  }
}
