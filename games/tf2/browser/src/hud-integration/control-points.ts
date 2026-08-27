import type { VguiOperation, VguiPanelId, VguiRuntime, VguiViewport } from "@playsrc/vgui"
import type { ControlPoints, RoundSnapshot } from "../codec"
import type { Tf2VguiResources } from "../ui-integration"

type Icon = { wrapper: number; root: number; base: number; swipe: number; players: number; count: number; countdown: number; overlay: number; white: number; finish: number; highlight: number; highlightImage: number; owner: number; capping: number; rememberedCapper: number; pulseStart: number | null; pulseDuration: number; accelerate: boolean; finishStart: number | null; swoopStart: number | null }

export function controlPointSwipeUv(progress: number, red: boolean, up: boolean): readonly [number, number, number, number] {
  const move = 48 / 69, head = 15 / 69
  const remap = (a: number, b: number) => a + (b - a) * progress
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
  #team = 0

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
    const white = this.#create(root, "ImagePanel", "CapWhitePulse")
    const finish = this.#create(1, "ImagePanel", `CapPulse${index}`)
    const highlight = this.#create(1, "EditablePanel", `CapHighlight${index}`)
    const highlightImage = this.#create(highlight, "ImagePanel", "CapHighlightImage")
    this.#image(white, "../sprites/obj_icons/icon_obj_white"); this.#image(finish, "../sprites/obj_icons/icon_obj_white")
    this.#image(highlightImage, "../sprites/obj_icons/capture_highlight")
    this.#apply({ kind: "set-panel-state", panel: white, z: 1 })
    this.#apply({ kind: "set-panel-state", panel: finish, z: -1 })
    this.#apply({ kind: "set-panel-state", panel: highlight, z: 10 })
    this.#apply({ kind: "set-panel-state", panel: base, z: 0 })
    this.#apply({ kind: "set-panel-state", panel: swipe, z: 2 })
    const countdown = this.#child(root, "Countdown")
    this.#load(countdown, "resource/ui/controlpointcountdown.res")
    this.#visible(this.#child(root, "CPTimerBG"), false)
    this.#visible(this.#child(root, "CPTimerLabel"), false)
    const icon: Icon = { wrapper, root, base, swipe, countdown, white, finish, highlight, highlightImage, owner: 0, capping: 0, rememberedCapper: 0, pulseStart: null, pulseDuration: 0, accelerate: true, finishStart: null, swoopStart: null, players: this.#child(root, "CapPlayerImage"), count: this.#child(root, "CapNumPlayers"), overlay: this.#child(root, "OverlayImage") }
    this.#icons[index] = icon
    return icon
  }

  publish(state: ControlPoints | null | undefined, round: RoundSnapshot | undefined, team: number, viewport: VguiViewport, now: number, suppressed: boolean): void {
    this.#visible(this.#root, !!state && !suppressed)
    if (!state || suppressed) { this.#visible(this.#bubble, false); for (const icon of this.#icons) { if (icon) { this.#visible(icon.finish, false); this.#visible(icon.highlight, false); } } return }
    if (round?.events.some(event => event.kind === 5)) {
      for (const [index, icon] of this.#icons.entries()) { if (icon) { icon.owner = state.points[index]?.owner ?? 0; icon.capping = 0; icon.rememberedCapper = 0; icon.pulseStart = null; icon.finishStart = null; icon.swoopStart = null; } }
    }
    const scale = viewport.height / 480
    const minimal = this.#resources.resolutionSuffixes.includes("minmode")
    const size = Math.trunc((minimal ? 21 : 33) * scale), gap = Math.trunc(9 * scale), vertical = Math.trunc(7 * scale)
    const lines = state.capLayout ? state.capLayout.split(",").map(line => line.trim().split(/\s+/u).map(Number)) : [state.points.flatMap((p, i) => p.visible ? [i] : [])]
    const widths = lines.map(line => gap + line.length * (size + gap)), width = Math.max(...widths), height = vertical + lines.length * (size + vertical)
    const x = state.customPosition[0] === -1 ? (viewport.width - width) / 2 : state.customPosition[0] * viewport.width
    const y = state.customPosition[1] === -1 ? viewport.height - height : state.customPosition[1] * viewport.height
    this.#bounds(this.#root, x, y, width, height)
    for (let index = 0; index < this.#icons.length; index++) {
      const icon = this.#icons[index]
      if (icon && (!state.points[index]?.visible || !lines.some(line => line.includes(index)))) {
        this.#visible(icon.wrapper, false); this.#visible(icon.finish, false); this.#visible(icon.highlight, false)
        icon.pulseStart = null; icon.finishStart = null; icon.swoopStart = null
      }
    }
    for (let line = 0; line < lines.length; line++) {
      for (let column = 0; column < lines[line]!.length; column++) {
        const index = lines[line]![column]!, point = state.points[index]
        if (!point) throw new Error("Authored control point layout references missing point")
        if (!point.visible) continue
        const icon = this.#icon(index), ix = (width - widths[line]!) / 2 + gap + column * (size + gap), iy = vertical + line * (vertical + size)
        this.#bounds(icon.wrapper, ix, iy, size, size)
        this.#bounds(icon.root, 0, 0, size, size); this.#bounds(icon.base, 0, 0, size, size); this.#bounds(icon.swipe, 0, 0, size, size)
        const countdown = point.unlockAt === null ? 0 : Math.trunc(Math.trunc(point.unlockAt) - now)
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
        if (point.capturingTeam !== icon.capping || point.owner !== icon.owner) {
          if (capturing) {
            icon.pulseStart = now + (team > 1 && team !== point.capturingTeam ? 0.15 : 0)
            icon.pulseDuration = 0; icon.accelerate = true
            icon.swoopStart = team > 1 && team !== point.capturingTeam ? now : null
            icon.rememberedCapper = point.capturingTeam
          } else { icon.pulseStart = null; icon.swoopStart = null }
          if (point.owner !== icon.owner && icon.rememberedCapper !== 0 && point.owner === icon.rememberedCapper) {
            icon.rememberedCapper = 0; icon.finishStart = now; icon.pulseStart = now + 0.2; icon.pulseDuration = 0.5; icon.accelerate = false
          }
          icon.owner = point.owner; icon.capping = point.capturingTeam
        }
        if (team !== this.#team) icon.swoopStart = null
        const pulseElapsed = icon.pulseStart === null ? -1 : now - icon.pulseStart
        this.#visible(icon.white, pulseElapsed > 0)
        if (pulseElapsed > 0) {
          this.#bounds(icon.white, 0, 0, size, size)
          const alpha = Math.trunc(255 * Math.abs(Math.sin(pulseElapsed * (icon.accelerate ? 2 + 3 * Math.max(0, Math.min(1, point.progress)) : 20))))
          this.#put(`pulse${index}`, alpha, { kind: "mutate-control", panel: icon.white, mutation: { drawColor: [255,255,255,alpha] } })
          if (icon.pulseDuration > 0 && pulseElapsed >= icon.pulseDuration) icon.pulseStart = null
        }
        const finishElapsed = icon.finishStart === null ? -1 : now - icon.finishStart
        this.#visible(icon.finish, point.visible && finishElapsed > 0)
        if (finishElapsed > 0) {
          const extent = Math.trunc(size * (3 - 2 * Math.min(1, finishElapsed / 0.2)))
          this.#bounds(icon.finish, x + ix + (size - extent) / 2, y + iy + (size - extent) / 2, extent, extent)
          if (finishElapsed >= 0.2) icon.finishStart = null
        }
        const swoopElapsed = icon.swoopStart === null ? -1 : now - icon.swoopStart
        this.#visible(icon.highlight, point.visible && swoopElapsed >= 0 && swoopElapsed < 0.4)
        if (swoopElapsed >= 0 && swoopElapsed < 0.4) {
          const above = Math.trunc(viewport.height * 0.75), h = above + size - 2 * scale, w = size - 4 * viewport.width / 640
          this.#bounds(icon.highlight, x + ix + 2 * viewport.width / 640, y + iy - above, w, h)
          this.#bounds(icon.highlightImage, 0, h * swoopElapsed / 0.4, w, h)
        }
        this.#visible(icon.swipe, capturing)
        if (capturing) {
          this.#image(icon.swipe, `../sprites/obj_icons/icon_obj_cap_${point.capturingTeam === 2 ? "red" : "blu"}${lines.length > 1 ? "_up" : ""}`)
          const imageUv = controlPointSwipeUv(point.progress, point.capturingTeam === 2, lines.length > 1)
          this.#put(`uv${index}`, imageUv, { kind: "mutate-control", panel: icon.swipe, mutation: { imageUv, fixedDetailUv: true } })
        }
        const count = point.capturingTeam === 0 ? 0 : point.playerCounts[point.capturingTeam - 2]!
        this.#visible(icon.players, count > 0); this.#visible(icon.count, count > 1)
        const playerWidth = (minimal ? 7 : 10) * scale, playerHeight = (minimal ? 14 : 20) * scale
        this.#bounds(icon.players, (size - playerWidth) / 2 - (count > 1 ? 4 * viewport.width / 640 : 0), (size - playerHeight) / 2, playerWidth, playerHeight)
        this.#put(`n${index}`, count, { kind: "set-dialog-variable", panel: icon.root, name: "numcappers", value: count })
        this.#put(`nc${index}`, true, { kind: "mutate-control", panel: icon.count, mutation: { foregroundColor: [0, 0, 0, 255] } })
        if (state.localPoint === index) {
          const bubbleWidth = (minimal ? 65 : 100) * scale, bubbleHeight = (minimal ? 42 : 65) * scale
          const side = line > 0, right = (column + 1) / lines[line]!.length > 0.5
          const edge = (bubbleWidth - (minimal ? 35 : 54) * scale) / 2
          this.#visible(this.#drop, !side); this.#visible(this.#side, side)
          if (side) this.#image(this.#side, `progress_bar_pointer_${right ? "right" : "left"}`, 128, 128)
          this.#bounds(this.#bubble, x + ix + (side ? right ? size - edge : -bubbleWidth + edge : -(bubbleWidth - size) / 2), y + iy - bubbleHeight, bubbleWidth, bubbleHeight)
        }
      }
    }
    this.#team = team
    const local = state.localPoint === null ? undefined : state.points[state.localPoint]
    this.#visible(this.#bubble, !!local && round?.state !== 5)
    if (local) {
      const ring = state.localCaptureText === ""
      this.#visible(this.#bar, ring); this.#visible(this.#text, !ring); this.#visible(this.#blocked, !ring)
      if (ring) {
        const mutation = { progress: 1 - local.progress, foregroundImage: local.owner === 0 ? "progress_bar" : `progress_bar_${local.owner === 2 ? "red" : "blu"}`, backgroundImage: `progress_bar_${local.capturingTeam === 2 ? "red" : "blu"}` }
        this.#put("ring", mutation, { kind: "mutate-control", panel: this.#bar, mutation })
      } else {
        const text = this.#resources.localization.tokens.find(t => t.name.toLowerCase() === state.localCaptureText.replace(/^#/u, "").toLowerCase())?.value
        if (!text) throw new Error(`Authored control point text missing: ${state.localCaptureText}`)
        this.#put("reason", text, { kind: "mutate-control", panel: this.#text, mutation: { text } })
      }
    }
  }
}
