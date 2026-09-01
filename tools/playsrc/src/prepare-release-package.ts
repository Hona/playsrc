import { buildStaticSite } from "./deploy"
import { staticStartupPackage } from "../profile/static-startup-package"
import { repositoryRoot } from "./config"
import path from "node:path"

await buildStaticSite(undefined, { approved: true })
const packaged = await staticStartupPackage(path.join(repositoryRoot, "apps/web/tf2/dist/cloudflare"))
console.log(JSON.stringify({ packageSha256: packaged.sha256, applicationBuild: packaged.configuration.applicationBuild, files: packaged.files }))
