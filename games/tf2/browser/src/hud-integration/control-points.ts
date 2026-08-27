import type { VguiOperation, VguiPanelId, VguiRuntime, VguiViewport } from "@playsrc/vgui"
import type { ControlPoints, RoundSnapshot } from "../codec"
import type { Tf2VguiResources } from "../ui-integration"

type Icon = { wrapper: number; root: number; base: number; swipe: number; players: number; count: number; countdown: number; overlay: number }

export function controlPointSwipeUv(remaining: number, red: boolean, up: boolean): readonly [number, number, number, number] {
  const move = 48 / 69, head = 15 / 69
  const remap = (a: number, b: number) => a + (b - a) * remaining
  if (up) return [0, remap(-move - 0.07, head - 0.07), 1, remap(0.07, move - 0.07)]
  const a = remap(0.9 + move, 1.1 - head), b = remap(0.9, 0)
  return red ? [a, 0, b, 1] : [b, 0, a, 1]
}

/** Authored CP icon resources; all capture decisions and explanatory text come from Rust. */
export class ControlPointHud {
  readonly #runtime: VguiRuntime
  readonly #resources: Tf2VguiResources
  readonly #load: (panel: number, resource: string) => void
  readonly #root: number
  readonly #bubble: number
  readonly #bar: number
  readonly #text: number
  readonly #blocked: number
  readonly #drop: number
  readonly #side: number
  readonly #icons: Icon[] = []
  readonly #values = new Map<string, string>()

