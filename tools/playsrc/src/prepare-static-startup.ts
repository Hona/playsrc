import path from "node:path"
import { loadLocalConfig, repositoryRoot } from "./config"
import { buildTf2ReleaseCandidate } from "./tf2-release"
import { buildStaticSite } from "./deploy"
import { staticStartupPackage } from "../profile/static-startup-package"
import { objectPath } from "@playsrc/asset-store"
import { readFile } from "node:fs/promises"
import { assertReleaseWasmInterface } from "./deploy"

const config = await loadLocalConfig()
const approved=process.argv.slice(2).join(" ")==="--approved"
if(process.argv.length>2&&!approved)throw new Error("Expected no arguments or --approved")
const artifact=approved?undefined:await buildTf2ReleaseCandidate(config)
await buildStaticSite(undefined, approved?{approved:true}:{candidate:artifact!.release})
const directory = path.join(repositoryRoot, "apps/web/tf2/dist/cloudflare")
const packaged = await staticStartupPackage(directory)
const wasmFile=approved?objectPath(config.assetDir,packaged.configuration.wasm.sha256):artifact!.files.get(packaged.configuration.wasm.sha256)?.pathname
if(!wasmFile)throw new Error("Selected WASM changed while preparing the static package")
if(approved)assertReleaseWasmInterface(await readFile(path.join(repositoryRoot,"games/tf2/browser/src/wasm-generated/tf2_wasm_bg.wasm")),await readFile(wasmFile))
console.log(JSON.stringify({ directory, packageSha256: packaged.sha256, wasmFile, assetDir: config.assetDir, approved }))
