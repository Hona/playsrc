// Source shader gamma behavior is adapted from Valve Source SDK 2013;
// the Source 1 SDK License applies.

export type OutputColorSpace = "srgb"
export type CanvasFormat = "bgra8unorm" | "rgba8unorm"
export type CanvasAlphaMode = "opaque" | "premultiplied"
export type ToneOperator = "identity" | "source-pc-integer"

export type ExposureConfiguration = Readonly<{
  fixedStepSeconds: number
  minimum: number
  maximum: number
  brightPixelFraction: number
  targetFraction: number
  minimumMedianLuminance: number
  adaptationRate: number
  accelerateDownRate: number
  maximumStepAlpha: number
  maximumDeltaSeconds: number
  maximumSteps: number
}>

export type RenderConfiguration = Readonly<{
  lightingProfile: "ldr" | "hdr"
  exposure: ExposureConfiguration
  toneOperator: ToneOperator
  outputColorSpace: OutputColorSpace
  canvasFormat: CanvasFormat
  alphaMode: CanvasAlphaMode
}>

export const SOURCE_EXPOSURE = Object.freeze({
  fixedStepSeconds: 0.015,
  minimum: 0.5,
  maximum: 2,
  brightPixelFraction: 0.02,
  targetFraction: 0.6,
  minimumMedianLuminance: 0.03,
  adaptationRate: 1,
  accelerateDownRate: 3,
  maximumStepAlpha: 1 / 64,
  maximumDeltaSeconds: 0.25,
  maximumSteps: 32,
} satisfies ExposureConfiguration)

export const SOURCE_PC_INTEGER_HDR = Object.freeze({
  lightingProfile: "hdr",
  exposure: SOURCE_EXPOSURE,
  toneOperator: "source-pc-integer",
  outputColorSpace: "srgb",
  canvasFormat: "bgra8unorm",
  alphaMode: "opaque",
} satisfies RenderConfiguration)

export const SOURCE_LDR = Object.freeze({
  lightingProfile: "ldr",
  exposure: Object.freeze({ ...SOURCE_EXPOSURE, minimum: 1, maximum: 1 }),
  toneOperator: "identity",
  outputColorSpace: "srgb",
  canvasFormat: "bgra8unorm",
  alphaMode: "opaque",
} satisfies RenderConfiguration)

export class ColorOutputError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "ColorOutputError"
  }
}

function finite(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value)
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(maximum, value))
}

export function validateRenderConfiguration(input: RenderConfiguration): RenderConfiguration {
  const exposure = input.exposure
  if (
    (input.lightingProfile !== "ldr" && input.lightingProfile !== "hdr")
    || input.outputColorSpace !== "srgb"
    || (input.canvasFormat !== "bgra8unorm" && input.canvasFormat !== "rgba8unorm")
    || (input.alphaMode !== "opaque" && input.alphaMode !== "premultiplied")
    || (input.lightingProfile === "ldr" ? input.toneOperator !== "identity" : input.toneOperator !== "source-pc-integer")
    || !finite(exposure.fixedStepSeconds) || exposure.fixedStepSeconds <= 0
    || !finite(exposure.minimum) || exposure.minimum <= 0
    || !finite(exposure.maximum) || exposure.maximum < exposure.minimum
    || !finite(exposure.brightPixelFraction) || exposure.brightPixelFraction <= 0 || exposure.brightPixelFraction > 1
    || !finite(exposure.targetFraction) || exposure.targetFraction <= 0 || exposure.targetFraction > 1
    || !finite(exposure.minimumMedianLuminance) || exposure.minimumMedianLuminance < 0
    || !finite(exposure.adaptationRate) || exposure.adaptationRate < 0
    || !finite(exposure.accelerateDownRate) || exposure.accelerateDownRate < 1
    || !finite(exposure.maximumStepAlpha) || exposure.maximumStepAlpha < 0 || exposure.maximumStepAlpha > 1
    || !finite(exposure.maximumDeltaSeconds) || exposure.maximumDeltaSeconds <= 0
    || !Number.isSafeInteger(exposure.maximumSteps) || exposure.maximumSteps < 1 || exposure.maximumSteps > 1_024
  ) {
    throw new ColorOutputError("render configuration is invalid")
  }
  if (input.lightingProfile === "ldr" && (exposure.minimum !== 1 || exposure.maximum !== 1)) {
    throw new ColorOutputError("LDR exposure must remain exactly one")
  }
  return Object.freeze({
    ...input,
    exposure: Object.freeze({ ...exposure }),
  })
}

