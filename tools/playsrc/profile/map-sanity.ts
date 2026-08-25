export type SanityPosition = readonly [number, number, number]

export type SanityCoverageSample = Readonly<{
  leaf: number
  cluster: number
  area: number
  position: SanityPosition
}>

export type SanityWaterVolume = Readonly<{
  surfaceZ: number
  bounds: readonly [SanityPosition, SanityPosition]
  areas?: readonly number[]
}>

export type SanityLandmarks = Readonly<{
  target: string
  spawn: SanityPosition
  samples: readonly SanityCoverageSample[]
  water: readonly SanityWaterVolume[]
  skyArea: number | null
  objectives: readonly SanityPosition[]
  pickups: readonly SanityPosition[]
}>

export type SanityCheckpoint = Readonly<{
  kind: "spawn" | "outdoor-terrain" | "floor" | "water" | "bridge" | "objective"
  position: SanityPosition
  focus: SanityPosition
  leaf: number | null
}>

const distance = (left: SanityPosition, right: SanityPosition): number =>
  Math.hypot(left[0] - right[0], left[1] - right[1], left[2] - right[2])

function nearest(samples: readonly SanityCoverageSample[], target: SanityPosition): SanityCoverageSample | undefined {
  return samples.reduce<SanityCoverageSample | undefined>((selected, candidate) =>
    selected === undefined || distance(candidate.position, target) < distance(selected.position, target)
      ? candidate
      : selected, undefined)
}

function center(volume: SanityWaterVolume): SanityPosition {
  return Object.freeze([
    (volume.bounds[0][0] + volume.bounds[1][0]) / 2,
    (volume.bounds[0][1] + volume.bounds[1][1]) / 2,
    volume.surfaceZ,
  ])
}

export function selectSanityCheckpoints(landmarks: SanityLandmarks): readonly SanityCheckpoint[] {
  if (landmarks.samples.length === 0) throw new Error(`${landmarks.target} has no authored empty-space camera samples`)
  const checkpoints: SanityCheckpoint[] = [{
    kind: "spawn",
    position: landmarks.spawn,
    focus: Object.freeze([landmarks.spawn[0] + 256, landmarks.spawn[1], landmarks.spawn[2] - 48]),
    leaf: null,
  }]
  const used = new Set<string>()
  const insert = (kind: SanityCheckpoint["kind"], sample: SanityCoverageSample | undefined, focus: SanityPosition): void => {
    if (!sample) return
    const key = sample.position.join(",")
    if (used.has(key)) return
    used.add(key)
    checkpoints.push(Object.freeze({ kind, position: sample.position, focus, leaf: sample.leaf }))
  }

  const floorAnchor = landmarks.pickups[0] ?? landmarks.spawn
  insert("floor", nearest(landmarks.samples.filter((sample) => distance(sample.position, landmarks.spawn) > 96), floorAnchor),
    Object.freeze([floorAnchor[0], floorAnchor[1], floorAnchor[2] - 48]))

  if (landmarks.target === "pl_upward") {
    const outdoor = landmarks.samples
      .filter((sample) => distance(sample.position, landmarks.spawn) > 768)
      .reduce<SanityCoverageSample | undefined>((selected, sample) =>
        selected === undefined || sample.position[2] > selected.position[2] ? sample : selected, undefined)
    if (!outdoor) throw new Error("pl_upward has no authored outdoor terrain sample")
    insert("outdoor-terrain", outdoor, Object.freeze([landmarks.spawn[0], landmarks.spawn[1], outdoor.position[2] - 96]))
  }

  const volume = landmarks.water[0]
  if (volume && (landmarks.skyArea === null || !volume.areas?.includes(landmarks.skyArea))) {
    const surface = center(volume)
    const above = landmarks.samples.filter((sample) => sample.position[2] >= volume.surfaceZ - 24)
    insert("water", nearest(above, Object.freeze([surface[0], surface[1], surface[2] + 48])), surface)
    if (landmarks.target === "ctf_2fort") {
      const elevated = landmarks.samples.filter((sample) => sample.position[2] >= volume.surfaceZ + 96)
      insert("bridge", nearest(elevated, Object.freeze([surface[0], surface[1], surface[2] + 192])), surface)
    }
  }

  if (landmarks.target === "ctf_2fort" && !checkpoints.some((checkpoint) => checkpoint.kind === "bridge")) {
    const [red, blue] = landmarks.objectives
    if (red && blue) {
      const midpoint: SanityPosition = Object.freeze([
        (red[0] + blue[0]) / 2,
        (red[1] + blue[1]) / 2,
        Math.max(red[2], blue[2]) + 192,
      ])
      insert("bridge", nearest(landmarks.samples.filter((sample) => !used.has(sample.position.join(","))), midpoint), midpoint)
    }
  }

  const objective = landmarks.objectives[0]
  if (objective) {
    insert("objective", nearest(landmarks.samples.filter((sample) => !used.has(sample.position.join(","))), objective), objective)
  }

  if (landmarks.target === "ctf_2fort" && !checkpoints.some((checkpoint) => checkpoint.kind === "objective")) {
    throw new Error("ctf_2fort has no authored intelligence objective checkpoint")
  }
  return Object.freeze(checkpoints)
}

export function sanityViewAngles(position: SanityPosition, focus: SanityPosition): Readonly<{ pitch: number; yaw: number }> {
  const x = focus[0] - position[0]
  const y = focus[1] - position[1]
  const z = focus[2] - position[2]
  if (![x, y, z].every(Number.isFinite) || Math.hypot(x, y, z) < 0.001) {
    throw new Error("authored sanity camera direction is invalid")
  }
  return Object.freeze({
    pitch: Number((-Math.atan2(z, Math.hypot(x, y)) * 180 / Math.PI).toFixed(4)),
    yaw: Number((Math.atan2(y, x) * 180 / Math.PI).toFixed(4)),
  })
}
