import path from "node:path"
import { headedProfileConfiguration } from "./tools/playsrc/profile/profile-config"
import { loadLocalConfig } from "./tools/playsrc/src/config"

const local = await loadLocalConfig()

export default headedProfileConfiguration({
  match: "three-map-load.profile.ts",
  output: path.join(local.sourceCacheDir, "profiles", "three-map-load", "playwright-results"),
})
