// Dynamic-light and shadow records in this file are adapted from Valve Source SDK 2013;
// the Source 1 SDK License applies.

export type TransparentFamily =
  | "world"
  | "displacement"
  | "detail"
  | "entity"
  | "particle"
  | "rope"
  | "beam"
  | "sprite"
  | "glow"

export type TransparentItem = Readonly<{
  identity: bigint
  family: TransparentFamily
  leaf: number
  sourceOrder: number
  ignoreDepth: boolean
  framebuffer: "none" | "power-of-two" | "full-frame"
}>

export type TransparentOperation =
  | Readonly<{ kind: "copy"; target: "power-of-two" | "full-frame"; consumer: bigint }>
  | Readonly<{ kind: "draw"; item: TransparentItem }>

export class FrameContractError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "FrameContractError"
  }
}

export function orderTransparentItems(
  leavesFrontToBack: readonly number[],
  items: readonly TransparentItem[],
): readonly TransparentOperation[] {
  const leafOrder = new Map<number, number>()
  for (const [order, leaf] of leavesFrontToBack.entries()) {
    if (!Number.isSafeInteger(leaf) || leaf < 0 || leafOrder.has(leaf)) {
      throw new FrameContractError("transparent leaf order is invalid")
    }
    leafOrder.set(leaf, order)
  }
  const identities = new Set<bigint>()
  for (const item of items) {
    if (
      item.identity < 0n
      || !identities.add(item.identity)
      || !leafOrder.has(item.leaf)
      || !Number.isSafeInteger(item.sourceOrder)
      || item.sourceOrder < 0
    ) throw new FrameContractError("transparent item is invalid")
  }
  const ordinary = items.filter((item) => !item.ignoreDepth)
  const ignoreDepth = items.filter((item) => item.ignoreDepth)
  const compare = (left: TransparentItem, right: TransparentItem) =>
    leafOrder.get(right.leaf)! - leafOrder.get(left.leaf)!
    || right.sourceOrder - left.sourceOrder
    || (left.identity < right.identity ? -1 : left.identity > right.identity ? 1 : 0)
  ordinary.sort(compare)
  ignoreDepth.sort(compare)
  const output: TransparentOperation[] = []
  for (const item of [...ordinary, ...ignoreDepth]) {
    if (item.framebuffer !== "none") output.push(Object.freeze({ kind: "copy", target: item.framebuffer, consumer: item.identity }))
    output.push(Object.freeze({ kind: "draw", item }))
  }
  return Object.freeze(output)
}

export type DepthStencilState = Readonly<{
  depthLoad: "load" | "clear"
  depthStore: "store" | "discard"
  depthClear: number
  depthCompare: "never" | "less" | "less-equal" | "equal" | "greater-equal" | "greater" | "always"
  depthWrite: boolean
  depthRange: readonly [number, number]
  depthBias: number
  depthBiasSlopeScale: number
  depthBiasClamp: number
  stencilLoad: "load" | "clear"
  stencilStore: "store" | "discard"
  stencilClear: number
  stencilReadMask: number
  stencilWriteMask: number
  stencilReference: number
}>

export type ClipPlane = Readonly<{ normal: readonly [number, number, number]; distance: number; keep: "positive" | "negative" }>

export type ViewState = Readonly<{
  identity: string
  cameraIdentity: string
  projectionIdentity: string
  viewport: readonly [number, number, number, number]
  colorTarget: string
  depthStencilTarget: string
  depthStencil: DepthStencilState
  clips: readonly ClipPlane[]
  fogIdentity: string | null
  visibilityIdentity: string
  drawFlags: number
  debug: string
  framebufferCopy: string | null
}>

function validIdentity(value: string): boolean {
  return value.length > 0 && value.length <= 1024
}

