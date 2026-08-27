// Scope paint and charge proxy behavior follow Valve Source SDK 2013;
// the Source 1 SDK License applies.
import { tf2AuthoredScope } from "../ui-resources/scope"
import type { HudMaterial, HudMaterialDraw, HudMaterialFrame, HudTexture } from "@playsrc/rendering"

export type Tf2ScopeGeometry = Readonly<{
  left: number; top: number; right: number; bottom: number; middleX: number; middleY: number
}>

export function tf2ScopeGeometry(width: number, height: number): Tf2ScopeGeometry {
  if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height) || width <= 0 || height <= 0) {
    throw new Error("TF2 Sniper scope viewport is invalid")
  }
  const middleX = Math.trunc(width / 2), middleY = Math.trunc(height / 2)
  const scopeWidth = width > height ? Math.trunc(height * 4 / 3) : width
  const scopeHeight = width > height ? height : Math.trunc(width * 3 / 4)
  return Object.freeze({
    left: middleX - Math.trunc(scopeWidth / 2), top: middleY - Math.trunc(scopeHeight / 2),
    right: middleX + Math.trunc(scopeWidth / 2), bottom: middleY + Math.trunc(scopeHeight / 2), middleX, middleY,
  })
}

const texture = (value: typeof tf2AuthoredScope.tint): HudTexture => Object.freeze({ width: value.width, height: value.height, clampS: value.clampS, clampT: value.clampT, noLod: value.noLod, mips: value.mips.map(mip => mip.pngDataUrl) })
const materials: readonly HudMaterial[] = Object.freeze([
  { kind: "refract", normal: texture(tf2AuthoredScope.normal), tint: texture(tf2AuthoredScope.tint), amount: .1, blur: 1 },
  { kind: "solid", color: [0, 0, 0, 1] },
  { kind: "two-texture-additive", base: texture(tf2AuthoredScope.chargeBase), second: texture(tf2AuthoredScope.chargeMask) },
])

export class Tf2HudScopePresentation {
  readonly #element: HTMLElement
  readonly #charge: HTMLElement
  readonly #quadrants: HTMLElement[] = []
  #viewport = ""
  #chargeValue = -1
  #draws: readonly HudMaterialDraw[] = []
  #frame?: HudMaterialFrame
  #chargeBounds: readonly [number, number, number, number] = [0, 0, 0, 0]

  constructor(root: HTMLElement) {
    const document = root.ownerDocument
    const element = document.createElement("div")
    element.dataset.tf2Scope = "authored"
    Object.assign(element.style, { position: "absolute", inset: "0", pointerEvents: "none", display: "none", zIndex: "-1", overflow: "hidden" })
    // Semantic HUD diagnostics only. Authored material pixels are painted in
    // the renderer, below the remaining HUD, against the actual framebuffer.
    for (let index = 0; index < 4; index++) {
      const quadrant = document.createElement("div")
      quadrant.dataset.scopeQuadrant = (["ul", "ur", "lr", "ll"] as const)[index]
      quadrant.dataset.sourceMaterial = tf2AuthoredScope.quadrants[index]!.logicalPath
      quadrant.dataset.sourceMaterialSha256 = tf2AuthoredScope.quadrants[index]!.sha256
      quadrant.style.position = "absolute"
      element.append(quadrant)
      this.#quadrants.push(quadrant)
    }
    const charge = document.createElement("div")
    charge.dataset.tf2ScopeCharge = "authored"
    charge.dataset.sourceMaterial = tf2AuthoredScope.chargeMaterial.logicalPath
    charge.style.position = "absolute"
    element.append(charge)
    root.prepend(element)
    this.#element = element
    this.#charge = charge
  }

  publish(visible: boolean, chargedDamage: number, viewport: Readonly<{ width: number; height: number }>): void {
    if (!visible) { this.hide(); return }
    if (!Number.isFinite(chargedDamage) || chargedDamage < 0 || chargedDamage > 150) throw new Error("TF2 Sniper scope charge is invalid")
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
      const blocks = [
        [0, 0, g.left, viewport.height], [g.right, 0, viewport.width - g.right, viewport.height],
        [0, 0, viewport.width, g.top], [0, g.bottom, viewport.width, viewport.height - g.bottom],
      ] as const
      const uv1 = .5 / 256, uv2 = 1 - uv1
      const uv = [[uv1, uv1, uv2, uv2], [uv2, uv1, uv1, uv2], [uv2, uv2, uv1, uv1], [uv1, uv2, uv2, uv1]] as const
      this.#draws = Object.freeze([
        ...bounds.map((bounds, index): HudMaterialDraw => ({ material: 0, bounds, uv: uv[index]! })),
        ...blocks.map(([x, y, w, h]): HudMaterialDraw => ({ material: 1, bounds: [x, y, Math.max(0, w), Math.max(0, h)], uv: [0, 0, 1, 1] })),
      ])
      for (const [index, [x, y, width, height]] of bounds.entries()) {
        Object.assign(this.#quadrants[index]!.style, { left: `${x}px`, top: `${y}px`, width: `${width}px`, height: `${height}px` })
      }
      const scale = viewport.height / 480
      const chargeBounds = [Math.trunc(viewport.width / 2 + 64 * scale), Math.trunc(viewport.height / 2 - 64 * scale), Math.trunc(64 * scale), Math.trunc(128 * scale)] as const
      this.#chargeBounds = chargeBounds
      Object.assign(this.#charge.style, { left: `${chargeBounds[0]}px`, top: `${chargeBounds[1]}px`, width: `${chargeBounds[2]}px`, height: `${chargeBounds[3]}px` })
      this.#chargeValue = -1
    }
    if (this.#chargeValue !== chargedDamage || !this.#frame) {
      this.#chargeValue = chargedDamage
      const charge = chargedDamage / 150
      const translation = (1 - charge) * .8 + .6
      // SniperRifleCharge transforms texture coordinates, not image geometry:
      // T(charge) * T(.5) * S(1,.25) * T(-.5).
      this.#frame = Object.freeze({ materials, draws: Object.freeze([...this.#draws, {
        material: 2, bounds: this.#chargeBounds,
        uv: [0, .375 + translation, 1, .625 + translation] as const, secondUv: [0, 0, 1, 1] as const,
      }]) })
      this.#charge.dataset.charge = String(charge)
    }
    this.#element.style.display = "block"
  }

  materialFrame(): HudMaterialFrame | undefined { return this.#frame }
  setViewport(viewport: Readonly<{ width: number; height: number }>): void {
    if (this.#frame) this.publish(true, this.#chargeValue, viewport)
  }
  hide(): void { this.#element.style.display = "none"; this.#frame = undefined }
  destroy(): void { this.hide(); this.#element.remove() }
}