const HISTOGRAM_BUCKETS = 16

export function histogramBoundary(index: number): number {
  return Math.pow(clamp(index, 0, HISTOGRAM_BUCKETS) / HISTOGRAM_BUCKETS, 1.5)
}

export function buildLuminanceHistogram(values: readonly number[] | Float32Array): Uint32Array {
  const histogram = new Uint32Array(HISTOGRAM_BUCKETS)
  for (const value of values) {
    if (!Number.isFinite(value)) continue
    const bucket = Math.min(
      HISTOGRAM_BUCKETS - 1,
      Math.floor(Math.pow(clamp(value, 0, 1), 1 / 1.5) * HISTOGRAM_BUCKETS),
    )
    histogram[bucket] += 1
  }
  return histogram
}

function histogramLocation(histogram: Uint32Array, fraction: number, sticky: number | undefined): number | undefined {
  const total = histogram.reduce((sum, value) => sum + value, 0)
  if (total === 0) return undefined
  let testedPixels = 0
  let testedRange = 0
  for (let index = HISTOGRAM_BUCKETS - 1; index >= 0; index -= 1) {
    const binFraction = histogram[index]! / total
    const required = fraction - testedPixels
    const minimum = histogramBoundary(index)
    const maximum = histogramBoundary(index + 1)
    const range = maximum - minimum
    if (binFraction >= required) {
      if (sticky !== undefined && sticky >= minimum && sticky <= maximum) return sticky
      const ratio = binFraction > 0 ? required / binFraction : 0
      return clamp(1 - (testedRange + range * ratio), minimum, maximum)
    }
    testedPixels += binFraction
    testedRange += range
  }
  return undefined
}

export function exposureTarget(
  histogram: Uint32Array,
  current: number,
  configuration: ExposureConfiguration = SOURCE_EXPOSURE,
): number {
  if (histogram.length !== HISTOGRAM_BUCKETS || !Number.isFinite(current) || current <= 0) {
    throw new ColorOutputError("exposure target input is invalid")
  }
  const bright = Math.max(0.0001, histogramLocation(
    histogram,
    configuration.brightPixelFraction,
    configuration.targetFraction,
  ) ?? configuration.targetFraction)
  let target = configuration.targetFraction / bright * current
  const median = histogramLocation(histogram, 0.5, undefined)
  if (median !== undefined && median > 0) target = Math.max(target, configuration.minimumMedianLuminance / median)
  return clamp(target, configuration.minimum, configuration.maximum)
}

export type ExposureSnapshot = Readonly<{
  current: number
  goal: number
  fixedSteps: number
  submittedHistograms: number
  droppedSeconds: number
}>

export class ExposureController {
  readonly configuration: ExposureConfiguration
  #current = 1
  #goal = 1
  #fixedSteps = 0
  #submittedHistograms = 0
  #accumulator = 0
  #droppedSeconds = 0
  #history: number[] = []

  constructor(configuration: ExposureConfiguration = SOURCE_EXPOSURE) {
    this.configuration = validateRenderConfiguration({
      ...SOURCE_PC_INTEGER_HDR,
      exposure: configuration,
    }).exposure
  }

