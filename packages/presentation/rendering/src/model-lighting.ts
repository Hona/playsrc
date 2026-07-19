// Behavior in this file is adapted from Valve Source SDK 2013 lighting and eye shaders;
// the Source 1 SDK License applies.

export type SourceVector3 = readonly [number, number, number]
export type SourceVector4 = readonly [number, number, number, number]

export type ModelLocalLight = Readonly<{
  kind: "point" | "directional" | "spot"
  color: SourceVector3
  position: SourceVector3
  direction: SourceVector3
  range: number
  falloff: number
  attenuation: SourceVector3
  theta: number
  phi: number
}>

export type ModelLightingInput = Readonly<{
  lightingOrigin: SourceVector3
  ambientCube: readonly [SourceVector3, SourceVector3, SourceVector3, SourceVector3, SourceVector3, SourceVector3]
  localLights: readonly ModelLocalLight[]
  cameraPosition: SourceVector3
  localEnvironment: string | null
  ambientLight: boolean
  staticLightVertex: boolean
  staticLightTexel: boolean
}>

export type ModelEyeState = Readonly<{
  primitive: number
  mesh: number
  eyeball: number
  texture: number
  worldOrigin: SourceVector3
  authoredUp: SourceVector3
  irisU: SourceVector4
  irisV: SourceVector4
  glintU: SourceVector4
  glintV: SourceVector4
}>

export type ModelDrawRequirement =
  | "ambient-cube"
  | "local-lights"
  | "camera-position"
  | "studio-eye-parameters"
  | "local-environment"
  | "current-framebuffer"
  | "authored-texture-planes"
  | "game-proxy-values"

export type PreparedModelDrawInputs = Readonly<{
  primitive: number
  lighting: ModelLightingInput | null
  eye: ModelEyeState | null
}>

export class ModelLightingError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "ModelLightingError"
  }
}

function finiteVector(value: readonly number[], length: number): boolean {
  return value.length === length && value.every(Number.isFinite)
}

function validEnvironment(identity: string | null): boolean {
  if (identity === null) return true
  return identity.startsWith("materials/")
    && identity === identity.toLowerCase()
    && !identity.includes("\\")
    && identity.split("/").every((part) => part.length > 0 && part !== "." && part !== "..")
}

export function validateModelLightingInput(input: ModelLightingInput): ModelLightingInput {
  if (
    !finiteVector(input.lightingOrigin, 3)
    || !finiteVector(input.cameraPosition, 3)
    || input.ambientCube.length !== 6
    || input.ambientCube.some((side) => !finiteVector(side, 3))
    || input.localLights.length > 4
    || !validEnvironment(input.localEnvironment)
    || typeof input.ambientLight !== "boolean"
    || typeof input.staticLightVertex !== "boolean"
    || typeof input.staticLightTexel !== "boolean"
  ) {
    throw new ModelLightingError("model lighting input is invalid")
  }
  for (const light of input.localLights) {
    if (
      !["point", "directional", "spot"].includes(light.kind)
      || !finiteVector(light.color, 3)
      || !finiteVector(light.position, 3)
      || !finiteVector(light.direction, 3)
      || !finiteVector(light.attenuation, 3)
      || ![light.range, light.falloff, light.theta, light.phi].every(Number.isFinite)
    ) {
      throw new ModelLightingError("model local-light input is invalid")
    }
  }
  return input
}

export function validateModelEyeStates(states: readonly ModelEyeState[]): readonly ModelEyeState[] {
  const primitives = new Set<number>()
  for (const state of states) {
    if (
      ![state.primitive, state.mesh, state.eyeball, state.texture].every((value) => Number.isSafeInteger(value) && value >= 0)
      || primitives.has(state.primitive)
      || !finiteVector(state.worldOrigin, 3)
      || !finiteVector(state.authoredUp, 3)
      || !finiteVector(state.irisU, 4)
      || !finiteVector(state.irisV, 4)
      || !finiteVector(state.glintU, 4)
      || !finiteVector(state.glintV, 4)
    ) {
      throw new ModelLightingError("model eye state is invalid")
    }
    primitives.add(state.primitive)
  }
  return states
}

