import path from "node:path"
import { headedProfileConfiguration } from "./tools/playsrc/profile/profile-config"
import { loadLocalConfig } from "./tools/playsrc/src/config"

const local = await loadLocalConfig()
export default headedProfileConfiguration({
  match: "shared-lighting.profile.ts",
  output: path.join(local.sourceCacheDir, "profiles", "shared-lighting", "playwright-results"),
})
