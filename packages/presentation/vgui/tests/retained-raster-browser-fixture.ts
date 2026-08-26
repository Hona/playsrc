import { shadeVguiImage, VguiImageRasterizer, type VguiImageRasterRequest } from "../src/image-renderer"
import type { VguiImageMaterialPresentation } from "../src/runtime-contract"

const baseline = document.querySelector<HTMLCanvasElement>("#baseline")!
const retained = document.querySelector<HTMLCanvasElement>("#retained")!
const texture = document.createElement("canvas")
texture.width = 256
texture.height = 128
const context = texture.getContext("2d", { willReadFrequently: true })!
const bytes = new Uint8ClampedArray(texture.width * texture.height * 4)
for (let y = 0; y < texture.height; y += 1) {
  for (let x = 0; x < texture.width; x += 1) bytes.set([x, y * 2, 255 - x, x], (y * texture.width + x) * 4)
}
context.putImageData(new ImageData(bytes, texture.width, texture.height), 0, 0)
const material: VguiImageMaterialPresentation = {
  shader: "unlit-generic",
  base: { logicalIdentity: "raster-parity", revision: "1", browserUrl: texture.toDataURL(), width: texture.width, height: texture.height, hardwareFiltered: true, colorRead: "srgb" },
  second: null, detail: null, detailScale: [1, 1], detailBlendMode: 0, detailBlendFactor: 1, detailTint: [1, 1, 1],
  distanceAlpha: false, distanceAlphaFromDetail: false, softEdges: false, scaleSoftEdges: false,
  edgeSoftnessStart: 0.6, edgeSoftnessEnd: 0.5, outline: false, outlineColor: [1, 1, 1], outlineAlpha: 0,
  outlineStart0: 0, outlineStart1: 0, outlineEnd0: 0, outlineEnd1: 0, scaleOutline: false,
  glow: false, glowColor: [1, 1, 1], glowAlpha: 1, glowStart: 0.7, glowEnd: 0.5, glowX: 0, glowY: 0,
}
const request: VguiImageRasterRequest = {
  width: 256, height: 128, viewportWidth: innerWidth, viewportHeight: innerHeight,
  tint: [210, 160, 255, 200], geometry: { kind: "stretch", rotation: 0 }, material,
}
const original = shadeVguiImage(request, new Map([[material.base.logicalIdentity, {
  width: texture.width, height: texture.height, filtered: true, colorRead: "srgb",
  rgba: context.getImageData(0, 0, texture.width, texture.height).data,
}]]))
baseline.width = request.width
baseline.height = request.height
baseline.getContext("2d")!.putImageData(new ImageData(original, request.width, request.height), 0, 0)
const rasterizer = new VguiImageRasterizer(document)
await rasterizer.render(retained, request)
let writes = 0
const retainedContext = retained.getContext("2d")!
const putImageData = retainedContext.putImageData.bind(retainedContext)
retainedContext.putImageData = (...parameters: Parameters<CanvasRenderingContext2D["putImageData"]>) => { writes += 1; putImageData(...parameters) }
const observer = new MutationObserver(records => { document.body.dataset.rasterMutations = String(Number(document.body.dataset.rasterMutations ?? 0) + records.length) })
observer.observe(retained, { attributes: true })
for (let index = 0; index < 20; index += 1) await rasterizer.render(retained, request)
// A superseded asynchronous paint must not overwrite a newer request for the
// pixels already on screen, nor reset the canvas while it is pending.
await Promise.all([
  rasterizer.render(retained, { ...request, tint: [180, 200, 90, 100] }),
  rasterizer.render(retained, request),
])
document.body.dataset.unchangedRasterWrites = String(writes)
await rasterizer.render(retained, { ...request, tint: [180, 200, 90, 100] })
await rasterizer.render(retained, request)
document.body.dataset.changedRasterWrites = String(writes)
observer.disconnect()
document.body.dataset.ready = "true"
document.querySelector("button")!.addEventListener("click", () => {
  document.body.dataset.activations = String(Number(document.body.dataset.activations ?? 0) + 1)
})
addEventListener("pagehide", () => rasterizer.destroy(), { once: true })
