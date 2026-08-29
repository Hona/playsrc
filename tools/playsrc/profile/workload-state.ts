/** Only semantic state, never timings, process addresses or frame counters. */
export function canonicalWorkloadState(value: unknown): any {
  if (typeof value === "bigint") return { uint64: value.toString() }
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new Error("Non-finite workload state")
    if (Number.isSafeInteger(value) && !Object.is(value, -0)) return value
    const bytes = new Uint8Array(8); new DataView(bytes.buffer).setFloat64(0, value, true)
    return { float64le: Array.from(bytes, byte => byte.toString(16).padStart(2, "0")).join("") }
  }
  if (value === undefined) return { undefined: true }
  if (value === null || typeof value === "string" || typeof value === "boolean") return value
  if (ArrayBuffer.isView(value)) return { view: value.constructor.name, bytes: Array.from(new Uint8Array(value.buffer, value.byteOffset, value.byteLength)) }
  if (Array.isArray(value)) return value.map(canonicalWorkloadState)
  if (value instanceof Map) return { map: [...value].map(([key, entry]) => [canonicalWorkloadState(key), canonicalWorkloadState(entry)]) }
  if (typeof value === "object") return Object.fromEntries(Object.entries(value).map(([key, entry]) => [key, canonicalWorkloadState(entry)]))
  throw new Error("Unsupported workload state")
}
export function workloadState(frame: any) {
  if (frame?.schema !== 3 || !frame.round) throw new Error("Missing actual workload state")
  // All semantic state belongs to the displayed prepared publication. The live
  // producer may already be preparing another tick; keep that backlog in the
  // measurement, never splice its round state into this older visible frame.
  return canonicalWorkloadState({ schema: 3, visibleSourceTick: String(frame.tick), round: structuredClone(frame.round), frame: {
    tick: frame.tick, playerClass: frame.playerClass, weapon: frame.weapon,
    position: frame.position, yaw: frame.yaw, pitch: frame.pitch,
    drawSurfaces: frame.drawSurfaces, leaves: frame.leaves, props: frame.props,
    skySurfaces: frame.skySurfaces, skyProps: frame.skyProps,
    mainVisibilityIdentity: frame.mainVisibilityIdentity, skyVisibilityIdentity: frame.skyVisibilityIdentity,
    actors: { bots: frame.detail.bots, buildings: frame.detail.buildings, pickups: frame.detail.pickups },
    particles: { items: frame.detail.particleItems, batches: frame.detail.particleBatches, inputs: frame.particleInputs },
    models: frame.modelInputs,
    scene: frame.sceneInputs,
  } })
}
export function assertMatchingWorkloadState(expected: unknown, actual: unknown) {
  // Live asynchronous raster feedback is not replayed by the command clock.
  // Its halo colors still belong to this strict scene witness: never omit them
  // or turn a rejected scene into a pass merely because gameplay hashes match.
  if (JSON.stringify(expected) !== JSON.stringify(actual)) throw new Error("Actual initial bot/round/model/scene state differs; comparison rejected")
}
