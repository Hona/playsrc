import type {
  ClientDiagnosticFrame,
  ClientDiagnosticOperation,
  ClientDiagnosticOperationResult,
  ClientDiagnosticResources,
  ClientDiagnosticSnapshot,
  ClientDiagnostics,
  ClientDiagnosticsConfiguration,
  ClientDiagnosticsInitialization,
  ConsoleViewport,
  Rgba,
} from "./contract"
import { VguiControl, VguiDomRuntime } from "./panel-runtime"

type DiagnosticLine = ClientDiagnosticSnapshot["lines"][number]

type DiagnosticModel = {
  lifecycle: "initialized" | "mounted" | "destroyed"
  revision: number
  viewport: ConsoleViewport
  resources: ClientDiagnosticResources
  visible: boolean
  drawing: boolean
  lastRealTime: number | null
  average: number | null
  low: number | null
  high: number | null
  lines: DiagnosticLine[]
}

const color = (value: Rgba): Rgba => Object.freeze([...value]) as Rgba
const rgba = (value: Rgba): string => `rgba(${value[0]}, ${value[1]}, ${value[2]}, ${value[3] / 255})`
const validColor = (value: Rgba): boolean =>
  Array.isArray(value) && value.length === 4 && value.every((channel) => Number.isSafeInteger(channel) && channel >= 0 && channel <= 255)
const validIdentity = (value: string): boolean =>
  typeof value === "string" && value.length > 0 && value.length <= 1024 && value === value.toLowerCase()
    && /^[\x20-\x7e]+$/u.test(value) && !value.startsWith("/") && !value.includes("\\")
const validViewport = (viewport: ConsoleViewport): boolean =>
  !!viewport && Number.isFinite(viewport.width) && Number.isFinite(viewport.height)
    && viewport.width >= 1 && viewport.width <= 32767 && viewport.height >= 1 && viewport.height <= 32767
    && Number.isFinite(viewport.devicePixelRatio) && viewport.devicePixelRatio >= 0.5 && viewport.devicePixelRatio <= 8
const validVector = (value: readonly number[]): boolean =>
  Array.isArray(value) && value.length === 3 && value.every(Number.isFinite)

function cloneResources(resources: ClientDiagnosticResources): ClientDiagnosticResources {
  return Object.freeze({
    identity: resources.identity,
    scheme: Object.freeze({ ...resources.scheme }),
    font: Object.freeze({ ...resources.font }),
    colors: Object.freeze({
      goodFps: color(resources.colors.goodFps),
      warningFps: color(resources.colors.warningFps),
      badFps: color(resources.colors.badFps),
      position: color(resources.colors.position),
    }),
    panelWidthPx: resources.panelWidthPx,
    panelPaddingPx: resources.panelPaddingPx,
    panelHeightPaddingPx: resources.panelHeightPaddingPx,
    lineGapPx: resources.lineGapPx,
    maximumLines: 4,
  })
}

function validResources(resources: ClientDiagnosticResources): boolean {
  return !!resources
    && validIdentity(resources.identity)
    && validIdentity(resources.scheme.logicalIdentity)
    && resources.scheme.tag.length > 0
    && resources.scheme.revision.length > 0
    && validIdentity(resources.font.logicalIdentity)
    && resources.font.family.length > 0
    && Number.isFinite(resources.font.sizePxAt480)
    && resources.font.sizePxAt480 > 0
    && Number.isFinite(resources.font.lineHeightPxAt480)
    && resources.font.lineHeightPxAt480 >= resources.font.sizePxAt480
    && Number.isSafeInteger(resources.font.weight)
    && resources.font.weight >= 0
    && resources.font.weight <= 1000
    && (resources.font.style === "normal" || resources.font.style === "italic")
    && typeof resources.font.proportional === "boolean"
    && Number.isFinite(resources.font.outlinePxAt480)
    && resources.font.outlinePxAt480 >= 0
    && resources.font.outlinePxAt480 <= 16
    && Object.values(resources.colors).every(validColor)
    && [resources.panelWidthPx, resources.panelPaddingPx, resources.panelHeightPaddingPx, resources.lineGapPx]
      .every((value) => Number.isFinite(value) && value >= 0 && value <= 1024)
    && resources.panelWidthPx >= 1
    && resources.maximumLines === 4
}

