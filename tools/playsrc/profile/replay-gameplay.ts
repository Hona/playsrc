import { createHash } from "node:crypto"
import { readFile } from "node:fs/promises"
import path from "node:path"
import { encodeResourceBatch, parseResourceGraph } from "@playsrc/asset-store/graph"
import { objectPath } from "@playsrc/asset-store"
import { loadLocalConfig } from "../src/config"
import { acquireMap } from "../src/targets"
import { buildCollisionReplay } from "../src/collision-replay-build"
import { summarizeDistribution } from "./gameui-profile"
import { parseGameplayReplay, REPLAY_BYTES } from "./gameplay-replay"
import { ADMISSION_EVENT_BYTES, MAX_ADMISSION_EVENTS, decodeAdmissionMetrics } from "../../../games/tf2/browser/src/admission-metrics"

const hash = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex")
function require(value: unknown, message: string): asserts value { if (!value) throw new Error(message) }

export function verifyReplayHash(actual: string, recorded: string, key: string, baselineHashes?: Map<string, string>, collectBaseline = false) {
  if (baselineHashes && collectBaseline) baselineHashes.set(key, actual)
  require(actual === (baselineHashes ? baselineHashes.get(key) : recorded), `Complete replay transcript diverged at ${key}`)
  return actual !== recorded
}

