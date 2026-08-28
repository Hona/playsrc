import { createHash } from "node:crypto"
import { readFile, readdir } from "node:fs/promises"
import path from "node:path"
import { parseBrowserConfiguration } from "../../../apps/web/tf2/src/config"
import { parseTf2Release, TF2_APPLICATION_ORIGIN } from "../../../apps/web/tf2/src/deployment"
import { assertStaticBundleGeneration } from "../../../apps/web/tf2/generation-plugin"
import { objectPath } from "@playsrc/asset-store"

export const startupDigest = (bytes: Uint8Array | string) => createHash("sha256").update(bytes).digest("hex")
export type StaticFile = Readonly<{ name: string; bytes: number; sha256: string }>

async function files(directory: string, prefix = ""): Promise<StaticFile[]> {
  const result: StaticFile[] = []
  for (const entry of await readdir(path.join(directory, prefix), { withFileTypes: true })) {
    const name = prefix + entry.name
    if (entry.isSymbolicLink()) throw new Error("Static startup package contains a symbolic link")
    if (entry.isDirectory()) result.push(...await files(directory, name + "/"))
    else if (entry.isFile()) {
      const bytes = await readFile(path.join(directory, name))
      result.push({ name, bytes: bytes.byteLength, sha256: startupDigest(bytes) })
    }
  }
  if (result.length > 256) throw new Error("Static startup package file bound exceeded")
  return result.sort((a, b) => a.name < b.name ? -1 : a.name > b.name ? 1 : 0)
}

/** The receipt identifies every exact packaged byte, not a source commit or a
 * freshly rebuilt development server. No package file may change after admission. */
export async function staticStartupPackage(directory: string) {
  const configuration = parseBrowserConfiguration(JSON.parse(await readFile(path.join(directory, "tf2/playsrc-config.json"), "utf8")), TF2_APPLICATION_ORIGIN)
  const deployment = JSON.parse(await readFile(path.join(directory, "release.json"), "utf8"))
  const release = parseTf2Release(deployment.release)
  if (deployment.applicationBuild !== configuration.applicationBuild || release.objects.wasm.sha256 !== configuration.wasm.sha256
    || release.objects.wasm.byteLength !== configuration.wasm.byteLength) throw new Error("Static release/configuration identity differs")
  const inventory = await files(directory)
  for (const prefix of ["index-", "gameplay-worker-"]) {
    const entries = inventory.filter(file => file.name.startsWith(`tf2/assets/${prefix}`) && file.name.endsWith(".js"))
    if (entries.length !== 1) throw new Error("Static startup entry identity is ambiguous")
    assertStaticBundleGeneration(await readFile(path.join(directory, entries[0]!.name), "utf8"), configuration)
  }
  return { schema: "playsrc-static-package-v1" as const, sha256: startupDigest(JSON.stringify(inventory)), files: inventory, configuration, release }
}

function headersFor(text: string, pathname: string): Record<string, string> {
  let active = false
  const headers: Record<string, string> = {}
  for (const line of text.split(/\r?\n/)) {
    if (!line.trim()) continue
    if (!/^\s/.test(line)) {
      active = line.endsWith("*") ? pathname.startsWith(line.slice(0, -1)) : pathname === line
    } else if (active) {
      const colon = line.indexOf(":")
      if (colon < 1) throw new Error("Static response header is malformed")
      headers[line.slice(0, colon).trim().toLowerCase()] = line.slice(colon + 1).trim()
    }
  }
  return headers
}

/** Serve a package at its real application origin through a browser's request
 * routing API. Only local verified bytes are fulfilled; no server is deployed.
 * A warm upgrade uses one retained previous HTML entry and its original assets,
 * while configuration and subsequent navigation belong to the new package. */
export async function staticStartupRouter(options: { directory: string; assetDir: string; wasmFile: string; previousDirectory: string }) {
  const admitted = await staticStartupPackage(options.directory)
  const wasm = await readFile(options.wasmFile)
  if (startupDigest(wasm) !== admitted.configuration.wasm.sha256 || String(wasm.byteLength) !== admitted.configuration.wasm.byteLength) throw new Error("Static startup WASM bytes differ from the selected package")
  const headerText = await readFile(path.join(options.directory, "_headers"), "utf8")
  const previous = await files(options.previousDirectory)
  const previousHtml = previous.find(file => file.name === "tf2/index.html")
  if (!previousHtml) throw new Error("Warm upgrade requires the retained previous HTML entry")
  let upgrade = false, previousEntryUsed = false, upgradeNavigations = 0
  const reads: Array<{ url: string; sha256: string; bytes: number; owner: string }> = []
  return {
    admitted, previous: { sha256: startupDigest(JSON.stringify(previous)), files: previous }, reads,
    warmUpgrade() { upgrade = true; previousEntryUsed = false; upgradeNavigations = 0 },
    get previousEntryUsed() { return previousEntryUsed },
    get upgradeNavigations() { return upgradeNavigations },
    async response(url: string) {
      const requested = new URL(url)
      let body: Buffer, headers: Record<string, string>, owner = "candidate"
      if (requested.origin === admitted.configuration.assetOrigin) {
        const match = /^\/objects\/sha256\/([0-9a-f]{64})$/.exec(requested.pathname)
        if (!match) return null
        const hash = match[1]!
        body = hash === admitted.configuration.wasm.sha256 ? wasm : await readFile(objectPath(options.assetDir, hash))
        if (startupDigest(body) !== hash) throw new Error("Static startup resource bytes differ from their immutable URL")
        headers = { "content-type": "application/octet-stream", "access-control-allow-origin": TF2_APPLICATION_ORIGIN,
          "access-control-expose-headers": "Content-Length,ETag", "cross-origin-resource-policy": "cross-origin",
          "cache-control": "public, max-age=31536000, immutable, no-transform", etag: `"${hash}"` }
        owner = "immutable-object"
      } else if (requested.origin === TF2_APPLICATION_ORIGIN) {
        const name = requested.pathname === "/tf2" || requested.pathname === "/tf2/" ? "tf2/index.html" : requested.pathname.slice(1)
        let file = admitted.files.find(file => file.name === name)
        let directory = options.directory
        if(upgrade && name === "tf2/index.html") upgradeNavigations++
        if (upgrade && name === "tf2/index.html" && !previousEntryUsed) {
          file = previousHtml; directory = options.previousDirectory; previousEntryUsed = true; owner = "previous-entry"
        } else if (!file && upgrade && name.startsWith("tf2/assets/")) {
          file = previous.find(file => file.name === name); directory = options.previousDirectory; owner = "previous-asset"
        }
        if (!file) return null
        body = await readFile(path.join(directory, file.name))
        if (startupDigest(body) !== file.sha256) throw new Error("Admitted static package changed during startup verification")
        const mime = name.endsWith(".html") ? "text/html" : name.endsWith(".js") ? "text/javascript" : name.endsWith(".css") ? "text/css" : "application/json"
        headers = { ...headersFor(headerText, requested.pathname), "content-type": mime }
      } else return null
      reads.push({ url, sha256: startupDigest(body), bytes: body.byteLength, owner })
      headers["content-length"] = String(body.byteLength)
      return { status: 200, headers, body }
    },
    async verifyUnchanged() {
      if ((await staticStartupPackage(options.directory)).sha256 !== admitted.sha256) throw new Error("Static package changed after startup capture")
      if (startupDigest(await readFile(options.wasmFile)) !== admitted.configuration.wasm.sha256) throw new Error("WASM changed after startup capture")
    },
  }
}
