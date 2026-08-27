import { createHash } from "node:crypto"
import { readFile, stat } from "node:fs/promises"
import { summarizeCpuProfile } from "./gameui-profile"
import { loadWorkerIncidents } from "./worker-incident-attribution"
import { loadCompositorEvidence, loadMainCpuEvidence } from "./compositor-evidence"
import { CPU_PROFILE_LIMITS } from "./cpu-profile-time"
import { assertMatchingCapturePlans } from "./upward-capture-plan"
import { loadAllocationMemoryEvidence } from "./allocation-memory-evidence"

/** Offline only. v2 resolves main bytes exclusively through the manifest; v1
 * supplied profiles stay explicitly unauthenticated and are never migrated. */
export async function replayCpuProfiles(manifest: string, mainProfile?: string) {
  const loaded = await loadCompositorEvidence(manifest)
  if (loaded.manifest.schema !== "playsrc-compositor-evidence-v1" && mainProfile !== undefined) throw new Error("Main CPU profile must come from the authenticated manifest, not a supplied file")
  const linked = await loadMainCpuEvidence(manifest, loaded)
  const worker = await loadWorkerIncidents(manifest, loaded)
  let main = null
  if (linked) main = { ...linked.evidence, authenticated: true, identity: "linked by compositor manifest",
    captureComplete: linked.profile !== null, cpu: linked.profile ? summarizeCpuProfile(linked.profile) : null,
    activeCpu: linked.profile ? summarizeCpuProfile(linked.profile, worker.window) : null }
  if (mainProfile !== undefined) {
    if ((await stat(mainProfile)).size > CPU_PROFILE_LIMITS.bytes) throw new Error("Main CPU profile byte bound exceeded")
    const bytes = await readFile(mainProfile)
    if (bytes.length > CPU_PROFILE_LIMITS.bytes) throw new Error("Main CPU profile byte bound exceeded")
    const profile = JSON.parse(bytes.toString("utf8"))
    main = { sha256: createHash("sha256").update(bytes).digest("hex"), bytes: bytes.length,
      authenticated: false, identity: "separately supplied; not linked by compositor manifest",
      cpu: summarizeCpuProfile(profile), activeCpu: summarizeCpuProfile(profile, worker.window) }
  }
  return { schema: "playsrc-cpu-profile-reanalysis-v4", offline: true, capturePlan: worker.capturePlan,
    memory: await loadAllocationMemoryEvidence(manifest, loaded),
    workerInstrumentation: worker.workerInstrumentation,
    window: worker.window, compositorComplete: worker.compositorComplete, compositorErrors: worker.compositorErrors,
    workerArtifact: worker.workerArtifact, unsampledTargets: worker.unsampledTargets,
    workers: worker.analyses.map(analysis => ({ target: analysis.target, captureComplete: analysis.captureComplete,
      deadlineStopped: analysis.deadlineStopped, droppedTasks: analysis.droppedTasks, cpu: analysis.cpu, activeCpu: analysis.activeCpu })), main }
}

export async function compareWorkerCpuProfiles(beforeManifest: string, afterManifest: string) {
  const before = await replayCpuProfiles(beforeManifest), after = await replayCpuProfiles(afterManifest)
  assertMatchingCapturePlans(before.capturePlan, after.capturePlan)
  if (before.capturePlan?.workerCpu !== "required") throw new Error("Worker comparison requires requested Worker instrumentation, not absent CPU")
  if (!before.compositorComplete || !after.compositorComplete) throw new Error("Worker comparison requires complete compositor evidence")
  return { schema: "playsrc-worker-cpu-comparison-v1", comparable: true, before, after,
    conclusion: "Matching instrumented plans; sampled estimates, not exclusive CPU wall or statistical non-regression proof" }
}

if (import.meta.main) {
  const args = process.argv.slice(2)
  if (args[0] === "--compare-workers" && args.length === 3) console.log(JSON.stringify(await compareWorkerCpuProfiles(args[1]!, args[2]!), null, 2))
  else {
    const [manifest, mainProfile, ...extra] = args
    if (!manifest || manifest.startsWith("--") || extra.length) throw new Error("Usage: bun tools/playsrc/profile/replay-cpu-profile.ts <sha256.manifest.json> [historical-main.cpuprofile] | --compare-workers <before.manifest.json> <after.manifest.json>")
    console.log(JSON.stringify(await replayCpuProfiles(manifest, mainProfile), null, 2))
  }
}
