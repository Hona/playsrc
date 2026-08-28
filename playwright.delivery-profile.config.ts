import { headedProfileConfiguration } from "./tools/playsrc/profile/profile-config"
const base = headedProfileConfiguration({ match: "upward-training-bots.profile.ts", target: "pl_upward" })
export default { ...base, projects: [
  { name: "ordinary", metadata: { frameDeliveryMode: "ordinary" } },
  { name: "presentation", metadata: { frameDeliveryMode: "presentation" } },
  { name: "traced", metadata: { frameDeliveryMode: "traced" } },
] }
