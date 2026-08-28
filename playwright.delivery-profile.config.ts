import { headedProfileConfiguration } from "./tools/playsrc/profile/profile-config"
const base = headedProfileConfiguration({ match: "upward-training-bots.profile.ts", target: "pl_upward" })
export default { ...base, projects: [
  { name: "ordinary", metadata: { frameDeliveryMode: "ordinary" } },
  { name: "presentation", metadata: { frameDeliveryMode: "presentation" } },
  { name: "ordinary-native", use: { viewport: { width: 1689, height: 1277 } }, metadata: { frameDeliveryMode: "ordinary" } },
  { name: "presentation-native", use: { viewport: { width: 1689, height: 1277 } }, metadata: { frameDeliveryMode: "presentation" } },
  { name: "cpu", metadata: { frameDeliveryMode: "cpu" } },
  { name: "cpu-native", use: { viewport: { width: 1689, height: 1277 } }, metadata: { frameDeliveryMode: "cpu" } },
  { name: "rpc", metadata: { frameDeliveryMode: "rpc", diagnosticMinimumTick: 618 } },
  { name: "traced", metadata: { frameDeliveryMode: "traced" } },
] }
