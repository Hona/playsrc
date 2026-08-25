import {
  resolveTf2CrosshairGeometry,
  type Tf2HudBinding,
} from "../hud"
import type { Tf2AuthoredCrosshair } from "../ui-resources/crosshair"

export function paintTf2AuthoredCrosshair(
  element: HTMLElement,
  asset: Tf2AuthoredCrosshair,
  color: readonly [number, number, number, number],
  frameIndex = 0,
): void {
  const frame = asset.frames[frameIndex]
  if (!frame) throw new Error(`TF2 authored crosshair ${asset.file || "stock"} frame ${frameIndex} is unavailable`)
  const width = asset.crop?.width ?? asset.textureWidth
  const height = asset.crop?.height ?? asset.textureHeight
  const matrix = [
    color[0] / 255, 0, 0, 0, 0,
    0, color[1] / 255, 0, 0, 0,
    0, 0, color[2] / 255, 0, 0,
    0, 0, 0, color[3] / 255, 0,
  ].join(" ")
  const image = `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}"><defs><filter id="t" color-interpolation-filters="sRGB"><feColorMatrix type="matrix" values="${matrix}"/></filter></defs><image width="${width}" height="${height}" href="${frame.pngDataUrl}" filter="url(#t)"/></svg>`
  element.style.backgroundImage = `url("data:image/svg+xml,${encodeURIComponent(image)}")`
  element.style.backgroundRepeat = "no-repeat"
  element.style.backgroundSize = "100% 100%"
  element.dataset.sourceMaterial = asset.material.logicalPath
  element.dataset.sourceMaterialSha256 = asset.material.sha256
  element.dataset.sourceTexture = asset.texture.logicalPath
  element.dataset.sourceTextureSha256 = asset.texture.sha256
  element.dataset.sourceFrame = String(frameIndex)
  element.dataset.sourceFrameSha256 = frame.pngSha256
  element.dataset.crosshairColor = color.join(" ")
}

export class Tf2HudCrosshairPresentation {
  readonly #element: HTMLElement
  #fingerprint = ""

  constructor(root: HTMLElement) {
    const element = root.ownerDocument.createElement("div")
    element.dataset.tf2Crosshair = "authored"
    element.style.position = "absolute"
    element.style.pointerEvents = "none"
    element.style.display = "none"
    root.append(element)
    this.#element = element
  }

  publish(binding: Tf2HudBinding, viewport: Readonly<{ width: number; height: number }>): void {
    const geometry = resolveTf2CrosshairGeometry(binding, viewport)
    if (!geometry || geometry.width <= 0 || geometry.height <= 0) {
      this.#element.style.display = "none"
      this.#fingerprint = ""
      return
    }
    const fingerprint = [
      geometry.kind,
      geometry.asset.material.sha256,
      geometry.left,
      geometry.top,
      geometry.width,
      geometry.height,
      ...geometry.color,
    ].join(":")
    if (fingerprint === this.#fingerprint) return
    this.#fingerprint = fingerprint
    this.#element.style.left = `${geometry.left}px`
    this.#element.style.top = `${geometry.top}px`
    this.#element.style.width = `${geometry.width}px`
    this.#element.style.height = `${geometry.height}px`
    this.#element.dataset.crosshairStyle = geometry.kind === "stock" ? "stock" : geometry.asset.file
    paintTf2AuthoredCrosshair(this.#element, geometry.asset, geometry.color)
    this.#element.style.display = "block"
  }

  hide(): void {
    this.#element.style.display = "none"
    this.#fingerprint = ""
  }

  destroy(): void {
    this.#element.remove()
    this.#fingerprint = ""
  }
}
