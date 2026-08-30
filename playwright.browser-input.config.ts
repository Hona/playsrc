import { headedProfileConfiguration } from "./tools/playsrc/profile/profile-config"

export default headedProfileConfiguration({
  match: "browser-input.profile.ts",
  target: "pl_upward",
  ...(process.platform === "win32" ? { channel: "msedge" } : {}),
})
