import { createHash } from "node:crypto"
import { spawn } from "node:child_process"
import { readFile, mkdir, writeFile, stat } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig, repositoryRoot } from "../src/config"
import { objectPath, putObject } from "@playsrc/asset-store"
import { createDeployedBrowserConfiguration, parseTf2Release } from "../../../apps/web/tf2/src/deployment"

// GitHub release assets, not reconstructed or invented historical configuration.
export const ARCHIVED_GENERATIONS = Object.freeze([
  { tag: "v0.0.8", commit: "b20950fd1a5a48f784187d4d5535711b2f790557", asset: 530370145, sha256: "ce870cb8d0c0455eae4cf13cd6f7ca338d44fdaecca1ba99bf6b3faaeb3fb614" },
  { tag: "v0.0.9", commit: "0b88a4865f7eaa3f0450ff6aea37bd3dcafe5e55", asset: 530628387, sha256: "e9969c4cbbdbfd3ead9184aad8deec01aa5be3f1710ca20d49ff66a53ef77ff7" },
])

export const digest = (bytes: string | Uint8Array) => createHash("sha256").update(bytes).digest("hex")

async function command(args: string[], cwd: string): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    const child = spawn(args[0]!, args.slice(1), { cwd, stdio: ["ignore", "pipe", "inherit"] })
    const chunks: Buffer[] = []
    child.stdout!.on("data", (bytes) => chunks.push(Buffer.from(bytes)))
    child.on("error", reject)
    child.on("close", (code) => code === 0 ? resolve(Buffer.concat(chunks)) : reject(new Error(`Release fixture command failed: ${args[0]} ${args[1]}`)))
  })
}

export async function archivedGeneration(fixture: typeof ARCHIVED_GENERATIONS[number], prepare = false) {
  const config = await loadLocalConfig()
  const directory = path.join(config.sourceCacheDir, "profiles", "application-upgrade", "fixtures", fixture.commit)
  await mkdir(directory, { recursive: true })
  const manifestPath = path.join(directory, "release.json")
  let bytes = await readFile(manifestPath).catch(() => null)
  if (!bytes) {
    bytes = Buffer.from(await command(["gh", "api", "-H", "Accept: application/octet-stream", `repos/Hona/playsrc/releases/assets/${fixture.asset}`], repositoryRoot))
    if (digest(bytes) !== fixture.sha256) throw new Error(`${fixture.tag} archived manifest digest differs`)
    await writeFile(manifestPath, bytes)
  }
  if (digest(bytes) !== fixture.sha256) throw new Error(`${fixture.tag} cached archive manifest is corrupt`)
  const manifest = JSON.parse(bytes.toString())
  if (manifest.applicationBuild !== digest(fixture.commit)) throw new Error(`${fixture.tag} public commit differs from its deployed application identity`)
  const release = parseTf2Release(manifest.release)
  const configuration = createDeployedBrowserConfiguration(release, manifest.applicationBuild)
  const source = path.join(directory, "source")
  if (!await stat(path.join(source, ".git")).catch(() => null)) {
    await command(["git", "clone", "--shared", "--no-checkout", repositoryRoot, source], repositoryRoot)
    await command(["git", "checkout", "--detach", fixture.commit], source)
  }
  const head = new TextDecoder().decode(await command(["git", "rev-parse", "HEAD"], source)).trim()
  if (head !== fixture.commit) throw new Error("Archived fixture checkout changed")
  await writeFile(path.join(source, "playsrc.local.json"), JSON.stringify(config))
  const app = path.join(source, "apps", "web", "tf2")
  const output = path.join(app, "dist", "cloudflare", "tf2")
  if (!await stat(path.join(output, "index.html")).catch(() => null)) {
    await command(["bun", "install", "--frozen-lockfile"], source)
    await command(["bun", "run", "build"], app)
  }
  const descriptor = release.objects.wasm
  const compiled = await readFile(path.join(source, "games", "tf2", "browser", "src", "wasm-generated", "tf2_wasm_bg.wasm"))
  if (digest(compiled) !== descriptor.sha256) throw new Error(`${fixture.tag} rebuilt bindings do not match the archived WASM`)
  const response = await fetch(`https://assets.playsrc.online/objects/sha256/${descriptor.sha256}`)
  if (!response.ok) throw new Error(`${fixture.tag} archived immutable WASM is unavailable: ${response.status}`)
  const wasm = new Uint8Array(await response.arrayBuffer())
  if (digest(wasm) !== descriptor.sha256 || wasm.byteLength !== Number(descriptor.byteLength)) throw new Error("Archived WASM is corrupt")
  if (prepare) await putObject(config.assetDir, descriptor, wasm)
  else {
    const stored = await readFile(objectPath(config.assetDir, descriptor.sha256)).catch(() => null)
    if (!stored || digest(stored) !== descriptor.sha256) throw new Error("Prepare authenticated archived objects with bun tools/playsrc/profile/release-generations.ts before the headed run")
  }
  return { ...fixture, directory, output, configuration }
}

if (import.meta.main) {
  for (const fixture of ARCHIVED_GENERATIONS) {
    const result = await archivedGeneration(fixture, true)
    console.log(JSON.stringify({ tag: result.tag, commit: result.commit, manifest: result.sha256, wasm: result.configuration.wasm.sha256, output: result.output }))
  }
}
