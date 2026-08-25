import { tf2AuthoredScope } from "../ui-resources/scope"

export type Tf2ScopeGeometry = Readonly<{
  left: number
  top: number
  right: number
  bottom: number
  middleX: number
  middleY: number
}>

export function tf2ScopeGeometry(width: number, height: number): Tf2ScopeGeometry {
  if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height) || width <= 0 || height <= 0) {
    throw new Error("TF2 Sniper scope viewport is invalid")
  }
  const middleX = Math.trunc(width / 2)
  const middleY = Math.trunc(height / 2)
  const scopeWidth = width > height ? Math.trunc(height * 4 / 3) : width
  const scopeHeight = width > height ? height : Math.trunc(width * 3 / 4)
  return Object.freeze({
    left: middleX - Math.trunc(scopeWidth / 2),
    top: middleY - Math.trunc(scopeHeight / 2),
    right: middleX + Math.trunc(scopeWidth / 2),
    bottom: middleY + Math.trunc(scopeHeight / 2),
    middleX,
    middleY,
  })
}

let nextFilter = 0

export class Tf2HudScopePresentation {
  readonly #element: HTMLElement
  readonly #charge: HTMLElement
  readonly #chargeBase: HTMLImageElement
  readonly #quadrants: HTMLElement[] = []
  readonly #blocks: HTMLElement[] = []
  #viewport = ""
  #chargeValue = -1

  constructor(root: HTMLElement) {
    const document = root.ownerDocument
    const element = document.createElement("div")
    element.dataset.tf2Scope = "authored"
    Object.assign(element.style, { position: "absolute", inset: "0", pointerEvents: "none", display: "none", zIndex: "-1", overflow: "hidden" })
    const filterIdentity = `playsrc-sniper-refract-${++nextFilter}`
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg")
    svg.setAttribute("width", "0")
    svg.setAttribute("height", "0")
    svg.style.position = "absolute"
    const filter = document.createElementNS("http://www.w3.org/2000/svg", "filter")
    filter.setAttribute("id", filterIdentity)
    filter.setAttribute("x", "0")
    filter.setAttribute("y", "0")
    filter.setAttribute("width", "1")
    filter.setAttribute("height", "1")
    filter.setAttribute("color-interpolation-filters", "sRGB")
    const normal = document.createElementNS("http://www.w3.org/2000/svg", "feImage")
    normal.setAttribute("href", tf2AuthoredScope.normal.frame.pngDataUrl)
    normal.setAttribute("result", "normal")
    normal.setAttribute("preserveAspectRatio", "none")
    const displacement = document.createElementNS("http://www.w3.org/2000/svg", "feDisplacementMap")
    displacement.setAttribute("in", "SourceGraphic")
    displacement.setAttribute("in2", "normal")
    displacement.setAttribute("scale", "0.1")
    displacement.setAttribute("xChannelSelector", "R")
    displacement.setAttribute("yChannelSelector", "G")
    filter.append(normal, displacement)
    svg.append(filter)
    element.append(svg)
    for (let index = 0; index < 4; index += 1) {
      const quadrant = document.createElement("div")
      quadrant.dataset.scopeQuadrant = (["ul", "ur", "lr", "ll"] as const)[index]
      quadrant.dataset.sourceMaterial = tf2AuthoredScope.quadrants[index]!.logicalPath
      quadrant.dataset.sourceMaterialSha256 = tf2AuthoredScope.quadrants[index]!.sha256
      quadrant.dataset.sourceTexture = tf2AuthoredScope.tint.source.logicalPath
      quadrant.dataset.sourceNormal = tf2AuthoredScope.normal.source.logicalPath
      Object.assign(quadrant.style, {
        position: "absolute",
        backgroundImage: `url("${tf2AuthoredScope.tint.frame.pngDataUrl}")`,
        backgroundSize: "100% 100%",
        backgroundRepeat: "no-repeat",
        backdropFilter: `url(#${filterIdentity})`,
        transform: index === 1 ? "scaleX(-1)" : index === 2 ? "scale(-1,-1)" : index === 3 ? "scaleY(-1)" : "none",
      })
      element.append(quadrant)
      this.#quadrants.push(quadrant)
    }
    for (let index = 0; index < 4; index += 1) {
      const block = document.createElement("div")
      Object.assign(block.style, { position: "absolute", backgroundColor: "rgb(0, 0, 0)" })
      element.append(block)
      this.#blocks.push(block)
    }
    const charge = document.createElement("div")
    charge.dataset.tf2ScopeCharge = "authored"
    charge.dataset.sourceMaterial = tf2AuthoredScope.chargeMaterial.logicalPath
    charge.dataset.sourceMaterialSha256 = tf2AuthoredScope.chargeMaterial.sha256
    Object.assign(charge.style, { position: "absolute", overflow: "hidden", mixBlendMode: "screen" })
    const mask = document.createElement("img")
    mask.src = tf2AuthoredScope.chargeMask.frame.pngDataUrl
    Object.assign(mask.style, { position: "absolute", inset: "0", width: "100%", height: "100%" })
    const base = document.createElement("img")
    base.src = tf2AuthoredScope.chargeBase.frame.pngDataUrl
    Object.assign(base.style, { position: "absolute", inset: "0", width: "100%", height: "100%", transformOrigin: "50% 50%", mixBlendMode: "multiply" })
    charge.append(mask, base)
    element.append(charge)
    root.prepend(element)
    this.#element = element
    this.#charge = charge
    this.#chargeBase = base
  }

