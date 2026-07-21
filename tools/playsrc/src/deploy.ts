import { copyFile, readdir, rm, stat, writeFile } from "node:fs/promises"
import path from "node:path"
import { createDeployedBrowserConfiguration, parseTf2Release, TF2_APPLICATION_ORIGIN } from "../../../apps/web/tf2/src/deployment"
import { applyCloudflareInfrastructure, validateCloudflareInfrastructure } from "./cloudflare-infra"
import { CLOUDFLARE_ASSET_ORIGIN, runWrangler, WRANGLER_CONFIG } from "./cloudflare"
import { repositoryRoot } from "./config"
import { readTf2Release } from "./tf2-release"
import { parseResourceCatalogBytes, parseResourceGraphBytes, resourceChunkObject, selectCatalogTarget } from "@playsrc/asset-store/graph"
import type { ObjectDescriptor } from "@playsrc/asset-store"

const APP_DIRECTORY = path.join(repositoryRoot, "apps", "web", "tf2")
const DIST_DIRECTORY = path.join(APP_DIRECTORY, "dist", "cloudflare")
const READY_TIMEOUT_MILLISECONDS = 10 * 60 * 1_000

export class DeploymentError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "DeploymentError"
  }
}

async function applicationBuildIdentity(): Promise<string> {
  const child = Bun.spawn(["git", "rev-parse", "HEAD"], {
    cwd: repositoryRoot,
    stdout: "pipe",
    stderr: "pipe",
  })
  const value = (await new Response(child.stdout).text()).trim()
  if ((await child.exited) !== 0 || !/^[0-9a-f]{40}$/.test(value)) {
    throw new DeploymentError("public application commit identity is unavailable")
  }
  return new Bun.CryptoHasher("sha256").update(value).digest("hex")
}

export async function buildStaticSite(target: string | undefined): Promise<string> {
  const release = await readTf2Release(target)
  const applicationBuild = await applicationBuildIdentity()
  await rm(DIST_DIRECTORY, { recursive: true, force: true })
  const child = Bun.spawn([process.execPath, "run", "build"], {
    cwd: APP_DIRECTORY,
    env: process.env,
    stdout: "inherit",
    stderr: "inherit",
  })
  if (await child.exited !== 0) throw new DeploymentError("TF2 static application build failed")
  const configuration = createDeployedBrowserConfiguration(release, applicationBuild)
  await Promise.all([
    copyFile(path.join(repositoryRoot, "apps", "web", "index.html"), path.join(DIST_DIRECTORY, "index.html")),
    copyFile(path.join(repositoryRoot, "apps", "web", "404.html"), path.join(DIST_DIRECTORY, "404.html")),
    copyFile(path.join(repositoryRoot, "apps", "web", "_headers"), path.join(DIST_DIRECTORY, "_headers")),
    writeFile(path.join(DIST_DIRECTORY, "tf2", "playsrc-config.json"), `${JSON.stringify(configuration)}\n`),
    writeFile(path.join(DIST_DIRECTORY, "release.json"), `${JSON.stringify({
      schema: "playsrc-cloudflare-deployment-v1",
      application: "tf2",
      target: release.target,
      applicationBuild,
      applicationOrigin: TF2_APPLICATION_ORIGIN,
      assetOrigin: CLOUDFLARE_ASSET_ORIGIN,
      release,
    }, null, 2)}\n`),
  ])
  await verifyStaticTree()
  return applicationBuild
}

async function verifyStaticTree(): Promise<void> {
  for (const relative of ["index.html", "404.html", "_headers", "release.json", "tf2/index.html", "tf2/playsrc-config.json"]) {
    const metadata = await stat(path.join(DIST_DIRECTORY, relative))
    if (!metadata.isFile() || metadata.size < 1) throw new DeploymentError(`static deployment file ${relative} is unavailable`)
  }
  const entries = await readdir(path.join(DIST_DIRECTORY, "tf2", "assets"))
  if (!entries.some((entry) => entry.endsWith(".js")) || !entries.some((entry) => entry.endsWith(".css"))) {
    throw new DeploymentError("TF2 static deployment assets are incomplete")
  }
}