export function prepareModelDrawInputs(request: Readonly<{
  primitive: number
  required: readonly ModelDrawRequirement[]
  lighting?: ModelLightingInput
  eyes?: readonly ModelEyeState[]
  currentFramebuffer: boolean
  authoredTexturePlanes: boolean
  gameProxyValues: boolean
}>): PreparedModelDrawInputs {
  if (!Number.isSafeInteger(request.primitive) || request.primitive < 0 || new Set(request.required).size !== request.required.length) {
    throw new ModelLightingError("model draw requirement input is invalid")
  }
  if (request.lighting) validateModelLightingInput(request.lighting)
  const eyes = validateModelEyeStates(request.eyes ?? [])
  const eye = eyes.find((state) => state.primitive === request.primitive) ?? null
  const missing: ModelDrawRequirement[] = []
  for (const requirement of request.required) {
    const available = requirement === "ambient-cube" || requirement === "local-lights" || requirement === "camera-position"
      ? request.lighting !== undefined
      : requirement === "studio-eye-parameters" ? eye !== null
      : requirement === "local-environment" ? request.lighting?.localEnvironment !== null && request.lighting?.localEnvironment !== undefined
      : requirement === "current-framebuffer" ? request.currentFramebuffer
      : requirement === "authored-texture-planes" ? request.authoredTexturePlanes
      : requirement === "game-proxy-values" ? request.gameProxyValues
      : false
    if (!available) missing.push(requirement)
  }
  if (missing.length > 0) throw new ModelLightingError(`model draw inputs are missing: ${missing.join(",")}`)
  return Object.freeze({ primitive: request.primitive, lighting: request.lighting ?? null, eye })
}