export function validateViewState(state: ViewState): ViewState {
  const depth = state.depthStencil
  if (
    ![
      state.identity, state.cameraIdentity, state.projectionIdentity, state.colorTarget,
      state.depthStencilTarget, state.visibilityIdentity, state.debug,
    ].every(validIdentity)
    || state.viewport.length !== 4
    || !state.viewport.every((value) => Number.isSafeInteger(value) && value >= 0)
    || depth.depthRange.length !== 2
    || !depth.depthRange.every(Number.isFinite)
    || depth.depthRange[0] < 0 || depth.depthRange[0] >= depth.depthRange[1] || depth.depthRange[1] > 1
    || ![depth.depthClear, depth.depthBias, depth.depthBiasSlopeScale, depth.depthBiasClamp].every(Number.isFinite)
    || depth.depthClear < 0 || depth.depthClear > 1
    || ![
      depth.stencilClear, depth.stencilReadMask, depth.stencilWriteMask, depth.stencilReference,
    ].every((value) => Number.isSafeInteger(value) && value >= 0 && value <= 0xff)
    || state.clips.length > 8
    || state.clips.some((clip) => clip.normal.length !== 3 || ![...clip.normal, clip.distance].every(Number.isFinite))
    || !Number.isSafeInteger(state.drawFlags) || state.drawFlags < 0
    || (state.fogIdentity !== null && !validIdentity(state.fogIdentity))
    || (state.framebufferCopy !== null && !validIdentity(state.framebufferCopy))
  ) throw new FrameContractError("view state is invalid")
  return state
}

export class SourceViewStack {
  readonly #maximumDepth: number
  readonly #states: ViewState[] = []

  constructor(maximumDepth: number) {
    if (!Number.isSafeInteger(maximumDepth) || maximumDepth < 1 || maximumDepth > 64) {
      throw new FrameContractError("view-stack depth limit is invalid")
    }
    this.#maximumDepth = maximumDepth
  }

