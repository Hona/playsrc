import { expect, test } from "bun:test"
import { sanityViewAngles, selectSanityCheckpoints } from "../profile/map-sanity"

const samples = [
  { leaf: 1, cluster: 1, area: 1, position: [128, 0, 64] as const },
  { leaf: 2, cluster: 2, area: 1, position: [32, 32, 160] as const },
  { leaf: 3, cluster: 3, area: 2, position: [16, 16, 256] as const },
  { leaf: 4, cluster: 4, area: 3, position: [1200, 0, 512] as const },
  { leaf: 5, cluster: 5, area: 4, position: [512, -512, -32] as const },
]

test("selects bounded authored floor, water, bridge, and intelligence viewpoints", () => {
  const checkpoints = selectSanityCheckpoints({
    target: "ctf_2fort",
    spawn: [0, 0, 64],
    samples,
    water: [{ surfaceZ: 64, bounds: [[-64, -64, 0], [64, 64, 64]] }],
    skyArea: null,
    objectives: [[512, -512, -64]],
    pickups: [[128, 0, 32]],
  })
  expect(checkpoints.map((checkpoint) => checkpoint.kind)).toEqual(["spawn", "floor", "water", "bridge", "objective"])
  expect(checkpoints.find((checkpoint) => checkpoint.kind === "objective")?.leaf).toBe(5)
  expect(checkpoints.find((checkpoint) => checkpoint.kind === "bridge")?.position[2]).toBeGreaterThanOrEqual(160)
})

test("selects Upward terrain directly from authored empty-space samples", () => {
  const checkpoints = selectSanityCheckpoints({
    target: "pl_upward",
    spawn: [0, 0, 64],
    samples,
    water: [],
    skyArea: null,
    objectives: [],
    pickups: [],
  })
  expect(checkpoints.find((checkpoint) => checkpoint.kind === "outdoor-terrain")?.position).toEqual([1200, 0, 512])
})

test("rejects missing configured map landmarks instead of inventing cameras", () => {
  expect(() => selectSanityCheckpoints({ target: "jump_beef", spawn: [0, 0, 0], samples: [], water: [], skyArea: null, objectives: [], pickups: [] }))
    .toThrow("no authored empty-space camera samples")
  expect(() => selectSanityCheckpoints({ target: "ctf_2fort", spawn: [0, 0, 0], samples, water: [], skyArea: null, objectives: [], pickups: [] }))
    .toThrow("no authored intelligence objective checkpoint")
})

test("never teleports gameplay into authored 3D-sky water volumes", () => {
  const checkpoints = selectSanityCheckpoints({
    target: "pl_upward",
    spawn: [0, 0, 64],
    samples,
    water: [{ surfaceZ: 64, bounds: [[-64, -64, 0], [64, 64, 64]], areas: [7] }],
    skyArea: 7,
    objectives: [],
    pickups: [],
  })
  expect(checkpoints.some((checkpoint) => checkpoint.kind === "water")).toBe(false)
})

test("retains a distinct intelligence viewpoint when its nearest sample is already occupied", () => {
  const checkpoints = selectSanityCheckpoints({
    target: "ctf_2fort",
    spawn: [0, 0, 64],
    samples,
    water: [],
    skyArea: null,
    objectives: [[128, 0, 64]],
    pickups: [[128, 0, 64]],
  })
  expect(checkpoints.filter((checkpoint) => checkpoint.kind === "objective")).toHaveLength(1)
  expect(checkpoints.find((checkpoint) => checkpoint.kind === "objective")?.leaf).not.toBe(
    checkpoints.find((checkpoint) => checkpoint.kind === "floor")?.leaf,
  )
})

test("derives the 2Fort bridge from both authored intelligence locations without synthetic water", () => {
  const checkpoints = selectSanityCheckpoints({
    target: "ctf_2fort",
    spawn: [0, 0, 64],
    samples,
    water: [],
    skyArea: null,
    objectives: [[512, -512, -64], [-512, 512, -64]],
    pickups: [[128, 0, 32]],
  })
  expect(checkpoints.find((checkpoint) => checkpoint.kind === "bridge")?.focus).toEqual([0, 0, 128])
  expect(checkpoints.some((checkpoint) => checkpoint.kind === "objective")).toBe(true)
})

test("derives exact Source pitch and yaw without changing simulation or rendering", () => {
  expect(sanityViewAngles([0, 0, 0], [0, 256, -256])).toEqual({ pitch: 45, yaw: 90 })
  expect(sanityViewAngles([0, 0, 0], [-128, 0, 0])).toEqual({ pitch: 0, yaw: 180 })
  expect(() => sanityViewAngles([1, 2, 3], [1, 2, 3])).toThrow("camera direction is invalid")
})