function dot(left: SourceVector3, right: SourceVector3): number {
  return left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

function subtract(left: SourceVector3, right: SourceVector3): SourceVector3 {
  return [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

function length(value: SourceVector3): number {
  return Math.sqrt(dot(value, value))
}

function normalized(value: SourceVector3, field: string): SourceVector3 {
  const magnitude = length(value)
  if (!Number.isFinite(magnitude) || magnitude <= 0) throw new ModelLightingError(`${field} is not normalizable`)
  return [value[0] / magnitude, value[1] / magnitude, value[2] / magnitude]
}

function saturate(value: number): number {
  return Math.max(0, Math.min(1, value))
}

export function evaluateAmbientCube(normal: SourceVector3, cube: ModelLightingInput["ambientCube"]): SourceVector3 {
  if (!finiteVector(normal, 3) || cube.length !== 6 || cube.some((side) => !finiteVector(side, 3))) {
    throw new ModelLightingError("ambient-cube evaluation input is invalid")
  }
  const n = normalized(normal, "world normal")
  const sideX = n[0] < 0 ? cube[1] : cube[0]
  const sideY = n[1] < 0 ? cube[3] : cube[2]
  const sideZ = n[2] < 0 ? cube[5] : cube[4]
  const x = n[0] * n[0]
  const y = n[1] * n[1]
  const z = n[2] * n[2]
  return [
    sideX[0] * x + sideY[0] * y + sideZ[0] * z,
    sideX[1] * x + sideY[1] * y + sideZ[1] * z,
    sideX[2] * x + sideY[2] * y + sideZ[2] * z,
  ]
}

export function evaluateLocalLight(
  light: ModelLocalLight,
  worldPosition: SourceVector3,
  worldNormal: SourceVector3,
  halfLambert: boolean,
): SourceVector3 {
  validateModelLightingInput({
    lightingOrigin: worldPosition,
    ambientCube: [[0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0], [0, 0, 0]],
    localLights: [light],
    cameraPosition: worldPosition,
    localEnvironment: null,
    ambientLight: false,
    staticLightVertex: false,
    staticLightTexel: false,
  })
  const normal = normalized(worldNormal, "world normal")
  let lightDirection: SourceVector3
  let attenuation = 1
  if (light.kind === "directional") {
    const direction = normalized(light.direction, "directional-light direction")
    lightDirection = [-direction[0], -direction[1], -direction[2]]
  } else {
    const delta = subtract(light.position, worldPosition)
    const distance = length(delta)
    if (distance <= 0) throw new ModelLightingError("model local light coincides with the shaded position")
    lightDirection = [delta[0] / distance, delta[1] / distance, delta[2] / distance]
    const denominator = light.attenuation[0] + light.attenuation[1] * distance + light.attenuation[2] * distance * distance
    if (denominator <= 0 || !Number.isFinite(denominator)) {
      throw new ModelLightingError("model local-light attenuation denominator is invalid")
    }
    attenuation = 1 / denominator
    if (light.kind === "spot") {
      const direction = normalized(light.direction, "spot-light direction")
      const cosine = dot(direction, [-lightDirection[0], -lightDirection[1], -lightDirection[2]])
      const theta = Math.cos(light.theta)
      const phi = Math.cos(light.phi)
      const reciprocalSpread = theta - phi > 1e-10 ? 1 / (theta - phi) : 1
      const prePower = Math.max(0.0001, (cosine - phi) * reciprocalSpread)
      attenuation *= saturate(prePower ** light.falloff)
      if (cosine <= phi) attenuation = 0
    }
  }
  let diffuse = dot(normal, lightDirection)
  diffuse = halfLambert ? saturate(diffuse * 0.5 + 0.5) ** 2 : saturate(diffuse)
  return [
    light.color[0] * attenuation * diffuse,
    light.color[1] * attenuation * diffuse,
    light.color[2] * attenuation * diffuse,
  ]
}

export function evaluateModelLighting(
  input: ModelLightingInput,
  worldPosition: SourceVector3,
  worldNormal: SourceVector3,
  halfLambert: boolean,
): SourceVector3 {
  validateModelLightingInput(input)
  if (!finiteVector(worldPosition, 3) || !finiteVector(worldNormal, 3)) {
    throw new ModelLightingError("model lighting evaluation point is invalid")
  }
  const output: [number, number, number] = input.ambientLight
    ? [...evaluateAmbientCube(worldNormal, input.ambientCube)]
    : [0, 0, 0]
  for (const light of input.localLights) {
    const value = evaluateLocalLight(light, worldPosition, worldNormal, halfLambert)
    output[0] += value[0]
    output[1] += value[1]
    output[2] += value[2]
  }
  return output
}

export function projectEyeCoordinate(row: SourceVector4, worldPosition: SourceVector3): number {
  if (!finiteVector(row, 4) || !finiteVector(worldPosition, 3)) {
    throw new ModelLightingError("eye projection input is invalid")
  }
  return row[0] * worldPosition[0] + row[1] * worldPosition[1] + row[2] * worldPosition[2] + row[3]
}

export function intersectEyeSphere(
  cameraPosition: SourceVector3,
  ray: SourceVector3,
  sphereCenter: SourceVector3,
  sphereRadius: number,
): number {
  if (![...cameraPosition, ...ray, ...sphereCenter, sphereRadius].every(Number.isFinite) || sphereRadius <= 0) {
    throw new ModelLightingError("eye sphere input is invalid")
  }
  const direction = normalized(ray, "eye ray")
  const offset = subtract(cameraPosition, sphereCenter)
  const b = dot(offset, direction)
  const discriminant = b * b - (dot(offset, offset) - sphereRadius * sphereRadius)
  return discriminant > 0 ? -b - Math.sqrt(discriminant) : 0
}

export function dilateIrisUv(uv: readonly [number, number], dilation: number): readonly [number, number] {
  if (!finiteVector(uv, 2) || !Number.isFinite(dilation)) throw new ModelLightingError("iris dilation input is invalid")
  const centered: [number, number] = [uv[0] - 0.5, uv[1] - 0.5]
  const radius = saturate(Math.hypot(centered[0], centered[1]) / 0.2)
  const amount = saturate(dilation) * 2.5 - 1.25
  const scale = 1 + (radius - 1) * amount
  return [centered[0] * scale + 0.5, centered[1] * scale + 0.5]
}

export function ambientCubeLuminance(cube: ModelLightingInput["ambientCube"]): number {
  if (cube.length !== 6 || cube.some((side) => !finiteVector(side, 3))) {
    throw new ModelLightingError("ambient cube is invalid")
  }
  let luminance = 0
  for (const side of cube) luminance += side[0] * 0.3 + side[1] * 0.59 + side[2] * 0.11
  return saturate(luminance / 6)
}
