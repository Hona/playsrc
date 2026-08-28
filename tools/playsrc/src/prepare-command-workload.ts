import path from "node:path"
import { loadLocalConfig } from "./config"
import { prepareCommandWorkload } from "../profile/command-workload"
const [manifest] = process.argv.slice(2)
if (!manifest || process.argv.length !== 3) throw new Error("prepare-command-workload.ts <retained compositor manifest>")
console.log(await prepareCommandWorkload(manifest, path.join((await loadLocalConfig()).sourceCacheDir, "profiles/command-workloads")))
