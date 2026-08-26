export type ApplicationOperation = Readonly<{
  generation: number
  signal: AbortSignal
}>

export function admitBotConfiguration<Configuration>(
  configuration: Configuration | undefined,
  mapIdentity: string,
  dependencies: ReadonlyMap<string, unknown>,
): Configuration | undefined {
  return dependencies.has(`maps/${mapIdentity}.nav`) ? configuration : undefined
}

export class ApplicationFrameClock {
  #current = 0

  get current(): number {
    return this.#current
  }

  admit(timeSeconds: number): number {
    if (!Number.isFinite(timeSeconds) || timeSeconds < 0) {
      throw new TypeError("Application frame timestamp is invalid")
    }
    this.#current = Math.max(this.#current, timeSeconds)
    return this.#current
  }
}

export class ApplicationOperationLedger {
  #generation = 0
  #controller?: AbortController
  #operation?: ApplicationOperation

  begin(): ApplicationOperation {
    this.#controller?.abort()
    this.#controller = new AbortController()
    this.#operation = Object.freeze({
      generation: ++this.#generation,
      signal: this.#controller.signal,
    })
    return this.#operation
  }

  current(operation: ApplicationOperation): boolean {
    return this.#operation === operation && !operation.signal.aborted
  }

  complete(operation: ApplicationOperation): boolean {
    if (!this.current(operation)) return false
    this.#operation = undefined
    this.#controller = undefined
    return true
  }

  cancel(): void {
    this.#controller?.abort()
    this.#controller = undefined
    this.#operation = undefined
  }
}

export type PresentationGeneration<Mapper, Encoder> = Readonly<{
  generation: number
  mapper: Mapper
  encoder: Encoder
}>

export function currentPresentationGeneration<Mapper, Encoder>(
  owned: PresentationGeneration<Mapper, Encoder>,
  generation: number,
  mapper: Mapper | undefined,
  encoder: Encoder,
): boolean {
  return owned.generation === generation && owned.mapper === mapper && owned.encoder === encoder
}

export function selectAuthoredSky(visibility: number, hasController: boolean): boolean {
  return visibility === 2 && hasController
}

export function routeApplicationEscape(input: Readonly<{
  code: string
  repeat: boolean
  phase: string
  gameUi: string
  optionsVisible: boolean
}>): "options" | "activate" | "resume" | "ignore" {
  if (input.code !== "Escape" || input.repeat) return "ignore"
  if (input.optionsVisible) return "options"
  if (input.phase !== "Ready") return "ignore"
  if (input.gameUi === "in-game") return "activate"
  if (input.gameUi === "pause") return "resume"
  return "ignore"
}

type EyePosition = readonly [number, number, number]

type EyeSample = Readonly<{
  tick: bigint
  position: EyePosition
}>

function validateEye(position: EyePosition): void {
  if (position.length !== 3 || !position.every(Number.isFinite)) {
    throw new TypeError("Predicted eye sample is invalid")
  }
}

export class PredictedEyeInterpolation {
  #previous?: EyeSample
  #current?: EyeSample

  reset(tick: bigint, position: EyePosition): void {
    validateEye(position)
    if (tick < 0n) throw new RangeError("Predicted eye tick reversed")
    const value = Object.freeze({ tick, position })
    this.#previous = value
    this.#current = value
  }

  admit(tick: bigint, position: EyePosition, discontinuity = false): void {
    validateEye(position)
    if (!this.#current) {
      this.reset(tick, position)
      return
    }
    if (tick <= this.#current.tick) throw new RangeError("Predicted eye tick reversed")
    const next = Object.freeze({ tick, position })
    this.#previous = discontinuity ? next : this.#current
    this.#current = next
  }

  sample(interpolation: number): EyePosition | undefined {
    if (!Number.isFinite(interpolation) || interpolation < 0 || interpolation > 1) {
      throw new TypeError("Predicted eye interpolation phase is invalid")
    }
    const current = this.#current
    const previous = this.#previous
    if (!current || !previous || current === previous) return current?.position
    if (interpolation === 0) return previous.position
    if (interpolation === 1) return current.position
    return Object.freeze([
      previous.position[0] + (current.position[0] - previous.position[0]) * interpolation,
      previous.position[1] + (current.position[1] - previous.position[1]) * interpolation,
      previous.position[2] + (current.position[2] - previous.position[2]) * interpolation,
    ])
  }

  suspend(): void {
    this.#previous = this.#current
  }

  clear(): void {
    this.#previous = undefined
    this.#current = undefined
  }
}

type SourceEye = Readonly<{
  position: EyePosition
  yawDegrees: number
  pitchDegrees: number
}>

type ViewmodelTransform = Readonly<{
  position: EyePosition
  angles: EyePosition
}>

function sourceBasis(eye: SourceEye): readonly [EyePosition, EyePosition, EyePosition] {
  const pitch = eye.pitchDegrees * Math.PI / 180
  const yaw = eye.yawDegrees * Math.PI / 180
  const sinePitch = Math.sin(pitch)
  const cosinePitch = Math.cos(pitch)
  const sineYaw = Math.sin(yaw)
  const cosineYaw = Math.cos(yaw)
  return [
    [cosinePitch * cosineYaw, cosinePitch * sineYaw, -sinePitch],
    [sineYaw, -cosineYaw, 0],
    [sinePitch * cosineYaw, sinePitch * sineYaw, cosinePitch],
  ]
}

export function composeViewmodelTransform(
  model: ViewmodelTransform,
  eye: SourceEye,
): ViewmodelTransform {
  if (
    !model.position.every(Number.isFinite)
    || !model.angles.every(Number.isFinite)
    || !eye.position.every(Number.isFinite)
    || !Number.isFinite(eye.yawDegrees)
    || !Number.isFinite(eye.pitchDegrees)
  ) throw new TypeError("Viewmodel display transform is invalid")

  const basis = sourceBasis(eye)
  return Object.freeze({
    position: Object.freeze([
      eye.position[0] + model.position[0] * basis[0][0] + model.position[1] * basis[1][0] + model.position[2] * basis[2][0],
      eye.position[1] + model.position[0] * basis[0][1] + model.position[1] * basis[1][1] + model.position[2] * basis[2][1],
      eye.position[2] + model.position[0] * basis[0][2] + model.position[1] * basis[1][2] + model.position[2] * basis[2][2],
    ]),
    angles: Object.freeze([
      eye.pitchDegrees + model.angles[0],
      eye.yawDegrees + model.angles[1],
      model.angles[2],
    ]),
  })
}
