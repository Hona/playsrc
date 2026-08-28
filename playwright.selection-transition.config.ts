import { headedProfileConfiguration } from "./tools/playsrc/profile/profile-config"
// Ordinary --grep selects one exact matrix case through the native job runner.
// The default remains one bounded capture, never36 expensive browser runs.
const selected = process.argv.some(value => value === "--grep" || value.startsWith("--grep="))
export default headedProfileConfiguration({ match: "selection-transition.profile.ts", target: "pl_upward", ...(selected ? {} : { grep: /red class9 cold/ }) })
