import path from "node:path"
import { createHash, randomUUID } from "node:crypto"
import { readFile, mkdir, writeFile } from "node:fs/promises"
import { objectPath } from "@playsrc/asset-store"
import { parseResourceGraphBytes, encodeResourceBatch } from "@playsrc/asset-store/graph"
import { loadLocalConfig } from "./config"
import { acquireHeadedProfileLock, releaseHeadedProfileLock } from "./profile-lock"
import { exactWasmReplayRuntime } from "../profile/exact-wasm-replay"
import { installNodeWorkerHost } from "../profile/node-worker-host.mjs"
import { parseGameplayReplay } from "../profile/gameplay-replay"
import { replayMutation } from "../profile/replay-gameplay"
import { decodeAdmissionMetrics, MAX_ADMISSION_EVENTS, ADMISSION_EVENT_BYTES } from "../../../games/tf2/browser/src/admission-metrics"
import { summarizeObserveStages } from "../profile/observe-stages"
import { Session } from "node:inspector/promises"

// Explicit content experiment, NOT historical replay or visible acceptance.
// The strict replay verifier remains separate and unchanged. These commands
// keep their original times; a changed graph necessarily defines a new world.
const [source, capturePath, graphIdentity] = process.argv.slice(2)
const instrument = process.argv[5] === "--stages"
const hash = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex")
function require(value: unknown, message: string): asserts value { if (!value) throw new Error(message) }
require(source && capturePath && /^[a-f0-9]{64}$/.test(graphIdentity ?? ""), "Expected closure, capture and explicit diagnostic graph")
const config = await loadLocalConfig()
const captureBytes = await readFile(capturePath)
require(path.basename(capturePath) === `${hash(captureBytes)}.manifest.json`, "Capture hash differs")
const capture = JSON.parse(captureBytes.toString()), link = capture.identity.gameplayReplay
require(link.complete && /^[a-f0-9]{64}\.replay\.json$/.test(link.manifestFile), "Complete journal required")
const manifestBytes = await readFile(path.join(path.dirname(capturePath), link.manifestFile))
require(`${hash(manifestBytes)}.replay.json` === link.manifestFile, "Journal manifest hash differs")
const manifest = JSON.parse(manifestBytes.toString())
require(manifest.complete && manifest.file === `${link.sha256}.replay.bin` && manifest.sha256 === link.sha256 && manifest.bytes === link.bytes, "Journal linkage differs")
const journal = await readFile(path.join(path.dirname(capturePath), manifest.file))
require(hash(journal) === link.sha256 && journal.length === link.bytes, "Journal bytes differ")
const replay = parseGameplayReplay(journal)
const graphBytes = await readFile(objectPath(config.assetDir, graphIdentity))
require(hash(graphBytes) === graphIdentity, "Diagnostic graph hash differs")
const graph = parseResourceGraphBytes(graphBytes)
const bsp = await readFile(path.join(config.sourceCacheDir, "objects/sha256", replay.bspSha256.slice(0, 2), replay.bspSha256))
require(hash(bsp) === replay.bspSha256, "BSP hash differs")
const directory = path.join(config.sourceCacheDir, "observe-diagnostics", "workload-" + randomUUID())
await mkdir(directory, { recursive: true })
const lockPath = path.join(config.sourceCacheDir, "evidence/tf2-browser-performance/chromium-profile.lock")
const deadline = setTimeout(() => process.exit(124), 170_000)
const lock = await acquireHeadedProfileLock(lockPath, "observe-content-experiment", 60_000)
const restore = installNodeWorkerHost()
let runtime: Awaited<ReturnType<typeof exactWasmReplayRuntime>> | undefined
try {
  runtime = await exactWasmReplayRuntime(source, directory, 2)
  const e = await runtime.instantiate(await readFile(path.join(source, "tf2_wasm_bg.wasm")), 0)
  if (replay.initialEquipment) replayMutation(e, 0, { kind: 9, bytes: Buffer.concat([Buffer.from([0]), replay.initialEquipment]) })
  const copy = (bytes: Uint8Array) => { const pointer = e.playsrc_alloc(bytes.length) >>> 0; new Uint8Array(e.memory.buffer, pointer, bytes.length).set(bytes); return pointer }
  const sections: { pointer: number; length: number }[] = []
  for (const descriptor of graph.chunks.filter(chunk => chunk.roles.includes("gameplay"))) {
    const bytes = await readFile(objectPath(config.assetDir, descriptor.encodedSha256))
    const batch = encodeResourceBatch([{ descriptor, bytes }]), pointer = copy(batch)
    require(e.playsrc_resource_decode(pointer, batch.length) === 1, "Resource authentication failed")
    e.playsrc_free(pointer, batch.length)
    const length = e.playsrc_resource_length()
    sections.push({ pointer: e.playsrc_resource_take() >>> 0, length })
  }
  const tableBytes = Buffer.alloc(sections.length * 8)
  sections.forEach((section, index) => { tableBytes.writeUInt32LE(section.pointer, index * 8); tableBytes.writeUInt32LE(section.length, index * 8 + 4) })
  const table = copy(tableBytes), digest = e.playsrc_alloc(32) >>> 0, bspPointer = copy(bsp)
  const configurationBytes = e.playsrc_resource_sections_hash(table, sections.length, digest)
  const configurationSha256 = Buffer.from(new Uint8Array(e.memory.buffer, digest, 32)).toString("hex")
  const handle = e.playsrc_compile_map(bspPointer, bsp.length, manifest.checkpoint.profile, table, sections.length, digest, 1)
  require(e.playsrc_result_error(handle) === 0, `Map compile failed: ${e.playsrc_result_error(handle)}`)
  e.playsrc_result_release(handle); e.playsrc_presentation_release(handle)
  const profiler = instrument ? new Session() : undefined
  if (profiler) {
    require(e.playsrc_gameplay_replay_begin(handle) === 1, "Diagnostic journal could not begin")
    profiler.connect()
    await profiler.post("Profiler.enable")
    await profiler.post("Profiler.start")
  }
  let cpuProfile: unknown
  const observations = [], publications = [], started = performance.now(), cpu = process.cpuUsage()
  let active = false
  for (const [index, record] of replay.records.entries()) {
    if (profiler && !cpuProfile && performance.now() - started >= 5000) {
      cpuProfile = await profiler.post("Profiler.stop")
      profiler.disconnect()
    }
    const data = record.bytes
    if (record.kind === 7) active = data.readUInt32LE(0) === 0
    else if (record.kind === 1) {
      const command = data.subarray(24), pointer = copy(command), begin = performance.now()
      const success = e.playsrc_simulation_observe(handle, data.readDoubleLE(0), pointer, command.length, data.readUInt32LE(8), data.readBigUInt64LE(12))
      const milliseconds = performance.now() - begin
      e.playsrc_free(pointer, command.length)
      require(success === 1, `Observe ${index} failed: ${e.playsrc_simulation_error()}`)
      const length = e.playsrc_simulation_output_length(handle), output = e.playsrc_simulation_output_pointer(handle) >>> 0
      publications.push(hash(new Uint8Array(e.memory.buffer, output, length)))
      observations.push({ index, active, milliseconds, bytes: length })
    } else if ([4, 5, 6, 9, 10].includes(record.kind)) replayMutation(e, handle, record)
    else if (record.kind === 8) require(data.readUInt32LE(0) === 1, "Incomplete journal footer")
    else require(record.kind === 2 || record.kind === 3, `Unsupported journal command ${record.kind}`)
  }
  const milliseconds = performance.now() - started, cpuMicroseconds = process.cpuUsage(cpu)
  if (profiler && !cpuProfile) { cpuProfile = await profiler.post("Profiler.stop"); profiler.disconnect() }
  if (instrument) require(e.playsrc_gameplay_replay_stop(handle) === 1, "Diagnostic journal incomplete")
  const length = e.playsrc_admission_metrics_length()
  require(length <= MAX_ADMISSION_EVENTS * ADMISSION_EVENT_BYTES, "Admission bound exceeded")
  const pointer = e.playsrc_alloc(Math.max(1, length)) >>> 0
  require(e.playsrc_admission_metrics_copy(pointer, length) === length, "Admission copy failed")
  const events = decodeAdmissionMetrics(new DataView(e.memory.buffer, pointer, length))
  const result = { schema: "playsrc-observe-content-experiment-v1", visibleAcceptance: false, historicalReplayAcceptance: false,
    graphIdentity, historicalGraphIdentity: capture.identity.applicationGeneration.resourceRoot, commandJournal: link.sha256,
    configurationBytes, configurationSha256, generatedClosure: runtime.manifest, engine: process.versions,
    milliseconds, cpuMicroseconds, instrument, observations, publications, admission: { events, dropped: e.playsrc_admission_metrics_dropped() }, stages: summarizeObserveStages(events) }
  if (cpuProfile) await writeFile(path.join(directory, "cpu.json"), JSON.stringify(cpuProfile))
  await writeFile(path.join(directory, "result.json"), JSON.stringify(result))
  console.log(JSON.stringify({ directory, milliseconds, cpuMicroseconds, stages: result.stages.phases.map(({ name, wall }) => ({ name, wall })) }, null, 2))
} finally { clearTimeout(deadline); await runtime?.close(); restore(); await releaseHeadedProfileLock(lockPath, lock.token) }
process.exit(0)
