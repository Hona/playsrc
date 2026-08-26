import { createHash } from "node:crypto"
import { readFile } from "node:fs/promises"
import path from "node:path"
import { encodeResourceBatch, parseResourceGraph } from "@playsrc/asset-store/graph"
import { loadLocalConfig } from "../src/config"
import { acquireMap } from "../src/targets"
import { summarizeDistribution } from "./gameui-profile"
import { parseGameplayReplay, REPLAY_BYTES } from "./gameplay-replay"

const hash = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex")
function require(value: unknown, message: string): asserts value { if (!value) throw new Error(message) }

// This is a CPU/WASM replay, not a hidden browser or a presentation benchmark.
// Build the opt-in collision-replay WASM feature; ordinary game builds contain
// neither the direct-sweep selector nor its per-plane diagnostics.
export async function replayGameplay(manifestPath: string, wasmPath: string, ticksOnly = false) {
  const started = performance.now()
  const manifestBytes = await readFile(manifestPath)
  require(path.basename(manifestPath) === `${hash(manifestBytes)}.replay.json`, "Replay manifest hash mismatch")
  const manifest = JSON.parse(manifestBytes.toString())
  require(manifest.schema === "playsrc-gameplay-replay-v1" && manifest.complete && manifest.bytes <= REPLAY_BYTES
    && manifest.file === `${manifest.sha256}.replay.bin`, "Replay manifest is incomplete or invalid")
  const bytes = await readFile(path.join(path.dirname(manifestPath), manifest.file))
  require(bytes.length === manifest.bytes && hash(bytes) === manifest.sha256, "Replay journal hash mismatch")
  const replay = parseGameplayReplay(bytes)
  const checkpoint = manifest.checkpoint
  require(checkpoint && /^[0-9a-f]{64}$/u.test(checkpoint.configurationSha256) && [0, 1].includes(checkpoint.profile), "Replay content checkpoint invalid")
  const config = await loadLocalConfig()
  const map = await acquireMap(config, "pl_upward")
  const bsp = await readFile(path.join(config.sourceCacheDir, map.decoded.cachePath))
  require(hash(bsp) === replay.bspSha256, "Configured Upward BSP differs from replay checkpoint")
  const graph = parseResourceGraph(JSON.parse(await readFile(path.join(config.sourceCacheDir, "browser-bundles/pl_upward.graph.json"), "utf8")))
  const wasm = await readFile(wasmPath)
  require(wasm.length <= 64 * 1024 * 1024, "Replay WASM bound exceeded")
  const loaded = await WebAssembly.instantiate(wasm, { playsrc_metrics: { monotonic_milliseconds: () => performance.now() } })
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
    const bytes = await readFile(path.join(config.sourceCacheDir, "browser-bundles/pl_upward.graph/objects", descriptor.encodedSha256))
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
  const passes = []
  for (const reference of [true, false]) {
    require(e.playsrc_collision_replay_mode(Number(reference)) === 1, "Replay selector failed")
    const compileStarted = performance.now()
    const handle = e.playsrc_compile_map(bspPointer, bsp.length, checkpoint.profile, table, sections.length, digest)
    require(e.playsrc_result_error(handle) === 0, `Replay checkpoint construction failed: ${e.playsrc_result_error(handle)}`)
    e.playsrc_result_release(handle)
    e.playsrc_presentation_release(handle)
    const compileMilliseconds = performance.now() - compileStarted
    require(e.playsrc_gameplay_replay_begin(handle) === 1, "Replay initial state rejected")
    let active = false, current: Buffer | undefined, activeTicks = 0, mutations = 0, verifiedTicks = 0, verifiedObserves = 0
    const observations: Array<{ milliseconds: number; ticks: number; counters: number[] }> = []
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
          observations.push({ milliseconds, ticks, counters: Array.from({ length: 6 }, (_, index) => e.playsrc_collision_replay_counter(index)) })
        }
      } else if (record.kind === 2) {
        verifiedTicks++
        if (active) activeTicks++
        if (ticksOnly) {
          const command = data.subarray(52), pointer = copy(command)
          e.playsrc_collision_replay_reset()
          const began = performance.now()
          require(e.playsrc_game_advance(handle, pointer, command.length, 1) === 1, `Replay authoritative tick ${index} failed`)
          const elapsed = performance.now() - began
          e.playsrc_free(pointer, command.length)
          require(hash(output(handle, "snapshot")) === data.subarray(16, 48).toString("hex"), `Authoritative tick/event transcript diverged at ${data.readBigUInt64LE(0)} (${reference ? "reference" : "candidate"})`)
          if (active) {
            tickTimes.push(elapsed)
            groupMilliseconds += elapsed
            if (++groupTicks === [2, 3, 4, 6][groupIndex % 4]) { groups.push({ ticks: groupTicks, milliseconds: groupMilliseconds }); groupTicks = 0; groupMilliseconds = 0; groupIndex++ }
          }
        }
      } else if (record.kind === 3 && !ticksOnly) {
        require(current && hash(current) === data.toString("hex"), `Complete host/tick/event publication diverged at record ${index} (${reference ? "reference" : "candidate"})`)
        verifiedObserves++
      } else if (record.kind === 4) {
        require(e.playsrc_team_select(handle, data.readUInt32LE(0)) === 1, "Replay team mutation failed"); mutations++
      } else if (record.kind === 5) {
        require(e.playsrc_player_set_position(handle, data.readFloatLE(0), data.readFloatLE(4), data.readFloatLE(8)) === 1, "Replay position mutation failed"); mutations++
      } else if (record.kind === 6) {
        const pointer = copy(data)
        require(e.playsrc_jump_configure(handle, pointer, data.length) === 1, "Replay course mutation failed")
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
      for (const record of parseGameplayReplay(actual).records) {
        if (record.kind === 7) active = record.bytes.readUInt32LE(0) === 0
        if (active && record.kind === 2) tickTimes.push(Number(record.bytes.readBigUInt64LE(8)) / 1e6)
      }
    }
    passes.push({ mode: reference ? "direct-sweep-reference" : "retained-candidate", compileMilliseconds, verifiedTicks, verifiedObserves, activeTicks, mutations,
      tickMilliseconds: summarizeDistribution(tickTimes), observeMilliseconds: summarizeDistribution(observations.map(value => value.milliseconds)), observations,
      counters: observations.reduce((sum, value) => sum.map((count, index) => count + value.counters[index]!), [0, 0, 0, 0, 0, 0]),
      authoritativeTickGroups: groups, liveBytes: e.playsrc_memory_bytes(0) >>> 0, linearBytes: e.memory.buffer.byteLength })
    e.playsrc_dispose(handle)
  }
  for (const section of sections) e.playsrc_resource_release(section.pointer, section.length)
  e.playsrc_free(table, sectionBytes.length); e.playsrc_free(digest, 32); e.playsrc_free(bspPointer, bsp.length)
  return { schema: "playsrc-gameplay-replay-comparison-v1", historicalIncident: false, replaySha256: manifest.sha256, wasmSha256: hash(wasm), ticksOnly,
    counterNames: ["snapshotQueries", "hierarchyNodes", "objectCandidates", "convexSweeps", "clipPlanes", "vertexProjections"], passes, totalMilliseconds: performance.now() - started }
}

if (import.meta.main) {
  const [manifest, wasm, mode] = process.argv.slice(2)
  if (!manifest || !wasm || (mode && mode !== "--ticks")) throw new Error("Usage: bun tools/playsrc/profile/replay-gameplay.ts <sha256.replay.json> <collision-replay.wasm> [--ticks]")
  console.log(JSON.stringify(await replayGameplay(manifest, wasm, mode === "--ticks"), null, 2))
}
