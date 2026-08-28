import { createHash } from "node:crypto"
import { readFile, writeFile, mkdir } from "node:fs/promises"
import path from "node:path"
import { gunzipSync } from "node:zlib"
import { parseGameplayReplay, validateReplayLifecycle } from "./gameplay-replay"
import type { CommandWorkload, WorkloadMutation, WorkloadObserve } from "../../../games/tf2/browser/src/command-workload"
import { validateWorkload } from "../../../games/tf2/browser/src/command-workload"
import { RecordedClientRenderFrames } from "../../../games/tf2/browser/src/client-render-frame"

const hash = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex")
function require(value: unknown, message: string): asserts value { if (!value) throw new Error(message) }
function projectObserves(records: ReturnType<typeof parseGameplayReplay>["records"]) {
  const observes: WorkloadObserve[] = [], pending: WorkloadMutation[] = []
  for (const record of records) {
    if ([4, 5, 6, 9, 10].includes(record.kind)) pending.push({ kind: record.kind as WorkloadMutation["kind"], hex: record.bytes.toString("hex") })
    if (record.kind === 1) observes.push({ nowSeconds: record.bytes.readDoubleLE(0), suspended: record.bytes.readUInt32LE(8) !== 0,
      snapshotTick: String(record.bytes.readBigUInt64LE(12)), command: record.bytes.subarray(24).toString("hex"), mutations: pending.splice(0) })
  }
  require(pending.length === 0, "Unobserved trailing mutations cannot be omitted from a workload")
  return observes
}
async function verifiedWorkload(captureFile: string) {
  const bytes = await readFile(captureFile), capture = JSON.parse(bytes.toString())
  require(path.basename(captureFile) === `${hash(bytes)}.manifest.json`, "Capture identity differs")
  require(capture.complete && capture.errors?.length === 0 && capture.identity.sampleError === null
    && capture.identity.sourceUnchanged && capture.identity.workloadState?.schema === 3, "Capture lacks complete unchanged-source scene authentication")
  const directory = path.dirname(captureFile), link = capture.identity.gameplayReplayLifecycle
  require(link?.complete, "Workload requires complete generation closure")
  const lifecycleBytes = await readFile(path.join(directory, link.file))
  require(hash(lifecycleBytes) === link.sha256 && lifecycleBytes.length === link.bytes, "Lifecycle identity differs")
  const lifecycle = JSON.parse(lifecycleBytes.toString()); validateReplayLifecycle(lifecycle)
  const entry = lifecycle.generations.at(-1), journal = entry.journal
  const journalManifestBytes = await readFile(path.join(directory, journal.manifestFile))
  require(`${hash(journalManifestBytes)}.replay.json` === journal.manifestFile, "Journal manifest identity differs")
  const manifest = JSON.parse(journalManifestBytes.toString()), raw = await readFile(path.join(directory, manifest.file))
  require(hash(raw) === journal.sha256 && raw.length === journal.bytes && manifest.complete, "Journal identity differs")
  const replay = parseGameplayReplay(raw)
  require(manifest.entropy?.file === `${manifest.entropy?.sha256}.map-entropy.bin`, "Missing recorded map-particle entropy")
  const entropy = await readFile(path.join(directory, manifest.entropy.file))
  require(entropy.length === manifest.entropy.bytes && hash(entropy) === manifest.entropy.sha256, "Recorded map-particle entropy changed")
  const probeBytes = await readFile(path.join(directory, capture.probes.file))
  require(probeBytes.length === capture.probes.bytes && hash(probeBytes) === capture.probes.sha256, "Measured phase identity differs")
  const probes = JSON.parse(gunzipSync(probeBytes, { maxOutputLength: 32 * 1024 * 1024 }).toString())
  require(probes.dropped === 0, "Workload phase contains dropped evidence")
  const observes = projectObserves(replay.records)
  require(replay.version === 4, "Workload requires recorded NextBot work-clock inputs")
  const workClockHex = Buffer.concat(replay.records.filter(record => record.kind === 2)
    .map(record => record.bytes.subarray(56 + record.bytes.readUInt32LE(48)))).toString("hex")
  const lastTick = replay.records.filter(record => record.kind === 2).at(-1)!.bytes.readBigUInt64LE(0)
  require(Array.isArray(capture.identity.presentationInputs), "Missing presentation input history")
  const presentations = capture.identity.presentationInputs.filter((entry: any) => BigInt(entry.lastHostTick) <= lastTick)
  const plan: CommandWorkload = { schema: 1, journalSha256: journal.sha256, bspSha256: replay.bspSha256,
    configurationSha256: journal.checkpoint.configurationSha256, configurationBytes: journal.checkpoint.configurationBytes,
    generation: journal.checkpoint.generation, profile: journal.checkpoint.profile,
    sampleStarted: probes.started, sampleEnded: probes.ended, observes,
    clientFrames: capture.identity.clientFrameInputs ?? undefined, presentations }
  validateWorkload(plan)
  require(Array.isArray(plan.clientFrames) && plan.clientFrames.length > 0, "Missing authenticated client-frame clock inputs")
  new RecordedClientRenderFrames(plan.clientFrames)
  require(capture.identity.workloadState?.frame?.models?.length > 0, "Missing authenticated initial model/scene state")
  require(capture.identity.workloadState?.frame?.scene?.surfaces, "Missing current renderer scene inputs")
  require(capture.identity.workloadState.frame.models.some((model: any) => model.viewModel === true && model.pose?.viewmodel?.phase === "draw"), "Workload does not begin in the declared equipped-primary draw phase")
  require(observes.at(-1)!.nowSeconds * 1000 - plan.sampleEnded >= 4000, "Recorded workload lacks an authenticated retention tail; author a new workload without modifying this one")
  return { schema: "playsrc-command-workload-v1", captureFile, captureSha256: hash(bytes), journalFile: path.join(directory, manifest.file),
    journalBytes: journal.bytes, headerHex: raw.subarray(0, replay.headerBytes).toString("hex"), applicationGeneration: entry.applicationGeneration,
    initialState: capture.identity.workloadState, entropyHex: entropy.toString("hex"),
    workClock: { hex: workClockHex, endedAt: observes.at(-1)!.nowSeconds }, plan }
}
export async function prepareCommandWorkload(captureFile: string, destination: string) {
  const result = await verifiedWorkload(captureFile)
  const output = Buffer.from(JSON.stringify(result)), identity = hash(output)
  await mkdir(destination, { recursive: true })
  await writeFile(path.join(destination, `${identity}.json`), output, { flag: "wx" })
  return identity
}
export async function loadCommandWorkload(directory: string, identity: string) {
  require(/^[a-f0-9]{64}$/.test(identity), "Invalid workload identity")
  const raw = await readFile(path.join(directory, `${identity}.json`))
  require(hash(raw) === identity, "Workload descriptor changed")
  const value = JSON.parse(raw.toString())
  require(value.schema === "playsrc-command-workload-v1" && value.initialState?.schema === 3, "Invalid workload schema/state authentication")
  require(JSON.stringify(await verifiedWorkload(value.captureFile)) === JSON.stringify(value), "Workload projection rewrites recorded inputs, phase or state")
  return value as typeof value & { plan: CommandWorkload }
}

/** Validate the actual admitted full-state hashes and input bytes. A nominal
 * roster, similar camera or duration never substitutes for this join. */
export function compareWorkloadJournal(expected: Buffer, actual: Buffer, throughNow: number) {
  const left = parseGameplayReplay(expected), right = parseGameplayReplay(actual, false)
  require(expected.subarray(0, left.headerBytes).equals(actual.subarray(0, right.headerBytes)), "Initial world/equipment state differs")
  const prefix = (records: typeof left.records) => {
    const result = []; let include = true
    for (const record of records) {
      if (record.kind === 1 && record.bytes.readDoubleLE(0) > throughNow) include = false
      if (!include) break
      // Timing instrumentation and local sample marks are not simulation state.
      if (record.kind === 2) result.push(Buffer.concat([record.bytes.subarray(0, 8), record.bytes.subarray(16)]))
      else if (![7, 8].includes(record.kind)) result.push(Buffer.concat([Buffer.from([record.kind]), record.bytes]))
    }
    return result
  }
  const a = prefix(left.records), b = prefix(right.records)
  require(a.length === b.length && a.every((record, i) => record.equals(b[i]!)), "Actual workload phase, commands or complete state hashes differ")
}