function validFrame(frame: ClientDiagnosticFrame): boolean {
  return !!frame
    && Number.isFinite(frame.realTimeMilliseconds)
    && frame.realTimeMilliseconds >= 0
    && (frame.fpsMode === 0 || frame.fpsMode === 1 || frame.fpsMode === 2)
    && (frame.positionMode === 0 || frame.positionMode === 1 || frame.positionMode === 2)
    && /^[a-z0-9_-]{1,255}$/u.test(frame.mapIdentity)
    && validVector(frame.view.position)
    && validVector(frame.view.angles)
    && validVector(frame.player.position)
    && (frame.player.angles === null || validVector(frame.player.angles))
    && validVector(frame.player.velocity)
}

class DiagnosticView {
  readonly runtime: VguiDomRuntime
  readonly panel: VguiControl
  private lineNodes: HTMLElement[] = []

  constructor(root: HTMLElement, identity: string) {
    this.runtime = new VguiDomRuntime(root, { maxDomNodes: 7, maxListeners: 1 })
    this.runtime.host.dataset.runtimeIdentity = identity
    this.runtime.host.dataset.vguiService = "client-diagnostics"
    this.panel = this.runtime.createControl("Panel", "ClientDiagnostics", "section", null)
    this.panel.element.classList.add("playsrc-vgui-diagnostics")
    this.panel.element.setAttribute("role", "status")
    this.panel.element.setAttribute("aria-live", "off")
    this.panel.element.setAttribute("aria-label", "Client diagnostics")
  }

  sync(model: DiagnosticModel): void {
    const { resources, viewport } = model
    const scale = viewport.height / 480
    const fontScale = resources.font.proportional ? scale : 1
    const fontHeight = Math.max(1, Math.trunc(resources.font.lineHeightPxAt480 * fontScale))
    const style = this.runtime.host.style
    style.setProperty("--vgui-diagnostic-font", resources.font.family)
    style.setProperty("--vgui-diagnostic-size", `${Math.max(1, Math.trunc(resources.font.sizePxAt480 * fontScale))}px`)
    style.setProperty("--vgui-diagnostic-line-height", `${fontHeight}px`)
    style.setProperty("--vgui-diagnostic-weight", resources.font.weight === 0 ? "normal" : String(resources.font.weight))
    style.setProperty("--vgui-diagnostic-style", resources.font.style)
    const outline = Math.trunc(resources.font.outlinePxAt480 * fontScale)
    style.setProperty(
      "--vgui-diagnostic-shadow",
      outline > 0
        ? `${-outline}px ${-outline}px 0 #000, ${outline}px ${-outline}px 0 #000, ${-outline}px ${outline}px 0 #000, ${outline}px ${outline}px 0 #000`
        : "none",
    )
    style.setProperty("--vgui-diagnostic-padding", `${resources.panelPaddingPx}px`)
    this.runtime.host.dataset.resourceIdentity = resources.identity
    this.runtime.host.dataset.schemeIdentity = resources.scheme.logicalIdentity
    this.runtime.host.dataset.schemeTag = resources.scheme.tag
    this.runtime.host.dataset.schemeRevision = resources.scheme.revision
    this.panel.setBounds(
      Math.max(0, viewport.width - resources.panelWidthPx),
      0,
      Math.min(viewport.width, resources.panelWidthPx),
      Math.min(viewport.height, resources.maximumLines * fontHeight + resources.panelHeightPaddingPx),
    )
    for (const node of this.lineNodes) this.runtime.releaseOwnedNode(node)
    this.lineNodes = []
    this.panel.element.replaceChildren()
    model.lines.forEach((line, index) => {
      const node = this.runtime.createOwnedNode("span")
      node.className = "playsrc-vgui-diagnostic-line"
      node.dataset.diagnosticKind = line.kind
      node.style.top = `${resources.panelPaddingPx + index * (fontHeight + resources.lineGapPx)}px`
      node.style.height = `${fontHeight}px`
      node.style.color = rgba(line.color)
      node.textContent = line.text
      this.panel.element.append(node)
      this.lineNodes.push(node)
    })
    this.panel.setVisible(model.visible)
  }

