import type { Rgba, VguiOperation, VguiPanelId, VguiPanelSnapshot, VguiRuntime } from "@playsrc/vgui"

type Delta = { panel: VguiPanelId; amount: number; diesAt: number; visible: boolean }

/** CTFHudTimeStatus' ten-entry timer_delta_t ring, rendered in its parent panel. */
export class TimerDeltas {
  readonly #runtime: VguiRuntime
  readonly #parent: VguiPanelId
  readonly #items: Array<Delta | undefined> = new Array(10)
  #head = 0
  #active = 0
  #settings: Readonly<{ x: number; startY: number; endY: number; lifetime: number; font: string; positive: Rgba; negative: Rgba; width: number; height: number }>

  constructor(runtime: VguiRuntime, panel: VguiPanelSnapshot) {
    this.#runtime = runtime
    this.#parent = panel.id
    this.#settings = this.#resolve(panel)
  }

  #apply(operation: VguiOperation): VguiPanelId | undefined {
    const result = this.#runtime.apply(operation)
    if (!result.ok) throw new Error(`${result.diagnostic.code}:${result.diagnostic.subject}`)
    return result.panel
  }

  #resolve(panel: VguiPanelSnapshot) {
    const variables = panel.animationVariables
    const number = (name: string): number => {
      const value = variables[name]
      if (typeof value !== "number" || !Number.isFinite(value)) throw new Error(`TF2 timer delta variable is unavailable: ${name}`)
      return value
    }
    const color = (name: string): Rgba => {
      const value = variables[name]
      if (!Array.isArray(value) || value.length !== 4) throw new Error(`TF2 timer delta color is unavailable: ${name}`)
      return value as Rgba
    }
    const font = variables.delta_item_font
    if (typeof font !== "string") throw new Error("TF2 timer delta font is unavailable")
    return Object.freeze({ x: number("delta_item_x"), startY: number("delta_item_start_y"), endY: number("delta_item_end_y"),
      lifetime: number("delta_lifetime"), font, positive: color("PositiveColor"), negative: color("NegativeColor"), width: panel.bounds.width, height: panel.bounds.height })
  }

  resize(panel: VguiPanelSnapshot): void { this.#settings = this.#resolve(panel) }
  get active(): boolean { return this.#active !== 0 }

  add(amount: number, now: number): void {
    if (amount === 0) return
    const slot = this.#head
    this.#head = (slot + 1) % this.#items.length
    let item = this.#items[slot]
    if (!item) {
      const panel = this.#apply({ kind: "create-panel", parent: this.#parent, control: "CExLabel", name: `TimerDelta${slot}`, properties: [
        { name: "font", value: this.#settings.font }, { name: "textAlignment", value: "north-west" }, { name: "zpos", value: "10" },
      ] })!
      this.#apply({ kind: "set-panel-state", panel, mouseInput: false, keyboardInput: false })
      item = { panel, amount: 0, diesAt: 0, visible: false }
      this.#items[slot] = item
    }
    item.amount = amount
    item.diesAt = now + this.#settings.lifetime
    if (!item.visible) this.#active++
    item.visible = true
    const seconds = amount > 0 ? amount : (-amount) | 0
    this.#apply({ kind: "mutate-control", panel: item.panel, mutation: { text: `${amount > 0 ? "+" : "-"}${Math.trunc(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}` } })
    this.#apply({ kind: "set-panel-state", panel: item.panel, visible: true })
  }

  frame(now: number): void {
    if (!this.#active) return
    const settings = this.#settings
    for (const item of this.#items) {
      if (!item?.visible) continue
      if (item.diesAt <= now) {
        item.visible = false
        this.#active--
        this.#apply({ kind: "set-panel-state", panel: item.panel, visible: false })
        continue
      }
      const percent = (item.diesAt - now) / settings.lifetime
      const color = item.amount > 0 ? settings.positive : settings.negative
      this.#apply({ kind: "set-bounds", panel: item.panel, bounds: { x: Math.trunc(settings.x), y: Math.trunc(settings.endY + percent * (settings.startY - settings.endY)), width: settings.width, height: settings.height } })
      this.#apply({ kind: "mutate-control", panel: item.panel, mutation: { foregroundColor: [color[0], color[1], color[2], percent < 0.5 ? Math.trunc(255 * percent / 0.5) : color[3]] } })
    }
  }

  reset(): void {
    for (const item of this.#items) if (item?.visible) {
      item.visible = false
      this.#apply({ kind: "set-panel-state", panel: item.panel, visible: false })
    }
    this.#head = 0
    this.#active = 0
  }

  destroy(): void {
    for (const item of this.#items) if (item) this.#apply({ kind: "delete-panel", panel: item.panel, deferred: false })
    this.#items.fill(undefined)
    this.#head = 0
    this.#active = 0
  }
}
