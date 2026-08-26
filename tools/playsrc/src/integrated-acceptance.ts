import { readFile } from "node:fs/promises"
import { acceptanceScenario, compareAcceptance } from "../profile/integrated-acceptance"
import { runHeadedProfile } from "./profile-runner"

const [scenario, ...arguments_] = process.argv.slice(2)
if (scenario === "compare") {
  if (arguments_.length !== 2) throw new Error("Usage: profile:acceptance compare before.json after.json")
  console.log(JSON.stringify(compareAcceptance(...await Promise.all(arguments_.map(async file => JSON.parse(await readFile(file, "utf8")))) as [any, any]), null, 2))
} else {
  const plan = acceptanceScenario(scenario ?? "")
  if (process.env.PROFILE_UPWARD_TRAINING_PLAYERS) throw new Error("Acceptance preserves the authored default 16-player training roster")
  process.env.PLAYSRC_PROFILE_BROWSER_CHANNEL ??= "msedge"
  process.env.PLAYSRC_PROFILE_DEVICE_SCALE_FACTOR = plan.dpr
  process.env.PROFILE_SAMPLE_SECONDS = "10"
  process.env.PROFILE_UPWARD_REQUIRE_COMPOSITOR = "1"
  process.env.PROFILE_UPWARD_TRAINING_WARM_RELOAD = "1"
  process.env.PROFILE_UPWARD_TRAINING_INTERACTION = "1"
  process.env.PROFILE_INTEGRATED_ACCEPTANCE = "1"
  process.env.PROFILE_UPWARD_TRAINING_LABEL ??= `acceptance-${scenario}-${Date.now()}`
  process.exitCode = await runHeadedProfile([plan.profile, ...arguments_])
}