  counts(): Readonly<{ nodes: number; listeners: number }> {
    return this.runtime.counts()
  }

  destroy(): void {
    this.runtime.destroy()
    this.lineNodes = []
  }
}

class ClientDiagnosticsImplementation implements ClientDiagnostics {
  private readonly identity: string
  private readonly model: DiagnosticModel
  private view: DiagnosticView | null = null

  constructor(configuration: ClientDiagnosticsConfiguration) {
    this.identity = configuration.runtimeIdentity
    this.model = {
      lifecycle: "initialized",
      revision: 0,
      viewport: Object.freeze({ ...configuration.viewport }),
      resources: cloneResources(configuration.resources),
      visible: false,
      drawing: false,
      lastRealTime: null,
      average: null,
      low: null,
      high: null,
      lines: [],
    }
  }

  apply(operation: ClientDiagnosticOperation): ClientDiagnosticOperationResult {
    if (this.model.lifecycle === "destroyed") {
      return operation.kind === "destroy"
        ? Object.freeze({ ok: true, revision: this.model.revision })
        : Object.freeze({ ok: false, code: "Destroyed" })
    }
    try {
      switch (operation.kind) {
        case "mount":
          if (this.view) return Object.freeze({ ok: false, code: "AlreadyMounted" })
          this.view = new DiagnosticView(operation.root, this.identity)
          this.model.lifecycle = "mounted"
          return this.commit()
        case "replace-root": {
          if (!this.view) return Object.freeze({ ok: false, code: "NotMounted" })
          if (this.view.runtime.root === operation.root) return Object.freeze({ ok: true, revision: this.model.revision })
          const replacement = new DiagnosticView(operation.root, this.identity)
          replacement.sync(this.model)
          this.view.destroy()
          this.view = replacement
          return this.commit()
        }
        case "set-viewport":
          if (!validViewport(operation.viewport)) return Object.freeze({ ok: false, code: "InvalidViewport" })
          this.model.viewport = Object.freeze({ ...operation.viewport })
          return this.commit()
        case "present":
          if (!this.view) return Object.freeze({ ok: false, code: "NotMounted" })
          if (!validFrame(operation.frame)) return Object.freeze({ ok: false, code: "InvalidFrame" })
          if (
            (operation.frame.fpsMode !== 0 || operation.frame.positionMode !== 0)
            && this.model.lastRealTime !== null
            && operation.frame.realTimeMilliseconds < this.model.lastRealTime
          ) {
            return Object.freeze({ ok: false, code: "InvalidFrame" })
          }
          if (
            operation.frame.fpsMode !== 0
            && this.model.lastRealTime !== null
            && operation.frame.realTimeMilliseconds > this.model.lastRealTime
            && !Number.isFinite(1000 / (operation.frame.realTimeMilliseconds - this.model.lastRealTime))
          ) return Object.freeze({ ok: false, code: "InvalidFrame" })
          this.present(operation.frame)
          return this.commit()
        case "destroy":
          this.view?.destroy()
          this.view = null
          this.model.lifecycle = "destroyed"
          this.model.visible = false
          this.model.drawing = false
          this.model.lines = []
          this.model.lastRealTime = null
          this.model.revision += 1
          return Object.freeze({ ok: true, revision: this.model.revision })
      }
    } catch {
      return Object.freeze({ ok: false, code: "DomFailure" })
    }
  }

  snapshot(): ClientDiagnosticSnapshot {
    const counts = this.view?.counts() ?? { nodes: 0, listeners: 0 }
    return Object.freeze({
      runtimeIdentity: this.identity,
      lifecycle: this.model.lifecycle,
      revision: this.model.revision,
      visible: this.model.visible,
      viewport: Object.freeze({ ...this.model.viewport }),
      fps: Object.freeze({ average: this.model.average, low: this.model.low, high: this.model.high }),
      lines: Object.freeze(this.model.lines.map((line) => Object.freeze({ ...line, color: color(line.color) }))),
      ownedResources: Object.freeze({ nodes: counts.nodes, listeners: counts.listeners, observers: 0, timers: 0 }),
    })
  }

