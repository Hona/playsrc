import { createHash } from "node:crypto"
import { readFile, stat } from "node:fs/promises"
import { summarizeCpuProfile } from "./gameui-profile"
import { replayWorkerIncidents } from "./worker-incident-attribution"

/** Offline only. Worker bytes and active marks are authenticated by the retained
 * compositor manifest. A separately supplied main profile is hashed here, NOT
 * claimed to be authenticated by that older manifest. */
export async function replayCpuProfiles(manifest: string, mainProfile?: string) {
  const worker = await replayWorkerIncidents(manifest)
  let main = null
  if (mainProfile !== undefined) {
    if ((await stat(mainProfile)).size > 64 * 1024 * 1024) throw new Error("Main CPU profile byte bound exceeded")
    const bytes = await readFile(mainProfile)
    if (bytes.length > 64 * 1024 * 1024) throw new Error("Main CPU profile byte bound exceeded")
    const profile = JSON.parse(bytes.toString("utf8"))
    main = { sha256: createHash("sha256").update(bytes).digest("hex"), bytes: bytes.length,
      identity: "separately supplied; not linked by compositor manifest",
      cpu: summarizeCpuProfile(profile), activeCpu: summarizeCpuProfile(profile, worker.window) }
  }
  return { schema: "playsrc-cpu-profile-reanalysis-v2", offline: true,
    window: worker.window, compositorComplete: worker.compositorComplete, compositorErrors: worker.compositorErrors,
    workerArtifact: worker.workerArtifact, unsampledTargets: worker.unsampledTargets,
    workers: worker.analyses.map(analysis => ({ target: analysis.target, captureComplete: analysis.captureComplete,
      deadlineStopped: analysis.deadlineStopped, droppedTasks: analysis.droppedTasks, cpu: analysis.cpu, activeCpu: analysis.activeCpu })), main }
}

if (import.meta.main) {
  const [manifest, mainProfile, ...extra] = process.argv.slice(2)
  if (!manifest || extra.length) throw new Error("Usage: bun tools/playsrc/profile/replay-cpu-profile.ts <sha256.manifest.json> [main.cpuprofile]")
  console.log(JSON.stringify(await replayCpuProfiles(manifest, mainProfile), null, 2))
}
