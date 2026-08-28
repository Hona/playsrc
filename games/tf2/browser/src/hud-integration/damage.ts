import type { Tf2HudBinding } from "../hud"

type DamageIndicator = Readonly<{
  element: HTMLDivElement
  started: number
  lifetime: number
  scale: number
  direction: readonly [number, number, number]
}>

export type Tf2DamageIndicatorInput = Readonly<{
  material: string
  texture: Readonly<{ width: number; height: number; rgba: Uint8Array }>
  eyePosition(): readonly [number, number, number]
  yawDegrees(): number
  random(): number
}>

export class Tf2HudDamagePresentation {
  readonly #root: HTMLElement
  readonly #input: Tf2DamageIndicatorInput
  readonly #image: string
  readonly #indicators: DamageIndicator[] = []

  constructor(root: HTMLElement, input: Tf2DamageIndicatorInput) {
    this.#root = root
    this.#input = input
    const canvas = root.ownerDocument.createElement("canvas")
    canvas.width = input.texture.width
    canvas.height = input.texture.height
    const context = canvas.getContext("2d")
    if (!context) throw new Error("Authored TF2 damage indicator texture cannot be decoded")
    context.putImageData(new ImageData(new Uint8ClampedArray(input.texture.rgba), canvas.width, canvas.height), 0, 0)
    this.#image = canvas.toDataURL("image/png")
  }

  publish(binding: Tf2HudBinding, now: number): void {
    for (const command of binding.commands) {
      if (command.kind !== "damage-indicator") continue
      const eye = this.#input.eyePosition()
      const delta = command.direction.map((value, axis) => value - eye[axis]!)
      let length = Math.hypot(...delta)
      if (length === 0) continue
      const noisy = delta.map((value) => value / length + this.#input.random() * 0.2 - 0.1)
      length = Math.hypot(...noisy)
      if (length === 0) continue
      const element = this.#root.ownerDocument.createElement("div")
      element.dataset.tf2DamageIndicator = "authored"
      element.dataset.sourceMaterial = this.#input.material
      element.style.position = "absolute"
      element.style.pointerEvents = "none"
      element.style.backgroundImage = `url("${this.#image}")`
      element.style.backgroundSize = "100% 100%"
      element.style.mixBlendMode = "plus-lighter"
      this.#root.append(element)
      this.#indicators.push(Object.freeze({ element, started: now, lifetime: command.lifetimeSeconds,
        scale: command.scale, direction: Object.freeze(noisy.map((value) => value / length)) as readonly [number, number, number] }))
    }
  }

  frame(now: number, viewport: Readonly<{ width: number; height: number }>): void {
    const proportional = viewport.height / 480
    const yaw = this.#input.yawDegrees() * Math.PI / 180
    const forward = [Math.cos(yaw), Math.sin(yaw)]
    const right = [-Math.sin(yaw), Math.cos(yaw)]
    for (let index = this.#indicators.length - 1; index >= 0; index -= 1) {
      const indicator = this.#indicators[index]!
      const elapsed = now - indicator.started
      if (elapsed >= indicator.lifetime) {
        indicator.element.remove()
        this.#indicators.splice(index, 1)
        continue
      }
      const scale = indicator.scale / 100
      const width = (10 + 90 * scale) * proportional
      const height = (20 + 80 * scale) * proportional
      const radius = (140 - 20 * Math.min(elapsed / 0.1, 1)) * proportional
      const side = indicator.direction[0] * right[0]! + indicator.direction[1] * right[1]!
      const front = indicator.direction[0] * forward[0]! + indicator.direction[1] * forward[1]!
      const rotation = Math.atan2(-side, -front) + Math.PI
      indicator.element.style.left = `${viewport.width / 2 - radius * Math.sin(rotation) - width / 2}px`
      indicator.element.style.top = `${viewport.height / 2 - radius * Math.cos(rotation) - height / 2}px`
      indicator.element.style.width = `${width}px`
      indicator.element.style.height = `${height}px`
      // TF2 DrawDamageIndicator rotates both UV axes by -flRotation in
      // screen coordinates, about the quad centre (not the radial origin).
      indicator.element.style.transform = `rotate(${-rotation}rad)`
      const progress = elapsed / indicator.lifetime
      indicator.element.style.opacity = String(progress <= 0.7 ? 1 : 1 - (progress - 0.7) / 0.3)
    }
  }

  reset(): void {
    for (const indicator of this.#indicators) indicator.element.remove()
    this.#indicators.length = 0
  }
}