  push(state: ViewState): ViewState {
    validateViewState(state)
    if (this.#states.length >= this.#maximumDepth) throw new FrameContractError("view-stack depth is exceeded")
    const retained = structuredClone(state) as ViewState
    this.#states.push(retained)
    return structuredClone(retained) as ViewState
  }

  pop(expectedIdentity: string): ViewState | null {
    const current = this.#states.at(-1)
    if (!current || current.identity !== expectedIdentity) throw new FrameContractError("view-stack pop identity differs")
    this.#states.pop()
    const prior = this.#states.at(-1)
    return prior ? structuredClone(prior) as ViewState : null
  }

  current(): ViewState | null {
    const current = this.#states.at(-1)
    return current ? structuredClone(current) as ViewState : null
  }

  get depth(): number {
    return this.#states.length
  }
}

export const FRAME_PHASES = Object.freeze([
  "validate-input",
  "interpolate-and-pose",
  "derive-views",
  "prune-and-sort",
  "upload",
  "auxiliary-views",
  "main-opaque-depth",
  "main-translucent",
  "viewmodels",
  "color-output",
  "diagnostics-and-capture",
  "finalize-commands",
  "submit",
] as const)

export type FramePhase = typeof FRAME_PHASES[number]
export type FramePhaseResult = Readonly<{ rollback?: () => void | Promise<void> }>
export type FramePhaseRunner = (phase: FramePhase) => void | FramePhaseResult | Promise<void | FramePhaseResult>

export type FrameGraphResult = Readonly<{ phases: readonly FramePhase[]; submitted: boolean; rollbacks: number }>

export async function executeFrameGraph(run: FramePhaseRunner): Promise<FrameGraphResult> {
  const phases: FramePhase[] = []
  const rollback: (() => void | Promise<void>)[] = []
  let submitted = false
  try {
    for (const phase of FRAME_PHASES) {
      const result = await run(phase) as FramePhaseResult | undefined
      phases.push(phase)
      if (phase === "submit") {
        submitted = true
        if (result && result.rollback) throw new FrameContractError("submitted frame cannot publish rollback work")
      } else if (result?.rollback) rollback.push(result.rollback)
    }
    return Object.freeze({ phases: Object.freeze(phases), submitted, rollbacks: 0 })
  } catch (error) {
    if (submitted) throw new FrameContractError(`frame failed after submission: ${String(error)}`)
    let count = 0
    let cleanupError: unknown
    for (let index = rollback.length - 1; index >= 0; index -= 1) {
      try { await rollback[index]!() } catch (cleanup) { cleanupError ??= cleanup } finally { count += 1 }
    }
    const cleanupDetail = cleanupError === undefined ? "" : `; rollback failure: ${String(cleanupError)}`
    throw new FrameContractError(`frame phase ${FRAME_PHASES[phases.length] ?? "complete"} failed after ${count} rollbacks: ${String(error)}${cleanupDetail}`)
  }
}

export type InterpolationPolicy = "linear" | "shortest-angle-degrees" | "quaternion" | "discrete"

export type InterpolationField = Readonly<{
  identity: string
  policy: InterpolationPolicy
  previous: number | readonly number[]
  current: number | readonly number[]
}>

export type InterpolationRequest = Readonly<{
  fraction: number
  discontinuities: ReadonlySet<string>
  fields: readonly InterpolationField[]
}>

function array(value: number | readonly number[]): readonly number[] {
  return typeof value === "number" ? [value] : value
}

function quaternion(value: readonly number[]): readonly [number, number, number, number] {
  if (value.length !== 4 || !value.every(Number.isFinite)) throw new FrameContractError("interpolation quaternion is invalid")
  const length = Math.hypot(...value)
  if (length <= 0) throw new FrameContractError("interpolation quaternion is zero")
  return [value[0]! / length, value[1]! / length, value[2]! / length, value[3]! / length]
}

function slerp(leftInput: readonly number[], rightInput: readonly number[], fraction: number): readonly number[] {
  const left = quaternion(leftInput)
  let right = quaternion(rightInput)
  let cosine = left.reduce((sum, value, index) => sum + value * right[index]!, 0)
  if (cosine < 0) {
    cosine = -cosine
    right = right.map((value) => -value) as unknown as typeof right
  }
  if (cosine > 0.9995) return quaternion(left.map((value, index) => value + (right[index]! - value) * fraction))
  const angle = Math.acos(Math.max(-1, Math.min(1, cosine)))
  const sine = Math.sin(angle)
  const a = Math.sin((1 - fraction) * angle) / sine
  const b = Math.sin(fraction * angle) / sine
  return left.map((value, index) => value * a + right[index]! * b)
}

export function interpolatePresentation(request: InterpolationRequest): ReadonlyMap<string, number | readonly number[]> {
  if (!Number.isFinite(request.fraction) || request.fraction < 0 || request.fraction > 1) {
    throw new FrameContractError("interpolation fraction is invalid")
  }
  const output = new Map<string, number | readonly number[]>()
  for (const field of request.fields) {
    if (!validIdentity(field.identity) || output.has(field.identity)) throw new FrameContractError("interpolation field identity is invalid")
    const previous = array(field.previous), current = array(field.current)
    if (previous.length !== current.length || previous.length < 1 || ![...previous, ...current].every(Number.isFinite)) {
      throw new FrameContractError("interpolation field values are invalid")
    }
    const snap = request.discontinuities.has(field.identity) || field.policy === "discrete"
    let value: readonly number[]
    if (snap) value = [...current]
    else if (field.policy === "linear") value = previous.map((item, index) => item + (current[index]! - item) * request.fraction)
    else if (field.policy === "shortest-angle-degrees") value = previous.map((item, index) => {
      const delta = ((current[index]! - item + 540) % 360) - 180
      return item + delta * request.fraction
    })
    else if (field.policy === "quaternion") value = slerp(previous, current, request.fraction)
    else throw new FrameContractError("interpolation policy is invalid")
    output.set(field.identity, typeof field.current === "number" ? value[0]! : Object.freeze(value))
  }
  return output
}

export type DynamicLightInput = Readonly<{
  identity: bigint
  kind: "dlight" | "elight"
  key: number
  flags: number
  origin: readonly [number, number, number]
  radius: number
  color: readonly [number, number, number]
  colorExponent: number
  dieTime: number
  decayPerSecond: number
  minimumLight: number
  style: number
  styleScalar: number
  direction: readonly [number, number, number]
  innerAngle: number
  outerAngle: number
  currentTime: number
}>

export type ProjectedLightInput = Readonly<{
  origin: readonly [number, number, number]
  orientation: readonly [number, number, number, number]
  nearZ: number
  farZ: number
  horizontalFovDegrees: number
  verticalFovDegrees: number
  attenuation: readonly [number, number, number]
  color: readonly [number, number, number, number]
  textureIdentity: string
  textureFrame: number
  enableShadows: boolean
  mapResolution: number
  filterSize: number
  slopeScaleDepthBias: number
  depthBias: number
  jitterSeed: number
  shadowAttenuation: number
  quality: number
  scissor: readonly [number, number, number, number] | null
}>

export type ShadowInput = Readonly<{
  identity: bigint
  kind: "simple" | "render-to-texture" | "render-to-texture-dynamic" | "render-to-depth-texture"
  caster: bigint
  receivers: readonly bigint[]
  worldToShadow: Float32Array
  projectionOrigin: readonly [number, number, number]
  projectionDirection: readonly [number, number, number]
  projectionSize: readonly [number, number]
  casterOrigin: readonly [number, number, number]
  casterBounds: readonly [readonly [number, number, number], readonly [number, number, number]]
  leaves: readonly number[]
  maximumHeight: number
  falloffOffset: number
  falloffAmount: number
  falloffBias: number
  textureIdentity: string
  textureOrigin: readonly [number, number]
  textureSize: readonly [number, number]
  clipPlanes: readonly ClipPlane[]
  depthBias: number
  slopeScaleBias: number
  enabled: boolean
  viewIdentity: string
  projectedLight: ProjectedLightInput | null
}>

export function validateDynamicLights(input: readonly DynamicLightInput[], maximum: number): readonly DynamicLightInput[] {
  if (!Number.isSafeInteger(maximum) || maximum < 0 || input.length > maximum) throw new FrameContractError("dynamic-light bound is exceeded")
  const identities = new Set<bigint>()
  for (const light of input) {
    if (
      light.identity < 0n || !identities.add(light.identity)
      || ![...light.origin, ...light.direction, light.radius, light.dieTime, light.decayPerSecond,
        light.minimumLight, light.styleScalar, light.innerAngle, light.outerAngle, light.currentTime].every(Number.isFinite)
      || light.color.some((value) => !Number.isSafeInteger(value) || value < 0 || value > 255)
      || !Number.isSafeInteger(light.colorExponent) || light.colorExponent < -128 || light.colorExponent > 127
      || light.radius < 0 || light.style < 0 || light.style > 63 || !Number.isSafeInteger(light.style)
      || !Number.isSafeInteger(light.key) || !Number.isSafeInteger(light.flags) || light.flags < 0 || (light.flags & ~0xf) !== 0
      || light.innerAngle < 0 || light.outerAngle < 0 || light.innerAngle > light.outerAngle
      || (light.outerAngle > 0 && Math.hypot(...light.direction) === 0)
    ) throw new FrameContractError("dynamic-light input is invalid")
  }
  return input
}

export function validateShadows(input: readonly ShadowInput[], maximum: number): readonly ShadowInput[] {
  if (!Number.isSafeInteger(maximum) || maximum < 0 || input.length > maximum) throw new FrameContractError("shadow bound is exceeded")
  const identities = new Set<bigint>()
  for (const shadow of input) {
    const projected = shadow.projectedLight
    if (
      shadow.identity < 0n || shadow.caster < 0n || !identities.add(shadow.identity)
      || shadow.receivers.some((receiver) => receiver < 0n)
      || new Set(shadow.receivers).size !== shadow.receivers.length
      || shadow.worldToShadow.length !== 16 || ![...shadow.worldToShadow].every(Number.isFinite)
      || !validIdentity(shadow.textureIdentity) || !validIdentity(shadow.viewIdentity)
      || ![...shadow.projectionOrigin, ...shadow.projectionDirection, ...shadow.projectionSize,
        ...shadow.casterOrigin, ...shadow.casterBounds[0], ...shadow.casterBounds[1], ...shadow.textureOrigin,
        ...shadow.textureSize, shadow.maximumHeight, shadow.falloffOffset, shadow.falloffAmount,
        shadow.falloffBias, shadow.depthBias, shadow.slopeScaleBias].every(Number.isFinite)
      || shadow.casterBounds.some((side, sideIndex) => side.some((value, axis) => sideIndex === 1 && value < shadow.casterBounds[0][axis]!))
      || shadow.leaves.some((leaf) => !Number.isSafeInteger(leaf) || leaf < 0) || new Set(shadow.leaves).size !== shadow.leaves.length
      || shadow.projectionSize.some((value) => value <= 0) || shadow.textureSize.some((value) => value <= 0)
      || shadow.maximumHeight < 0 || shadow.falloffAmount < 0 || shadow.falloffBias < 0 || shadow.falloffBias > 255 || !Number.isSafeInteger(shadow.falloffBias)
      || shadow.clipPlanes.length > 8
      || (shadow.kind === "render-to-depth-texture") !== (projected !== null)
      || (projected !== null && (
        ![...projected.origin, ...projected.orientation, projected.nearZ, projected.farZ,
          projected.horizontalFovDegrees, projected.verticalFovDegrees, ...projected.attenuation,
          ...projected.color, projected.mapResolution, projected.filterSize, projected.slopeScaleDepthBias,
          projected.depthBias, projected.jitterSeed, projected.shadowAttenuation].every(Number.isFinite)
        || projected.orientation.length !== 4 || Math.abs(projected.orientation.reduce((sum,value)=>sum+value*value,0)-1)>1e-4
        || projected.nearZ <= 0 || projected.farZ <= projected.nearZ
        || projected.horizontalFovDegrees <= 0 || projected.horizontalFovDegrees >= 180
        || projected.verticalFovDegrees <= 0 || projected.verticalFovDegrees >= 180
        || !validIdentity(projected.textureIdentity) || !Number.isSafeInteger(projected.textureFrame) || projected.textureFrame < 0
        || !Number.isSafeInteger(projected.mapResolution) || projected.mapResolution < 1
        || !Number.isSafeInteger(projected.quality) || projected.quality < 0
        || (projected.scissor !== null && (projected.scissor.some((value)=>!Number.isSafeInteger(value))
          || projected.scissor[2] < projected.scissor[0] || projected.scissor[3] < projected.scissor[1]))
      ))
    ) throw new FrameContractError("shadow input is invalid")
  }
  return input
}

export function visibleTriangleIndices(
  indices: Uint32Array,
  triangleFaces: Uint32Array,
  visibleFaces: ReadonlySet<number>,
  faceOrder?: ReadonlyMap<number, number>,
): Uint32Array {
  if (indices.length % 3 !== 0 || triangleFaces.length !== indices.length / 3) {
    throw new FrameContractError("world-face index input is invalid")
  }
  const triangles = Array.from({ length: triangleFaces.length }, (_, triangle) => triangle)
    .filter((triangle) => visibleFaces.has(triangleFaces[triangle]!))
  if (faceOrder) triangles.sort((left, right) =>
    (faceOrder.get(triangleFaces[right]!) ?? -1) - (faceOrder.get(triangleFaces[left]!) ?? -1)
    || left - right)
  const output = new Uint32Array(triangles.length * 3)
  triangles.forEach((triangle, target) => output.set(indices.subarray(triangle * 3, triangle * 3 + 3), target * 3))
  return output
}