  publish(visible: boolean, chargedDamage: number, viewport: Readonly<{ width: number; height: number }>): void {
    if (!visible) {
      this.#element.style.display = "none"
      return
    }
    if (!Number.isFinite(chargedDamage) || chargedDamage < 0 || chargedDamage > 150) {
      throw new Error("TF2 Sniper scope charge is invalid")
    }
    const key = `${viewport.width}:${viewport.height}`
    if (this.#viewport !== key) {
      this.#viewport = key
      const g = tf2ScopeGeometry(viewport.width, viewport.height)
      const bounds = [
        [g.left, g.top, g.middleX - g.left, g.middleY - g.top],
        [g.middleX - 1, g.top, g.right - g.middleX + 1, g.middleY - g.top + 1],
        [g.middleX, g.middleY, g.right - g.middleX, g.bottom - g.middleY],
        [g.left, g.middleY, g.middleX - g.left, g.bottom - g.middleY],
      ] as const
      for (const [index, item] of bounds.entries()) {
        Object.assign(this.#quadrants[index]!.style, { left: `${item[0]}px`, top: `${item[1]}px`, width: `${item[2]}px`, height: `${item[3]}px` })
      }
      const blocks = [
        [0, 0, g.left, viewport.height],
        [g.right, 0, viewport.width - g.right, viewport.height],
        [0, 0, viewport.width, g.top],
        [0, g.bottom, viewport.width, viewport.height - g.bottom],
      ] as const
      for (const [index, item] of blocks.entries()) {
        Object.assign(this.#blocks[index]!.style, { left: `${item[0]}px`, top: `${item[1]}px`, width: `${Math.max(0, item[2])}px`, height: `${Math.max(0, item[3])}px` })
      }
      const scale = viewport.height / 480
      Object.assign(this.#charge.style, {
        left: `${Math.trunc(viewport.width / 2 + 64 * scale)}px`,
        top: `${Math.trunc(viewport.height / 2 - 64 * scale)}px`,
        width: `${Math.trunc(64 * scale)}px`,
        height: `${Math.trunc(128 * scale)}px`,
      })
    }
    if (this.#chargeValue !== chargedDamage) {
      this.#chargeValue = chargedDamage
      const charge = chargedDamage / 150
      const translation = (1 - charge) * 0.8 + 0.6
      this.#chargeBase.style.transform = `translateY(${translation * 100}%) scaleY(0.25)`
      this.#charge.dataset.charge = String(charge)
    }
    this.#element.style.display = "block"
  }

  hide(): void { this.#element.style.display = "none" }
  destroy(): void { this.#element.remove() }
}