  constructor(runtime: VguiRuntime, resources: Tf2VguiResources, load: (panel: number, resource: string) => void) {
    this.#runtime = runtime; this.#resources = resources; this.#load = load
    this.#root = this.#create(1, "EditablePanel", "HudControlPointIcons")
    this.#bubble = this.#create(1, "EditablePanel", "ControlPointProgressBar")
    load(this.#bubble, "resource/ui/controlpointprogressbar.res")
    this.#bar = this.#child(this.#bubble, "ProgressBar")
    this.#text = this.#child(this.#bubble, "ProgressText")
    this.#blocked = this.#child(this.#bubble, "Blocked")
    this.#drop = this.#child(this.#bubble, "Teardrop")
    this.#side = this.#child(this.#bubble, "TeardropSide")
    this.#image(this.#drop, "progress_bar_pointer", 128, 155)
    this.#image(this.#blocked, "progress_bar_noCap", 128, 128)
    this.#visible(this.#side, false)
    this.#visible(this.#bubble, false)
  }

  #apply(operation: VguiOperation): VguiPanelId | undefined {
    const result = this.#runtime.apply(operation)
    if (!result.ok) throw new Error(`${result.diagnostic.code}:${result.diagnostic.subject}`)
    return result.panel
  }
  #put(key: string, value: unknown, operation: VguiOperation): void {
    const encoded = JSON.stringify(value)
    if (this.#values.get(key) === encoded) return
    this.#apply(operation); this.#values.set(key, encoded)
  }
  #create(parent: number, control: "EditablePanel" | "ImagePanel", name: string): number {
    const panel = this.#apply({ kind: "create-panel", parent, control, name, properties: control === "ImagePanel" ? [{ name: "scaleImage", value: "1" }] : undefined })!
    this.#apply({ kind: "set-panel-state", panel, mouseInput: false, keyboardInput: false, proportional: true })
    return panel
  }
  #child(parent: number, name: string): number {
    const panel = this.#runtime.snapshot().panels.find(p => p.parent === parent && p.name.toLowerCase() === name.toLowerCase())
    if (!panel) throw new Error(`Authored control point panel missing: ${name}`)
    return panel.id
  }
  #visible(panel: number, visible: boolean): void { this.#put(`v${panel}`, visible, { kind: "set-panel-state", panel, visible }) }
  #bounds(panel: number, x: number, y: number, width: number, height: number): void {
    const bounds = { x: Math.trunc(x), y: Math.trunc(y), width: Math.trunc(width), height: Math.trunc(height) }
    this.#put(`b${panel}`, bounds, { kind: "set-bounds", panel, bounds })
  }
  #image(panel: number, image: string, cropWidth?: number, cropHeight?: number): void {
    const source = this.#resources.clientScheme.images.find(i => i.name.toLowerCase() === image.toLowerCase())
    if (!source) throw new Error(`Authored control point image missing: ${image}`)
    const mutation = { image, scalarProperties: { scaleImage: 1 }, ...(cropWidth === undefined ? {} : { imageUv: [0, 0, cropWidth / source.width, cropHeight! / source.height] as const }) }
    this.#put(`i${panel}`, mutation, { kind: "mutate-control", panel, mutation })
  }
  #icon(index: number): Icon {
    if (this.#icons[index]) return this.#icons[index]!
    const wrapper = this.#create(this.#root, "EditablePanel", `ControlPointIcon${index}`)
    const root = this.#create(wrapper, "EditablePanel", "ControlPointIcon")
    this.#load(root, "resource/ui/controlpointicon.res")
    const base = this.#create(root, "ImagePanel", "BaseImage"), swipe = this.#create(root, "ImagePanel", "CapImage")
    this.#apply({ kind: "set-panel-state", panel: base, z: 0 })
    this.#apply({ kind: "set-panel-state", panel: swipe, z: 2 })
    const countdown = this.#child(root, "Countdown")
    this.#load(countdown, "resource/ui/controlpointcountdown.res")
    this.#visible(this.#child(root, "CPTimerBG"), false)
    this.#visible(this.#child(root, "CPTimerLabel"), false)
    const icon = { wrapper, root, base, swipe, countdown, players: this.#child(root, "CapPlayerImage"), count: this.#child(root, "CapNumPlayers"), overlay: this.#child(root, "OverlayImage") }
    this.#icons[index] = icon
    return icon
  }

  publish(state: ControlPoints | null | undefined, round: RoundSnapshot | undefined, viewport: VguiViewport, now: number, suppressed: boolean): void {
    this.#visible(this.#root, !!state && !suppressed)
    if (!state || suppressed) { this.#visible(this.#bubble, false); return }
    const scale = viewport.height / 480
    const size = Math.trunc(33 * scale), gap = Math.trunc(9 * scale), vertical = Math.trunc(7 * scale)
    const lines = state.capLayout ? state.capLayout.split(",").map(line => line.trim().split(/\s+/u).map(Number)) : [state.points.flatMap((p, i) => p.visible ? [i] : [])]
    const widths = lines.map(line => gap + line.length * (size + gap)), width = Math.max(...widths), height = vertical + lines.length * (size + vertical)
    const x = state.customPosition[0] === -1 ? (viewport.width - width) / 2 : state.customPosition[0] * viewport.width
    const y = state.customPosition[1] === -1 ? viewport.height - height : state.customPosition[1] * viewport.height
    this.#bounds(this.#root, x, y, width, height)
    for (let line = 0; line < lines.length; line++) {
      for (let column = 0; column < lines[line]!.length; column++) {
        const index = lines[line]![column]!, point = state.points[index]
        if (!point) throw new Error("Authored control point layout references missing point")
        const icon = this.#icon(index), ix = (width - widths[line]!) / 2 + gap + column * (size + gap), iy = vertical + line * (vertical + size)
        this.#bounds(icon.wrapper, ix, iy, size, size)
        this.#bounds(icon.root, 0, 0, size, size); this.#bounds(icon.base, 0, 0, size, size); this.#bounds(icon.swipe, 0, 0, size, size)
        const countdown = point.unlockAt === null ? 0 : Math.trunc(point.unlockAt - now)
        const counting = countdown > 0 && countdown <= 5 && !round?.waitingForPlayers && round?.state === 4
        const locked = !point.mayCapture.some((canCap, t) => canCap && point.owner !== t + 2)
        this.#image(icon.base, `../${point.icon}${locked && !counting ? "_locked" : ""}`)
        this.#visible(icon.wrapper, point.visible)
        this.#visible(icon.countdown, counting)
        if (counting) {
          this.#bounds(icon.countdown, 0, 0, size, size)
          this.#bounds(this.#child(icon.countdown, "CapCountdownLabel"), 0, 0, size, size)
          this.#put(`t${index}`, countdown, { kind: "set-dialog-variable", panel: icon.countdown, name: "capturetime", value: String(countdown) })
        }
        this.#visible(icon.overlay, point.overlay !== "")
        if (point.overlay) this.#image(icon.overlay, `../${point.overlay}`)
        const capturing = point.capturingTeam !== 0 && point.capturingTeam !== point.owner
        this.#visible(icon.swipe, capturing)
        if (capturing) {
          this.#image(icon.swipe, `../sprites/obj_icons/icon_obj_cap_${point.capturingTeam === 2 ? "red" : "blu"}${lines.length > 1 ? "_up" : ""}`)
          const imageUv = controlPointSwipeUv(1 - point.progress, point.capturingTeam === 2, lines.length > 1)
          this.#put(`uv${index}`, imageUv, { kind: "mutate-control", panel: icon.swipe, mutation: { imageUv, fixedDetailUv: true } })
        }
        const count = point.capturingTeam === 0 ? 0 : point.playerCounts[point.capturingTeam - 2]!
        this.#visible(icon.players, count > 0); this.#visible(icon.count, count > 1)
        this.#bounds(icon.players, (size - 10 * scale) / 2 - (count > 1 ? 4 * scale : 0), (size - 20 * scale) / 2, 10 * scale, 20 * scale)
        this.#put(`n${index}`, count, { kind: "set-dialog-variable", panel: icon.root, name: "numcappers", value: count })
        this.#put(`nc${index}`, true, { kind: "mutate-control", panel: icon.count, mutation: { foregroundColor: [0, 0, 0, 255] } })
        if (state.localPoint === index) {
          const bubbleWidth = 100 * scale, bubbleHeight = 65 * scale
          this.#bounds(this.#bubble, x + ix - (bubbleWidth - size) / 2, y + iy - bubbleHeight, bubbleWidth, bubbleHeight)
        }
      }
    }
    const local = state.localPoint === null ? undefined : state.points[state.localPoint]
    this.#visible(this.#bubble, !!local && round?.state !== 5)
    if (local) {
      const ring = state.localCaptureText === ""
      this.#visible(this.#bar, ring); this.#visible(this.#text, !ring); this.#visible(this.#blocked, !ring)
      if (ring) {
        const mutation = { progress: local.progress, foregroundImage: local.owner === 0 ? "progress_bar" : `progress_bar_${local.owner === 2 ? "red" : "blu"}`, backgroundImage: `progress_bar_${local.capturingTeam === 2 ? "red" : "blu"}` }
        this.#put("ring", mutation, { kind: "mutate-control", panel: this.#bar, mutation })
      } else {
        const text = this.#resources.localization.tokens.find(t => t.name.toLowerCase() === state.localCaptureText.replace(/^#/u, "").toLowerCase())?.value
        if (!text) throw new Error(`Authored control point text missing: ${state.localCaptureText}`)
        this.#put("reason", text, { kind: "mutate-control", panel: this.#text, mutation: { text } })
      }
    }
  }
}
