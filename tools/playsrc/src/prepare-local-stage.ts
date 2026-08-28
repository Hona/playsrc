import { TF2_TARGET_NAMES } from "@playsrc/game-tf2-browser/maps"
import path from "node:path"
import { loadLocalConfig, type LocalConfig } from "./config"
import { acquireHeadedProfileLock, releaseHeadedProfileLock } from "./profile-lock"
import { rustBuildIdentity } from "./build-identity"
import { buildTf2Wasm } from "./tf2-wasm-build"
import { buildSourceBundle, prepareSourceBundleProducer } from "./source-bundle"

export type LocalPreparationStage = Readonly<{ kind: "wasm" | "producer" }> | Readonly<{ kind: "resources"; target: string }>

export function parseLocalPreparationStage(args: readonly string[]): LocalPreparationStage {
  if (args.length === 1 && (args[0] === "wasm" || args[0] === "producer")) return { kind: args[0] }
  if (args.length === 2 && args[0] === "resources" && (TF2_TARGET_NAMES as readonly string[]).includes(args[1]!)) return { kind: "resources", target: args[1]! }
  throw new Error("build-stage accepts wasm | producer | resources <configured map>")
}

const owners = { identity: rustBuildIdentity, wasm: buildTf2Wasm, producer: prepareSourceBundleProducer, resources: buildSourceBundle }

/** One legitimate build owner per bounded local-job command. The normal dev
 * preparation still verifies and publishes the complete closure before Ready;
 * stages neither start listeners nor publish a partially ready application. */
export async function prepareLocalStage(config: LocalConfig, stage: LocalPreparationStage, dependencies = owners) {
  const identity = await dependencies.identity()
  const startedAt = Date.now()
  const artifact = stage.kind === "wasm" ? { wasm: await dependencies.wasm(config) }
    : stage.kind === "producer" ? { producer: (await dependencies.producer(config)).generatorSha256 }
      : { resources: (await dependencies.resources(config, stage.target)).report.graphDescriptor }
  if (await dependencies.identity() !== identity) throw new Error("Build inputs changed during local preparation stage")
  return { schema: "playsrc-local-preparation-stage-v1", stage, identity, startedAt, finishedAt: Date.now(), artifact }
}

if (import.meta.main) {
  try {
    const stage = parseLocalPreparationStage(process.argv.slice(2))
    const config = await loadLocalConfig()
    // A native build must not contend with another lane's measured capture.
    // The local-job owner still bounds the entire stage, including this wait.
    const lockPath = path.join(config.sourceCacheDir, "evidence", "tf2-browser-performance", "chromium-profile.lock")
    const lock = await acquireHeadedProfileLock(lockPath, `prepare-${stage.kind}`)
    try { console.log(JSON.stringify(await prepareLocalStage(config, stage))) }
    finally { await releaseHeadedProfileLock(lockPath, lock.token) }
  } catch (error) { console.error(String(error)); process.exitCode = 1 }
}
