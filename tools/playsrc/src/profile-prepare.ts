import path from "node:path"
import { repositoryRoot } from "./config"
import { prepareHeadedProfile } from "./profile-runner"

if (import.meta.main) {
  try {
    const args = process.argv.slice(2)
    const root = args[0] === "--application-root" ? args.splice(0, 2)[1] : repositoryRoot
    if (!root || !path.isAbsolute(root)) throw new Error("Application checkout must be an absolute path")
    process.exitCode = await prepareHeadedProfile(args, root)
  } catch (error) { console.error(String(error)); process.exitCode = 1 }
}
