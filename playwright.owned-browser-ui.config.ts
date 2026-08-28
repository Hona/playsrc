import path from "node:path"
import { headedProfileConfiguration } from "./tools/playsrc/profile/profile-config"
import { loadLocalConfig } from "./tools/playsrc/src/config"
const config = await loadLocalConfig()
export default headedProfileConfiguration({ match: "owned-browser-ui.profile.ts", target: "pl_upward", preciseMemory: true, output: path.join(config.sourceCacheDir, "profiles/owned-browser-ui/results") })