async function verifyRemoteObjects(target: string | undefined): Promise<void> {
  const release = await readTf2Release(target)
  const deadline = Date.now() + READY_TIMEOUT_MILLISECONDS
  let last = "asset origin did not respond"
  while (Date.now() < deadline) {
    try {
      const readObject = async (descriptor: ObjectDescriptor): Promise<Uint8Array> => {
        const response = await fetch(`${CLOUDFLARE_ASSET_ORIGIN}/objects/sha256/${descriptor.sha256}`, {
          method: "GET",
          headers: { origin: TF2_APPLICATION_ORIGIN },
          redirect: "error",
        })
        if (response.status !== 200 || response.headers.get("content-length") !== descriptor.byteLength) {
          throw new DeploymentError(`remote object ${descriptor.sha256} response differs`)
        }
        const bytes = new Uint8Array(await response.arrayBuffer())
        if (String(bytes.byteLength) !== descriptor.byteLength || new Bun.CryptoHasher("sha256").update(bytes).digest("hex") !== descriptor.sha256) {
          throw new DeploymentError(`remote object ${descriptor.sha256} bytes differ`)
        }
        return bytes
      }
      const catalogBytes = await readObject(release.objects.catalog)
      const catalog = parseResourceCatalogBytes(catalogBytes)
      const resources = selectCatalogTarget(catalog, release.target).resources
      const graph = parseResourceGraphBytes(await readObject(resources))
      if (graph.target !== release.target || graph.contentBuild !== release.contentBuild) throw new DeploymentError("remote resource graph identity differs")
      const closure = [...Object.values(release.objects), resources, ...graph.chunks.map(resourceChunkObject)]
      const unique = new Map(closure.map((descriptor) => [descriptor.sha256, descriptor]))
      let ready = true
      for (const descriptor of unique.values()) {
        const response = await fetch(`${CLOUDFLARE_ASSET_ORIGIN}/objects/sha256/${descriptor.sha256}`, {
          method: "HEAD",
          headers: { origin: TF2_APPLICATION_ORIGIN },
          redirect: "error",
        })
        if (response.status === 404) throw new DeploymentError(`remote object ${descriptor.sha256} is absent`)
        if (
          response.status === 200
          && (
            response.headers.get("content-length") !== descriptor.byteLength
            || response.headers.get("etag") === null
            || response.headers.get("access-control-allow-origin") !== TF2_APPLICATION_ORIGIN
          )
        ) throw new DeploymentError(`remote object ${descriptor.sha256} metadata differs`)
        if (response.status !== 200) {
          ready = false
          last = `remote object ${descriptor.sha256} returned malformed metadata`
          break
        }
      }
      if (ready) return
    } catch (error) {
      if (error instanceof DeploymentError) throw error
      last = error instanceof Error ? error.message : "asset-origin probe failed"
    }
    await Bun.sleep(2_000)
  }
  throw new DeploymentError(`asset origin did not become ready within 600000 ms: ${last}`)
}

async function waitForDeployment(target: string | undefined, applicationBuild: string): Promise<void> {
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
        const configuration = createDeployedBrowserConfiguration(
          parseTf2Release((await readTf2Release(target))),
          applicationBuild,
        )
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

export async function deployCloudflare(target: string | undefined): Promise<void> {
  await applyCloudflareInfrastructure()
  await verifyRemoteObjects(target)
  const applicationBuild = await buildStaticSite(target)
  const result = await runWrangler(["deploy", `--config=${WRANGLER_CONFIG}`])
  if (result.code !== 0) throw new DeploymentError(`Wrangler deployment failed: ${result.stderr.trim()}`)
  await waitForDeployment(target, applicationBuild)
  console.log(JSON.stringify({ target, applicationBuild, url: `${TF2_APPLICATION_ORIGIN}/tf2` }))
}