// This is a CPU/WASM replay, not a hidden browser or a presentation benchmark.
// Build the opt-in collision-replay WASM feature; ordinary game builds contain
// neither the direct-sweep selector nor its per-plane diagnostics.
export async function replayGameplay(manifestPath: string, wasmPath: string, ticksOnly = false, displacement = false, baselineWasmPath?: string) {
  const started = performance.now()
  const captureBytes = await readFile(manifestPath)
  require(path.basename(manifestPath) === `${hash(captureBytes)}.manifest.json`, "Capture manifest hash mismatch")
  const capture = JSON.parse(captureBytes.toString())
  const linked = capture.identity?.gameplayReplay
  const graphIdentity = capture.identity?.applicationGeneration?.resourceRoot
  require(["playsrc-compositor-evidence-v1", "playsrc-compositor-evidence-v2", "playsrc-compositor-evidence-v3"].includes(capture.schema) && linked?.complete && /^[0-9a-f]{64}\.replay\.json$/u.test(linked.manifestFile)
    && /^[0-9a-f]{64}$/u.test(graphIdentity), "Replay must be linked to the recorded compiled content root")
  const manifestBytes = await readFile(path.join(path.dirname(manifestPath), linked.manifestFile))
  require(linked.manifestFile === `${hash(manifestBytes)}.replay.json`, "Replay manifest hash mismatch")
  const manifest = JSON.parse(manifestBytes.toString())
  require(manifest.schema === "playsrc-gameplay-replay-v1" && manifest.complete && /^[0-9a-f]{64}$/u.test(manifest.sha256) && manifest.bytes <= REPLAY_BYTES
    && manifest.file === `${manifest.sha256}.replay.bin`, "Replay manifest is incomplete or invalid")
  require(manifest.sha256 === linked.sha256 && manifest.bytes === linked.bytes, "Capture/replay linkage changed")
  const bytes = await readFile(path.join(path.dirname(manifestPath), manifest.file))
  require(bytes.length === manifest.bytes && hash(bytes) === manifest.sha256, "Replay journal hash mismatch")
  const replay = parseGameplayReplay(bytes)
  const checkpoint = manifest.checkpoint
  require(checkpoint && /^[0-9a-f]{64}$/u.test(checkpoint.configurationSha256) && [0, 1].includes(checkpoint.profile), "Replay content checkpoint invalid")
  const config = await loadLocalConfig()
  const graphBytes = await readFile(objectPath(config.assetDir, graphIdentity))
  require(hash(graphBytes) === graphIdentity, "Captured resource graph hash mismatch")
  const graph = parseResourceGraph(JSON.parse(graphBytes.toString("utf8")))
  require(graph.target === "pl_upward" || graph.target === "ctf_2fort", "Captured graph is not a supported local bot map")
  const map = await acquireMap(config, graph.target)
  const bsp = await readFile(path.join(config.sourceCacheDir, map.decoded.cachePath))
  require(hash(bsp) === replay.bspSha256, "Configured BSP differs from replay checkpoint")
  const wasm = await readFile(wasmPath)
  require(wasm.length <= 64 * 1024 * 1024, "Replay WASM bound exceeded")
  const baseline = baselineWasmPath ? await readFile(baselineWasmPath) : undefined
  require(!baseline || baseline.length <= 64 * 1024 * 1024, "Baseline WASM bound exceeded")
  // An explicit two-build comparison is not historical replay acceptance. It
  // retains the immutable commands but compares publications from the supplied
  // baseline binary, reporting every mismatch against the historical hashes.
  const baselineHashes = new Map<string, string>()
  const passes = []
  for (const reference of [true, false]) {
    const loaded = await WebAssembly.instantiate(reference && baseline ? baseline : wasm, { playsrc_metrics: { monotonic_milliseconds: () => performance.now() } })
    const e = loaded.instance.exports as Record<string, any>
    require(typeof e.playsrc_collision_replay_mode === "function", "Replay requires the collision-replay diagnostic build")
    const copy = (bytes: Uint8Array) => {
      const pointer = e.playsrc_alloc(bytes.length) >>> 0
      new Uint8Array(e.memory.buffer, pointer, bytes.length).set(bytes)
      return pointer
    }
    const output = (handle: number, name: string) => {
      const length = e[`playsrc_${name}_length`](handle)
      require(length >= 0 && length <= 64 * 1024 * 1024, "Replay output bound exceeded")
      if (name === "simulation_output") {
        const pointer = e.playsrc_simulation_output_pointer(handle) >>> 0
        require(pointer !== 0, "Replay publication pointer absent")
        return Buffer.from(new Uint8Array(e.memory.buffer, pointer, length))
      }
      const pointer = e.playsrc_alloc(Math.max(length, 1)) >>> 0
      try {
        require(e[`playsrc_${name}_copy`](handle, pointer, length) === length, "Replay output copy failed")
        return Buffer.from(new Uint8Array(e.memory.buffer, pointer, length))
      } finally { e.playsrc_free(pointer, Math.max(length, 1)) }
    }
    const sections: Array<{ pointer: number; length: number }> = []
    for (const descriptor of graph.chunks.filter(chunk => chunk.roles.includes("gameplay"))) {
      const bytes = await readFile(objectPath(config.assetDir, descriptor.encodedSha256))
      const batch = encodeResourceBatch([{ descriptor, bytes }])
      const pointer = copy(batch)
      require(e.playsrc_resource_decode(pointer, batch.length) === 1, "Replay resource authentication failed")
      e.playsrc_free(pointer, batch.length)
      const length = e.playsrc_resource_length()
      sections.push({ pointer: e.playsrc_resource_take() >>> 0, length })
    }
    const sectionBytes = Buffer.alloc(sections.length * 8)
    sections.forEach((section, index) => { sectionBytes.writeUInt32LE(section.pointer, index * 8); sectionBytes.writeUInt32LE(section.length, index * 8 + 4) })
    const table = copy(sectionBytes), digest = e.playsrc_alloc(32) >>> 0, bspPointer = copy(bsp)
    require(e.playsrc_resource_sections_hash(table, sections.length, digest) === checkpoint.configurationBytes
      && Buffer.from(new Uint8Array(e.memory.buffer, digest, 32)).toString("hex") === checkpoint.configurationSha256, "Configured resource set differs from recorded checkpoint")
    require(e.playsrc_collision_replay_mode(reference && !baseline ? (displacement ? 2 : 1) : 0) === 1, "Replay selector failed")
    const compileStarted = performance.now()
    const handle = e.playsrc_compile_map(bspPointer, bsp.length, checkpoint.profile, table, sections.length, digest, 1)
    require(e.playsrc_result_error(handle) === 0, `Replay checkpoint construction failed: ${e.playsrc_result_error(handle)}`)
    e.playsrc_result_release(handle)
    e.playsrc_presentation_release(handle)
    const compileMilliseconds = performance.now() - compileStarted
    require(e.playsrc_gameplay_replay_begin(handle) === 1, "Replay initial state rejected")
    let active = false, current: Buffer | undefined, activeTicks = 0, mutations = 0, verifiedTicks = 0, verifiedObserves = 0
    let expectedAttackTick = 0n, verifiedAttackPublications = 0
    let historicalHashMismatches = 0
    const verify = (actual: string, recorded: string, key: string) => {
      historicalHashMismatches += Number(verifyReplayHash(actual, recorded, key, baseline ? baselineHashes : undefined, reference))
    }
    const cpuStarted = process.cpuUsage(), rssBefore = process.memoryUsage().rss
    const observations: Array<{ milliseconds: number; ticks: number; counters: number[] }> = []
    const counterTotals = Array(11).fill(0)
    const captureCounters = () => Array.from({ length: counterTotals.length }, (_, index) => {
      const count = e.playsrc_collision_replay_counter(index)
      counterTotals[index] += count
      return count
    })
    const tickTimes: number[] = [], groups: Array<{ ticks: number; milliseconds: number }> = []
    let groupTicks = 0, groupMilliseconds = 0, groupIndex = 0
    for (const [index, record] of replay.records.entries()) {
      require(performance.now() - started < 170_000, "Offline replay exceeded its bounded deadline")
      const data = record.bytes
      if (record.kind === 7) {
        active = data.readUInt32LE(0) === 0
        e.playsrc_gameplay_replay_mark(handle, data.readUInt32LE(0))
      } else if (record.kind === 1 && !ticksOnly) {
        const command = data.subarray(24), pointer = copy(command)
        e.playsrc_collision_replay_reset()
        const began = performance.now()
        const success = e.playsrc_simulation_observe(handle, data.readDoubleLE(0), pointer, command.length, data.readUInt32LE(8), data.readBigUInt64LE(12))
        const milliseconds = performance.now() - began
        e.playsrc_free(pointer, command.length)
        require(success === 1, `Replay observe ${index} failed: ${e.playsrc_simulation_error()}`)
        current = output(handle, "simulation_output")
        if (active) {
          let ticks = 0
          for (let next = index + 1; replay.records[next]?.kind === 2; next++) ticks++
          observations.push({ milliseconds, ticks, counters: captureCounters() })
        }
      } else if (record.kind === 2) {
        verifiedTicks++
        if (data.readUInt32LE(52 + 28) & 8) expectedAttackTick = data.readBigUInt64LE(0)
        if (active) activeTicks++
        if (ticksOnly) {
          const command = data.subarray(52), pointer = copy(command)
          e.playsrc_collision_replay_reset()
          const began = performance.now()
          require(e.playsrc_game_advance(handle, pointer, command.length, 1) === 1, `Replay authoritative tick ${index} failed`)
          const elapsed = performance.now() - began
          e.playsrc_free(pointer, command.length)
          verify(hash(output(handle, "snapshot")), data.subarray(16, 48).toString("hex"), `tick:${data.readBigUInt64LE(0)}`)
          if (active) {
            captureCounters()
            tickTimes.push(elapsed)
            groupMilliseconds += elapsed
            if (++groupTicks === [2, 3, 4, 6][groupIndex % 4]) { groups.push({ ticks: groupTicks, milliseconds: groupMilliseconds }); groupTicks = 0; groupMilliseconds = 0; groupIndex++ }
          }
        }
      } else if (record.kind === 3 && !ticksOnly) {
        require(current, `Replay publication absent at record ${index}`)
        verify(hash(current), data.toString("hex"), `observe:${index}`)
        if (displacement && !baseline) {
          require(e.playsrc_gameplay_replay_attack_tick(handle) === expectedAttackTick, `Rust attack admission differs from the actual tick journal at observe ${index}`)
          verifiedAttackPublications++
        }
        verifiedObserves++
      } else if (record.kind === 4) {
        require(e.playsrc_team_select(handle, data.readUInt32LE(0)) === 1, "Replay team mutation failed"); mutations++
      } else if (record.kind === 5) {
        const pointer = e.playsrc_alloc(data.length)
        new Uint8Array(e.memory.buffer, pointer, data.length).set(data)
        require(e.playsrc_equipment_update(handle, pointer, data.length) === 1, "Replay equipment mutation failed")
        e.playsrc_free(pointer, data.length); mutations++
      } else if (record.kind === 5) {
        require(e.playsrc_player_set_position(handle, data.readFloatLE(0), data.readFloatLE(4), data.readFloatLE(8)) === 1, "Replay position mutation failed"); mutations++
      } else if (record.kind === 6) {
        const pointer = copy(data)
        require(e.playsrc_jump_configure(handle, pointer, data.length) === 1, "Replay course mutation failed")
        e.playsrc_free(pointer, data.length); mutations++
      } else if (record.kind === 7) {
        const pointer = copy(data)
        require(e.playsrc_entity_fire(handle, pointer, data.length) === 1, "Replay entity input failed")
        e.playsrc_free(pointer, data.length); mutations++
      }
    }
    require(e.playsrc_gameplay_replay_stop(handle) === 1, "Replay owner journal failed")
    const replayLength = e.playsrc_gameplay_replay_length(handle), replayPointer = e.playsrc_alloc(replayLength) >>> 0
    require(e.playsrc_gameplay_replay_copy(handle, 0, replayPointer, replayLength) === replayLength, "Replay journal copy failed")
    const actual = Buffer.from(new Uint8Array(e.memory.buffer, replayPointer, replayLength))
    e.playsrc_free(replayPointer, replayLength)
    require(actual.subarray(0, 88).equals(bytes.subarray(0, 88)), "Reconstructed initial checkpoint differs")
    if (!ticksOnly) {
      let active = false
      const recordedTicks = replay.records.filter(record => record.kind === 2)
      let tick = 0
      for (const record of parseGameplayReplay(actual).records) {
        if (record.kind === 7) active = record.bytes.readUInt32LE(0) === 0
        if (record.kind === 2) {
          const expected = recordedTicks[tick++]?.bytes
          require(expected && record.bytes.subarray(0, 8).equals(expected.subarray(0, 8)) && record.bytes.subarray(48).equals(expected.subarray(48)), "Admitted tick/command sequence changed")
          verify(record.bytes.subarray(16, 48).toString("hex"), expected.subarray(16, 48).toString("hex"), `tick:${record.bytes.readBigUInt64LE(0)}`)
          if (active) tickTimes.push(Number(record.bytes.readBigUInt64LE(8)) / 1e6)
        }
      }
      require(tick === recordedTicks.length, "Admitted tick count changed")
    }
    let admission
    if (typeof e.playsrc_admission_metrics_length === "function") {
      const length = e.playsrc_admission_metrics_length()
      require(length <= MAX_ADMISSION_EVENTS * ADMISSION_EVENT_BYTES, "Admission evidence exceeded its bound")
      const pointer = e.playsrc_alloc(Math.max(1, length)) >>> 0
      try {
        require(e.playsrc_admission_metrics_copy(pointer, length) === length, "Admission evidence copy failed")
        admission = { dropped: e.playsrc_admission_metrics_dropped(), events: decodeAdmissionMetrics(new DataView(e.memory.buffer, pointer, length)) }
      } finally { e.playsrc_free(pointer, Math.max(1, length)) }
    }
    passes.push({ mode: reference ? (baseline ? "supplied-baseline-build" : displacement ? "direct-displacement-reference" : "lazy-direct-sweep-reference") : "retained-candidate", compileMilliseconds, verifiedTicks, verifiedObserves, verifiedAttackPublications, activeTicks, mutations, historicalHashMismatches, admission,
      cpuMicroseconds: process.cpuUsage(cpuStarted), rssBefore, rssAfter: process.memoryUsage().rss,
      tickMilliseconds: summarizeDistribution(tickTimes), observeMilliseconds: summarizeDistribution(observations.map(value => value.milliseconds)), observations,
      counters: counterTotals,
      authoritativeTickGroups: groups, liveBytes: e.playsrc_memory_bytes(0) >>> 0, linearBytes: e.memory.buffer.byteLength })
    e.playsrc_dispose(handle)
    for (const section of sections) e.playsrc_resource_release(section.pointer, section.length)
    e.playsrc_free(table, sectionBytes.length); e.playsrc_free(digest, 32); e.playsrc_free(bspPointer, bsp.length)
  }
  return { schema: "playsrc-gameplay-replay-comparison-v1", historicalIncident: false, replaySha256: manifest.sha256, wasmSha256: hash(wasm), ticksOnly,
    comparison: baseline ? "two-builds-on-recorded-commands" : "recorded-publications", baselineWasmSha256: baseline ? hash(baseline) : null,
    counterNames: ["snapshotQueries", "hierarchyNodes", "objectCandidates", "convexSweeps", "clipPlanes", "vertexProjections", "displacementNodes", "displacementTriangles", "displacementSweeps", "displacementEdgeProjections", "displacementIntervalHits"], passes, totalMilliseconds: performance.now() - started }
}

if (import.meta.main) {
  const [identity, ...modes] = process.argv.slice(2)
  if (!identity || !/^[0-9a-f]{64}$/u.test(identity) || modes.some(mode => mode !== "--ticks" && mode !== "--displacement" && !mode.startsWith("--baseline-wasm="))) throw new Error("Usage: bun run replay:gameplay <compositor-manifest-sha256> [--ticks] [--displacement] [--baseline-wasm=<path>]")
  const baseline = modes.find(mode => mode.startsWith("--baseline-wasm="))?.slice("--baseline-wasm=".length)
  require(baseline === undefined || baseline.length > 0, "Baseline WASM path is empty")
  const config = await loadLocalConfig()
  const manifest = path.join(config.sourceCacheDir, "profiles", "upward-training-bots", "compositor-evidence", `${identity}.manifest.json`)
  const wasm = await buildCollisionReplay(config)
  console.log(JSON.stringify(await replayGameplay(manifest, wasm, modes.includes("--ticks"), modes.includes("--displacement"), baseline), null, 2))
}
