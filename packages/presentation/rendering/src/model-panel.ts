import { sourceHorizontal4By3FovToVertical } from "./source-camera"

export type SourceModelPanelViewport = Readonly<{
  x: number
  y: number
  width: number
  height: number
}>

export type SourceModelPanelPresentation = Readonly<{
  viewport: SourceModelPanelViewport
  verticalFovDegrees: number
  origin: readonly [number, number, number]
}>

export function sourceModelPanelPresentation(request: Readonly<{
  model: string
  horizontalFov4By3: number
  origin: readonly [number, number, number]
  bounds: SourceModelPanelViewport
  displayWidth: number
  displayHeight: number
  devicePixelRatio: number
}>): SourceModelPanelPresentation {
  if (!request.model
    || !Number.isFinite(request.horizontalFov4By3)
    || request.horizontalFov4By3 <= 0
    || request.horizontalFov4By3 >= 180
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
  const offset = request.model.toLowerCase().startsWith("models/player/")
    ? ratio > 1.05 ? -60 : ratio < 0.95 ? 15 : 0
    : 0
  const x = Math.max(0, Math.round(request.bounds.x * request.devicePixelRatio))
  const y = Math.max(0, request.displayHeight - Math.round((request.bounds.y + request.bounds.height) * request.devicePixelRatio))
  const width = Math.min(Math.max(1, Math.round(request.bounds.width * request.devicePixelRatio)), request.displayWidth - x)
  const height = Math.min(Math.max(1, Math.round(request.bounds.height * request.devicePixelRatio)), request.displayHeight - y)
  if (width <= 0 || height <= 0) throw new TypeError("Source model-panel viewport is outside the display")
  return Object.freeze({
    viewport: Object.freeze({ x, y, width, height }),
    verticalFovDegrees: sourceHorizontal4By3FovToVertical(request.horizontalFov4By3),
    origin: Object.freeze([request.origin[0] + offset, request.origin[1], request.origin[2]]) as readonly [number, number, number],
  })
}
