import path from "node:path"
import { loadLocalConfig, type LocalConfig } from "./config"
import { acquireHeadedProfileLock, releaseHeadedProfileLock } from "./profile-lock"
import { rustBuildIdentity } from "./build-identity"
import { buildTf2Wasm } from "./tf2-wasm-build"
import { buildSourceBundle, prepareSourceBundleProducer } from "./source-bundle"
import { borrowedWindowsJobLock } from "./windows-job-native"
import { parseLocalPreparationStage, type LocalPreparationStage } from "./local-job-command"
import { prepareProfileBrowserDependencies } from "./profile-browser"

const owners = { identity: rustBuildIdentity, wasm: buildTf2Wasm, producer: prepareSourceBundleProducer, resources: buildSourceBundle, browser: prepareProfileBrowserDependencies }

/** One legitimate build owner per bounded local-job command. The normal dev
 * preparation still verifies and publishes the complete closure before Ready;
 * stages neither start listeners nor publish a partially ready application. */
export async function prepareLocalStage(config: LocalConfig, stage: LocalPreparationStage, dependencies = owners) {
  const identity = await dependencies.identity()
  const startedAt = Date.now()
  const artifact = stage.kind === "wasm" ? { wasm: await dependencies.wasm(config) }
    : stage.kind === "producer" ? { producer: (await dependencies.producer(config)).generatorSha256 }
      : stage.kind === "browser" ? { browser: await dependencies.browser() }
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
    const borrowed = await borrowedWindowsJobLock(lockPath, ["build-stage", ...process.argv.slice(2)])
    if (process.platform === "win32" && !borrowed) throw new Error("Windows preparation requires local-job run <job> build-stage ... and its checked job ownership")
    const lock = borrowed ?? await acquireHeadedProfileLock(lockPath, `prepare-${stage.kind}`)
    try { console.log(JSON.stringify(await prepareLocalStage(config, stage))) }
    finally { if (!borrowed) await releaseHeadedProfileLock(lockPath, lock.token) }
  } catch (error) { console.error(String(error)); process.exitCode = 1 }
}
