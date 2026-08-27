import type { VguiViewport } from "@playsrc/vgui"
import type { Tf2VguiResources } from "../ui-integration"
import type { Tf2HudBinding } from "../hud"
import { deathNoticeGeometry, deathNoticeRow, deathNoticeRivalries, retireDeathNotices, type DeathNoticeRow } from "../hud/deathnotice"
import { tf2DeathNoticeAssets } from "../ui-resources/deathnotice.generated"

const SVG = "http://www.w3.org/2000/svg"
const atlases: Readonly<Record<string, { width: number; height: number; pngDataUrl: string; textureSha256: string }>> = tf2DeathNoticeAssets.atlases

export class Tf2HudDeathNoticePresentation {
  readonly #root: HTMLElement
  readonly #resources: Tf2VguiResources
  readonly #properties: ReadonlyMap<string, string>
  readonly #rows: DeathNoticeRow[] = []
  #viewport: VguiViewport
  #lastEvent: { tick: bigint; ordinal: number } | null = null
  #duration = 6

  constructor(root: HTMLElement, resources: Tf2VguiResources, viewport: VguiViewport) {
    this.#resources = resources
    this.#viewport = viewport
    const layout = resources.document("scripts/hudlayout.res").root.children.find(n => n.name.toLowerCase() === "huddeathnotice")!
    this.#properties = new Map(layout.children.filter(n => {
      if (n.value === null) return false
      if (n.condition === null) return true
      const condition = n.condition.replace(/[\[\]$]/gu, "").toLowerCase()
      const negated = condition.startsWith("!")
      return resources.activeConditions.some(c => c.toLowerCase() === (negated ? condition.slice(1) : condition)) !== negated
    })
      .map(n => [n.name.toLowerCase(), n.value!]))
    this.#root = root.ownerDocument.createElement("div")
    this.#root.dataset.tf2Deathnotice = "authored"
    Object.assign(this.#root.style, { position: "absolute", pointerEvents: "none", overflow: "hidden" })
    root.append(this.#root)
    this.setViewport(viewport)
  }

  #number(name: string): number {
    const value = Number(this.#properties.get(name.toLowerCase()))
    if (!Number.isFinite(value)) throw new Error(`Missing authored death notice ${name}`)
    return value
  }

  #color(name: string): string {
    let value = this.#properties.get(name.toLowerCase()) ?? name
    for (let i = 0; i < 8; i++) {
      if (/^\d+ \d+ \d+ \d+$/u.test(value)) {
        const [r, g, b, a] = value.split(" ").map(Number)
        return `rgba(${r},${g},${b},${a! / 255})`
      }
      const next = this.#resources.clientScheme.colors.find(c => c.name.toLowerCase() === value.toLowerCase())
      if (!next) throw new Error(`Missing authored death notice color ${value}`)
      value = next.value
    }
    throw new Error("Cyclic death notice scheme color")
  }

  setViewport(viewport: VguiViewport): void {
    this.#viewport = viewport
    const scale = Math.trunc(viewport.height) / 480
    const width = Math.trunc(this.#number("wide") * scale)
    const xpos = this.#properties.get("xpos")!
    Object.assign(this.#root.style, {
      left: `${Math.trunc(viewport.width) - Math.trunc(Number(xpos.slice(1)) * scale)}px`,
      top: `${Math.trunc(this.#number("ypos") * scale)}px`,
      width: `${width}px`, height: `${Math.trunc(this.#number("tall") * scale)}px`,
    })
    this.#paint()
  }

  publish(binding: Tf2HudBinding, curtime: number, tickInterval: number): void {
    let changed = false
    const localize = (token: string) => this.#resources.localization.tokens.find(t => t.name.replace(/^#/u, "").toLowerCase() === token.slice(1).toLowerCase())?.value ?? token
    for (const command of binding.commands) {
      if (command.kind !== "killfeed-notice") continue
      if (this.#lastEvent && (command.tick < this.#lastEvent.tick || command.tick === this.#lastEvent.tick && command.ordinal <= this.#lastEvent.ordinal)) continue
      this.#lastEvent = command
      if (this.#duration === 0) continue
      const notice = command.notice
      const player = binding.facts.player.kind === "available" ? binding.facts.player.value : null
      if (notice.silent && player?.team.kind === "available" && notice.victim.team === player.team.value
        && !(notice.victim.identity.kind === "available" && notice.victim.identity.value === 1)) continue
      const row = deathNoticeRow(`${command.tick}_${command.ordinal}`, Math.fround(Math.fround(Number(command.tick)) * tickInterval), notice, localize)
      this.#rows.push(row, ...deathNoticeRivalries(row, localize))
      changed = true
    }
    if (retireDeathNotices(this.#rows, curtime, this.#duration, this.#number("MaxDeathNotices"))) changed = true
    if (changed) this.#paint()
  }

  #paint(): void {
    if (this.#rows.length === 0) { this.#root.replaceChildren(); return }
    const document = this.#root.ownerDocument
    const font = this.#resources.clientScheme.fonts.find(f => f.name.toLowerCase() === this.#properties.get("textfont")!.toLowerCase())!
    const metrics = font.metricsForViewport?.(Math.trunc(this.#viewport.height)) ?? font
    const measure = (text: string) => Math.ceil(metrics.measure!(text, null).width)
    const scale = Math.trunc(this.#viewport.height) / 480
    const width = Math.trunc(this.#number("wide") * scale)
    const nodes = this.#rows.map((row, index) => {
      const g = deathNoticeGeometry(row, index, width, scale, Math.trunc(this.#viewport.width) / 640, this.#number("LineHeight") * scale,
        this.#number("LineSpacing") * scale, metrics.lineHeightPx, measure, this.#number("RightJustify") !== 0)
      const element = document.createElement("div")
      element.dataset.vguiName = `DeathNotice${row.identity}`
      element.dataset.localPlayerInvolved = String(row.notice.localPlayerInvolved)
      Object.assign(element.style, { position: "absolute", left: `${g.x}px`, top: `${g.y}px`, width: `${g.width}px`, height: `${g.height}px`,
        fontFamily: JSON.stringify(font.cssFamily), fontSize: `${metrics.sizePx}px`, lineHeight: `${metrics.lineHeightPx}px`,
        fontWeight: String(metrics.weight ?? font.weight), fontStyle: metrics.style ?? font.style, whiteSpace: "pre" })
      const background = document.createElementNS(SVG, "svg")
      background.setAttribute("width", String(g.width)); background.setAttribute("height", String(g.height))
      background.style.position = "absolute"
      const polygon = document.createElementNS(SVG, "polygon")
      const radius = Math.trunc(this.#number("CornerRadius") * scale)
      const corners = Array.from({ length: 10 }, (_, i) => [radius * (1 - Math.cos(i / 9 * Math.PI / 2)), radius * (1 - Math.sin(i / 9 * Math.PI / 2))])
      const points = [...corners.map(([x, y]) => [x!, y! + 1]), ...corners.toReversed().map(([x, y]) => [g.width - x!, y! + 1]),
        ...corners.map(([x, y]) => [g.width - x!, g.height - 1 - y!]), ...corners.toReversed().map(([x, y]) => [x!, g.height - 1 - y!])]
      polygon.setAttribute("points", points.map(p => p.join(",")).join(" "))
      polygon.setAttribute("fill", this.#color(row.notice.localPlayerInvolved ? "LocalBackgroundColor" : "BaseBackgroundColor"))
      background.append(polygon); element.append(background)
      const teamColor = (team: number) => team === 2 ? this.#color("TeamRed") : team === 3 ? this.#color("TeamBlue")
        : row.notice.localPlayerInvolved ? this.#color("LocalPlayerColor") : "white"
      const text = (value: string, x: number, color: string) => {
        if (!value) return
        const span = document.createElement("span"); span.textContent = value
        Object.assign(span.style, { position: "absolute", left: `${x - g.x}px`, top: `${g.textY - g.y}px`, color })
        element.append(span)
      }
      const icon = (value: DeathNoticeRow["icon"]) => {
        const region = value.icon, atlas = atlases[region.atlas]!
        const svg = document.createElementNS(SVG, "svg")
        svg.dataset.deathIcon = value.name; svg.dataset.sourceTextureSha256 = atlas.textureSha256
        svg.setAttribute("width", String(g.iconWidth)); svg.setAttribute("height", String(g.iconHeight))
        // CHud::SetupNewHudTexture uses half-texel inset UVs.
        svg.setAttribute("viewBox", `${region.x + .5} ${region.y + .5} ${region.width - 1} ${region.height - 1}`)
        svg.setAttribute("preserveAspectRatio", "none")
        Object.assign(svg.style, { position: "absolute", left: `${g.iconX - g.x}px`, top: `${g.iconY - g.y}px` })
        const image = document.createElementNS(SVG, "image")
        image.setAttribute("width", String(atlas.width)); image.setAttribute("height", String(atlas.height)); image.setAttribute("href", atlas.pngDataUrl)
        svg.append(image); element.append(svg)
      }
      text(row.killer, g.killerX, teamColor(row.notice.killer.team))
      if (row.critIcon) icon(row.critIcon)
      icon(row.icon)
      text(row.info, g.infoX, row.notice.localPlayerInvolved ? this.#color("LocalPlayerColor") : "white")
      text(row.victim, g.victimX, teamColor(row.notice.victim.team))
      return element
    })
    this.#root.replaceChildren(...nodes)
  }

  reset(): void { this.#rows.length = 0; this.#lastEvent = null; this.#root.replaceChildren() }
  setDuration(seconds: number): void { this.#duration = Math.fround(seconds) }
  destroy(): void { this.reset(); this.#root.remove() }
}
