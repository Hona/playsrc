import path from "node:path"
import { headedProfileConfiguration } from "./tools/playsrc/profile/profile-config"
import { loadLocalConfig } from "./tools/playsrc/src/config"

const local = await loadLocalConfig()

export default headedProfileConfiguration({
  match: "gameui-performance.profile.ts",
  output: path.join(local.sourceCacheDir, "profiles", "gameui", "jump_beef", "playwright-results"),
  preciseMemory: true,
})
