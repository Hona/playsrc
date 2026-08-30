import path from "node:path"
import { headedProfileConfiguration } from "./tools/playsrc/profile/profile-config"
import { loadLocalConfig } from "./tools/playsrc/src/config"

const local = await loadLocalConfig()
export default headedProfileConfiguration({ match: "vgui-raster-parity.profile.ts", output: path.join(local.sourceCacheDir, "profiles/vgui-raster-parity/playwright-results") })