  submit(histogram: Uint32Array): ExposureSnapshot {
    const target = exposureTarget(histogram, this.#current, this.configuration)
    this.#history.push(target)
    if (this.#history.length > 10) this.#history.shift()
    this.#goal = target
    if (this.#history.length === 10) {
      let weighted = 0
      let total = 0
      for (let index = 0; index < 10; index += 1) {
        const weight = Math.abs(index - 5) / 5
        weighted += this.#history[index]! * weight
        total += weight
      }
      this.#goal = clamp(weighted / total, this.configuration.minimum, this.configuration.maximum)
    }
    this.#submittedHistograms += 1
    return this.snapshot()
  }

  advance(deltaSeconds: number): ExposureSnapshot {
    if (!Number.isFinite(deltaSeconds) || deltaSeconds < 0) throw new ColorOutputError("exposure delta is invalid")
    const accepted = Math.min(deltaSeconds, this.configuration.maximumDeltaSeconds)
    this.#droppedSeconds += deltaSeconds - accepted
    this.#accumulator += accepted
    let steps = 0
    while (this.#accumulator + 1e-12 >= this.configuration.fixedStepSeconds && steps < this.configuration.maximumSteps) {
      this.#step()
      this.#accumulator -= this.configuration.fixedStepSeconds
      steps += 1
    }
    if (steps === this.configuration.maximumSteps && this.#accumulator >= this.configuration.fixedStepSeconds) {
      this.#droppedSeconds += this.#accumulator
      this.#accumulator = 0
    }
    return this.snapshot()
  }

  reset(value = 1): ExposureSnapshot {
    if (!Number.isFinite(value)) throw new ColorOutputError("exposure reset value is invalid")
    this.#current = clamp(value, this.configuration.minimum, this.configuration.maximum)
    this.#goal = this.#current
    this.#fixedSteps = 0
    this.#submittedHistograms = 0
    this.#accumulator = 0
    this.#droppedSeconds = 0
    this.#history = []
    return this.snapshot()
  }

  snapshot(): ExposureSnapshot {
    return Object.freeze({
      current: this.#current,
      goal: this.#goal,
      fixedSteps: this.#fixedSteps,
      submittedHistograms: this.#submittedHistograms,
      droppedSeconds: this.#droppedSeconds,
    })
  }

  #step(): void {
    let rate = this.configuration.adaptationRate * 2
    if (this.#goal < this.#current) {
      const amount = clamp((this.#current - this.#goal) / 1.5, 0, 1)
      rate += (this.configuration.accelerateDownRate * rate - rate) * amount
    }
    const alpha = clamp(Math.min(
      rate * this.configuration.fixedStepSeconds,
      this.configuration.maximumStepAlpha,
    ), 0, 1)
    this.#current += (this.#goal - this.#current) * alpha
    this.#fixedSteps += 1
  }
}

export function linearToSrgb(value: number): number {
  const linear = clamp(value, 0, 1)
  return linear <= 0.0031308 ? linear * 12.92 : 1.055 * Math.pow(linear, 1 / 2.4) - 0.055
}

export function sourceShaderGammaToLinear(value: number): number {
  if (!Number.isFinite(value)) throw new ColorOutputError("Source shader gamma input is invalid")
  const gamma = Math.fround(value)
  if (gamma > 1) return gamma
  if (gamma < 0) return 0
  if (gamma >= Math.fround(0.95)) return 1

  const scaled = Math.fround(gamma * Math.fround(255))
  const lower = Math.floor(scaled)
  const fraction = scaled - lower
  const index = fraction < 0.5 ? lower : fraction > 0.5 ? lower + 1 : lower % 2 === 0 ? lower : lower + 1
  return Math.fround(Math.pow(Math.fround(index / 255), Math.fround(2.2)))
}

export function applyColorOutput(
  linear: readonly [number, number, number, number],
  configuration: RenderConfiguration,
  exposure = 1,
): readonly [number, number, number, number] {
  const accepted = validateRenderConfiguration(configuration)
  if (!linear.every(Number.isFinite) || !Number.isFinite(exposure) || exposure < 0) {
    throw new ColorOutputError("color output input is invalid")
  }
  const scale = accepted.toneOperator === "source-pc-integer" ? exposure : 1
  const alpha = accepted.alphaMode === "opaque" ? 1 : clamp(linear[3], 0, 1)
  const rgb = linear.slice(0, 3).map((value) => linearToSrgb(value * scale))
  if (accepted.alphaMode === "premultiplied") {
    for (let channel = 0; channel < 3; channel += 1) rgb[channel] *= alpha
  }
  return Object.freeze([...rgb, alpha]) as unknown as readonly [number, number, number, number]
}