  private commit(): ClientDiagnosticOperationResult {
    this.model.revision += 1
    this.view?.sync(this.model)
    return Object.freeze({ ok: true, revision: this.model.revision })
  }

  private present(frame: ClientDiagnosticFrame): void {
    const drawing = frame.fpsMode !== 0 || frame.positionMode !== 0
    if (!drawing) {
      this.model.visible = false
      this.model.drawing = false
      this.model.lastRealTime = null
      this.model.average = null
      this.model.low = null
      this.model.high = null
      this.model.lines = []
      return
    }
    if (!this.model.drawing) {
      this.model.average = null
      this.model.low = null
      this.model.high = null
      this.model.lastRealTime = null
    }
    const lines: DiagnosticLine[] = []
    const elapsed = this.model.lastRealTime === null ? 0 : (frame.realTimeMilliseconds - this.model.lastRealTime) / 1000
    if (frame.fpsMode !== 0 && elapsed > 0) {
      const instantaneous = 1 / elapsed
      let displayed: number
      if (frame.fpsMode === 2) {
        if (this.model.average === null) {
          this.model.average = instantaneous
          this.model.low = Math.trunc(instantaneous)
          this.model.high = Math.trunc(instantaneous)
        } else {
          this.model.average = this.model.average * 0.9 + instantaneous * 0.1
          this.model.low = Math.min(this.model.low ?? Math.trunc(instantaneous), Math.trunc(instantaneous))
          this.model.high = Math.max(this.model.high ?? Math.trunc(instantaneous), Math.trunc(instantaneous))
        }
        displayed = Math.trunc(this.model.average)
        lines.push(this.line(
          "fps",
          `${String(displayed).padStart(3)} fps (${String(this.model.low).padStart(3)}, ${String(this.model.high).padStart(3)}) ${(elapsed * 1000).toFixed(1)} ms on ${frame.mapIdentity}`,
          this.fpsColor(displayed),
        ))
      } else {
        this.model.average = null
        this.model.low = null
        this.model.high = null
        displayed = Math.trunc(instantaneous)
        lines.push(this.line(
          "fps",
          `${String(displayed).padStart(3)} fps on ${frame.mapIdentity}`,
          this.fpsColor(displayed),
        ))
      }
    }
    if (frame.positionMode !== 0) {
      const position = frame.positionMode === 2 ? frame.player.position : frame.view.position
      const angles = frame.positionMode === 2 ? frame.player.angles : frame.view.angles
      lines.push(this.line("position", `pos:  ${position.map((value) => value.toFixed(2)).join(" ")}`, this.model.resources.colors.position))
      lines.push(angles
        ? this.line("position", `ang:  ${angles.map((value) => value.toFixed(2)).join(" ")}`, this.model.resources.colors.position)
        : this.line("unsupported", "ang:  unavailable (player absolute angles)", this.model.resources.colors.position))
      lines.push(this.line(
        "position",
        `vel:  ${Math.hypot(...frame.player.velocity).toFixed(2)}`,
        this.model.resources.colors.position,
      ))
    }
    this.model.visible = true
    this.model.drawing = true
    this.model.lastRealTime = frame.realTimeMilliseconds
    this.model.lines = lines.slice(0, this.model.resources.maximumLines)
  }

  private line(kind: DiagnosticLine["kind"], text: string, value: Rgba): DiagnosticLine {
    return Object.freeze({ kind, text, color: color(value) })
  }

  private fpsColor(fps: number): Rgba {
    if (fps >= 60) return this.model.resources.colors.goodFps
    if (fps >= 50) return this.model.resources.colors.warningFps
    return this.model.resources.colors.badFps
  }
}

export function initializeClientDiagnostics(
  configuration: ClientDiagnosticsConfiguration,
): ClientDiagnosticsInitialization {
  if (
    !configuration
    || !/^[a-z][a-z0-9_-]{0,63}$/u.test(configuration.runtimeIdentity)
    || !validResources(configuration.resources)
  ) return Object.freeze({ ok: false, code: "InvalidConfiguration" })
  if (!validViewport(configuration.viewport)) return Object.freeze({ ok: false, code: "InvalidViewport" })
  return Object.freeze({ ok: true, diagnostics: new ClientDiagnosticsImplementation(configuration) })
}
