import { sourceHorizontal4By3FovToVertical } from "./source-camera"

export type SourceModelPanelViewport = Readonly<{
  x: number
  y: number
  width: number
  height: number
}>

export type SourceModelPanelPresentation = Readonly<{
  viewport: SourceModelPanelViewport
  rendererViewport: SourceModelPanelViewport
  verticalFovDegrees: number
  near: number
  far: number
  projection: Readonly<{ width: number; height: number; offsetX: number; offsetY: number }>
  origin: readonly [number, number, number]
}>

type TargetRectangle = { x: number; y: number; z: number; w: number; set(x: number, y: number, width: number, height: number): unknown }

// Three's render-target viewport is physical and takes precedence over the
// canvas viewport. HUD passes share the frame target in LDR; HDR readback can
// already have returned to the canvas. Keep both routes identical on screen.
export function withSourceModelPanelTargetViewport<T>(
  target: { viewport: TargetRectangle; scissor: TargetRectangle } | null,
  presentation: SourceModelPanelPresentation,
  draw: () => T,
): T {
  if (!target) return draw()
  const previousViewport = [target.viewport.x, target.viewport.y, target.viewport.z, target.viewport.w] as const
  const previousScissor = [target.scissor.x, target.scissor.y, target.scissor.z, target.scissor.w] as const
  const { x, y, width, height } = presentation.viewport
  target.viewport.set(x, y, width, height)
  target.scissor.set(x, y, width, height)
  try { return draw() }
  finally {
    target.viewport.set(...previousViewport)
    target.scissor.set(...previousScissor)
  }
}

export function sourceModelPanelPresentation(request: Readonly<{
  model: string
  kind: "entity" | "studio"
  fov: number
  origin: readonly [number, number, number]
  bounds: SourceModelPanelViewport
  displayWidth: number
  displayHeight: number
  devicePixelRatio: number
}>): SourceModelPanelPresentation {
  if (!request.model
    || (request.kind !== "entity" && request.kind !== "studio")
    || !Number.isFinite(request.fov)
    || request.fov <= 0
    || request.fov >= 180
    || !request.origin.every(Number.isFinite)
    || !Number.isFinite(request.devicePixelRatio)
    || request.devicePixelRatio <= 0
    || !Number.isSafeInteger(request.displayWidth)
    || !Number.isSafeInteger(request.displayHeight)
    || request.displayWidth < 1
    || request.displayHeight < 1
    || !Object.values(request.bounds).every(Number.isFinite)
    || request.bounds.width <= 0
    || request.bounds.height <= 0) {
    throw new TypeError("Source model-panel presentation is invalid")
  }
  const ratio = request.bounds.width / request.bounds.height / (4 / 3)
  const offset = request.kind === "entity" && request.model.toLowerCase().startsWith("models/player/")
    ? ratio > 1.05 ? -60 : ratio < 0.95 ? 15 : 0
    : 0
  const left = Math.round(request.bounds.x * request.devicePixelRatio)
  const top = Math.round(request.bounds.y * request.devicePixelRatio)
  const right = Math.round((request.bounds.x + request.bounds.width) * request.devicePixelRatio)
  const bottom = Math.round((request.bounds.y + request.bounds.height) * request.devicePixelRatio)
  const x = Math.max(0, left)
  const y = Math.max(0, top)
  const width = Math.min(request.displayWidth, right) - x
  const height = Math.min(request.displayHeight, bottom) - y
  if (width <= 0 || height <= 0) throw new TypeError("Source model-panel viewport is outside the display")
  const logicalPixel = (physical: number): number => {
    const logical = physical / request.devicePixelRatio
    return Math.floor(logical * request.devicePixelRatio) < physical
      ? logical + Number.EPSILON * Math.max(1, logical)
      : logical
  }
  return Object.freeze({
    viewport: Object.freeze({ x, y, width, height }),
    rendererViewport: Object.freeze({
      x: logicalPixel(x),
      y: logicalPixel(y),
      width: logicalPixel(width),
      height: logicalPixel(height),
    }),
    verticalFovDegrees: request.kind === "entity"
      ? sourceHorizontal4By3FovToVertical(request.fov)
      : 2 * Math.atan(Math.tan(request.fov * Math.PI / 360) * request.bounds.height / request.bounds.width) * 180 / Math.PI,
    near: request.kind === "entity" ? 7 : 3,
    far: request.kind === "entity" ? 1000 : 16384 * 1.73205080757,
    projection: Object.freeze({ width: right - left, height: bottom - top, offsetX: x - left, offsetY: y - top }),
    origin: Object.freeze([request.origin[0] + offset, request.origin[1], request.origin[2]]) as readonly [number, number, number],
  })
}
