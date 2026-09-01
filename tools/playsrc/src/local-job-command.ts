import { parseHeadedProfile } from "./profile-runner"
import { TF2_TARGET_NAMES } from "@playsrc/game-tf2-browser/maps"

export type LocalPreparationStage = Readonly<{ kind: "wasm" | "producer" | "browser" }> | Readonly<{ kind: "resources"; target: string }>

export function parseLocalPreparationStage(args: readonly string[]): LocalPreparationStage {
  if (args.length === 1 && (args[0] === "wasm" || args[0] === "producer" || args[0] === "browser")) return { kind: args[0] }
  if (args.length === 2 && args[0] === "resources" && (TF2_TARGET_NAMES as readonly string[]).includes(args[1]!)) return { kind: "resources", target: args[1]! }
  throw new Error("build-stage accepts wasm | producer | browser | resources <configured map>")
}

/** One authority for queue transport, native dispatch and ownership readback.
 * UI policy is derived from the workload, never a caller-supplied switch. */
export function localJobCommand(args: readonly string[]): { command: string[]; interactive: boolean; controller?: true } {
  if (!Array.isArray(args) || args.length > 20 || args.some(value => typeof value !== "string" || value.length > 1024 || value.includes("\0"))) throw new Error("Invalid workload arguments")
  const [kind, ...options] = args
  if (kind === "test") {
    if (options.some(value => !/^[A-Za-z0-9_./-]+\.test\.ts$/.test(value) || value.startsWith("-") || value.startsWith("/") || value.split("/").includes(".."))) {
      throw new Error("test accepts only repository-relative .test.ts files")
    }
    return { command: ["test", ...options], interactive: false }
  }
  if (kind === "profile" || kind === "prepare-profile") {
    const parsed = parseHeadedProfile(options)
    const listing = parsed.playwright.some(option => option === "--list" || option === "--help" || option === "-h")
    return { command: [kind === "profile" ? "tools/playsrc/src/profile-runner.ts" : "tools/playsrc/src/profile-prepare.ts", ...options], interactive: kind === "profile" && !listing, controller: true }
  }
  if (kind === "build" && options.length === 1 && (TF2_TARGET_NAMES as readonly string[]).includes(options[0]!)) {
    return { command: ["tools/playsrc/src/cli.ts", "dev", options[0]!, "--prepare-only"], interactive: false }
  }
  if (kind === "build-stage") {
    parseLocalPreparationStage(options)
    return { command: ["tools/playsrc/src/prepare-local-stage.ts", ...options], interactive: false }
  }
  if (kind === "diagnostic" && options.length === 2 && /^\d{1,5}$/.test(options[0]!) && Number(options[0]) <= 30_000 && /^[01]$/.test(options[1]!)) {
    return { command: ["tools/playsrc/src/local-job-diagnostic.ts", ...options], interactive: false }
  }
  throw new Error("Expected test [files...], build <map>, build-stage wasm|producer|browser|resources <map>, or profile <normal profile name> [normal profiler options]")
}
