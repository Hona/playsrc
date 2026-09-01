import { assertReleaseWasmInterface, buildStaticSite, readRemoteReleaseObject } from "./deploy"
import { staticStartupPackage } from "../profile/static-startup-package"
import { repositoryRoot } from "./config"
import path from "node:path"
import { readFile } from "node:fs/promises"
import { readTf2Release } from "./tf2-release"

const release = await readTf2Release(undefined)
const compiled = await readFile(path.join(repositoryRoot, "games/tf2/browser/src/wasm-generated/tf2_wasm_bg.wasm"))
const approved = await readRemoteReleaseObject(release.objects.wasm)
assertReleaseWasmInterface(compiled, approved)
await buildStaticSite(undefined, { approved: true })
const packaged = await staticStartupPackage(path.join(repositoryRoot, "apps/web/tf2/dist/cloudflare"))
console.log(JSON.stringify({ packageSha256: packaged.sha256, applicationBuild: packaged.configuration.applicationBuild, files: packaged.files }))
