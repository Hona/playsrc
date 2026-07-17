import { describe, expect, test } from "bun:test"
import {
  initializeClientDiagnostics,
  type ClientDiagnosticFrame,
  type ClientDiagnosticResources,
  type ClientDiagnostics,
} from "../src"
import { byName, createRoot, FakeDocument } from "./fake-dom"

const color = (red: number, green: number, blue: number, alpha = 255) =>
  Object.freeze([red, green, blue, alpha] as const)

function resources(): ClientDiagnosticResources {
  return Object.freeze({
    identity: "vgui/client-diagnostics/test",
    scheme: Object.freeze({
      logicalIdentity: "resource/sourcescheme.res",
      tag: "Tracker",
      revision: "test-1",
    }),
    font: Object.freeze({
      logicalIdentity: "resource/sourceschemebase.res#fonts/defaultfixedoutline",
      family: "Lucida Console",
      sizePxAt480: 10,
      lineHeightPxAt480: 10,
      weight: 0,
      style: "normal" as const,
      proportional: false,
      outlinePxAt480: 1,
    }),
    colors: Object.freeze({
      goodFps: color(0, 255, 0),
      warningFps: color(255, 255, 0),
      badFps: color(255, 0, 0),
      position: color(255, 255, 255),
    }),
    panelWidthPx: 300,
    panelPaddingPx: 2,
    panelHeightPaddingPx: 8,
    lineGapPx: 2,
    maximumLines: 4,
  })
}

function frame(
  realTimeMilliseconds: number,
  overrides: Partial<ClientDiagnosticFrame> = {},
): ClientDiagnosticFrame {
  return Object.freeze({
    realTimeMilliseconds,
    fpsMode: 0,
    positionMode: 0,
    mapIdentity: "jump_beef",
    view: Object.freeze({ position: Object.freeze([10, 20, 30]), angles: Object.freeze([1, 2, 3]) }),
    player: Object.freeze({
      position: Object.freeze([40, 50, 60]),
      angles: null,
      velocity: Object.freeze([3, 4, 12]),
    }),
    ...overrides,
  })
}

function mounted(document = new FakeDocument()): Readonly<{ diagnostics: ClientDiagnostics; root: ReturnType<typeof createRoot> }> {
  const initialized = initializeClientDiagnostics({
    runtimeIdentity: "diagnostic-test",
    resources: resources(),
    viewport: { width: 1280, height: 720, devicePixelRatio: 2 },
  })
  if (!initialized.ok) throw new Error(initialized.code)
  const root = createRoot(document)
  expect(initialized.diagnostics.apply({ kind: "mount", root: root as unknown as HTMLElement }).ok).toBe(true)
  return { diagnostics: initialized.diagnostics, root }
}

describe("client diagnostic panel", () => {
  test("rejects malformed configuration and frame input without partial publication", () => {
    expect(initializeClientDiagnostics({
      runtimeIdentity: "Bad identity",
      resources: resources(),
      viewport: { width: 1280, height: 720, devicePixelRatio: 1 },
    })).toEqual({ ok: false, code: "InvalidConfiguration" })
    expect(initializeClientDiagnostics({
      runtimeIdentity: "valid",
      resources: resources(),
      viewport: { width: 0, height: 720, devicePixelRatio: 1 },
    })).toEqual({ ok: false, code: "InvalidViewport" })

    const { diagnostics } = mounted()
    const before = diagnostics.snapshot()
    expect(diagnostics.apply({
      kind: "present",
      frame: frame(Number.NaN),
    })).toEqual({ ok: false, code: "InvalidFrame" })
    expect(diagnostics.snapshot()).toEqual(before)
    diagnostics.apply({ kind: "present", frame: frame(100, { fpsMode: 1 }) })
    const admitted = diagnostics.snapshot()
    expect(diagnostics.apply({ kind: "present", frame: frame(99, { fpsMode: 1 }) })).toEqual({ ok: false, code: "InvalidFrame" })
    expect(diagnostics.snapshot()).toEqual(admitted)
  })

  test("computes exact instantaneous and 0.1-weight smoothed FPS schedules with low, high, milliseconds, map, and threshold colors", () => {
    const { diagnostics, root } = mounted()
    diagnostics.apply({ kind: "present", frame: frame(1_000, { fpsMode: 2 }) })
    expect(diagnostics.snapshot()).toMatchObject({ visible: true, fps: { average: null, low: null, high: null }, lines: [] })

    diagnostics.apply({ kind: "present", frame: frame(1_020, { fpsMode: 2 }) })
    expect(diagnostics.snapshot().lines).toEqual([
      { kind: "fps", text: " 50 fps ( 50,  50) 20.0 ms on jump_beef", color: color(255, 255, 0) },
    ])
    diagnostics.apply({ kind: "present", frame: frame(1_030, { fpsMode: 2 }) })
    expect(diagnostics.snapshot()).toMatchObject({ fps: { average: 55, low: 50, high: 100 } })
    expect(diagnostics.snapshot().lines[0]).toEqual({
      kind: "fps",
      text: " 55 fps ( 50, 100) 10.0 ms on jump_beef",
      color: color(255, 255, 0),
    })

    diagnostics.apply({ kind: "present", frame: frame(1_040, { fpsMode: 1 }) })
    expect(diagnostics.snapshot()).toMatchObject({ fps: { average: null, low: null, high: null } })
    expect(diagnostics.snapshot().lines[0]).toEqual({
      kind: "fps",
      text: "100 fps on jump_beef",
      color: color(0, 255, 0),
    })
    const panel = byName(root, "ClientDiagnostics")
    expect(panel.children[0].style.color).toBe("rgba(0, 255, 0, 1)")

    diagnostics.apply({ kind: "present", frame: frame(1_080, { fpsMode: 1 }) })
    expect(diagnostics.snapshot().lines[0]).toEqual({
      kind: "fps",
      text: " 25 fps on jump_beef",
      color: color(255, 0, 0),
    })
  })

  test("formats view mode and reports unavailable player absolute angles instead of fabricating them", () => {
    const { diagnostics } = mounted()
    diagnostics.apply({ kind: "present", frame: frame(100, { positionMode: 1 }) })
    expect(diagnostics.snapshot().lines).toEqual([
      { kind: "position", text: "pos:  10.00 20.00 30.00", color: color(255, 255, 255) },
      { kind: "position", text: "ang:  1.00 2.00 3.00", color: color(255, 255, 255) },
      { kind: "position", text: "vel:  13.00", color: color(255, 255, 255) },
    ])

    diagnostics.apply({ kind: "present", frame: frame(116, { positionMode: 2 }) })
    expect(diagnostics.snapshot().lines).toEqual([
      { kind: "position", text: "pos:  40.00 50.00 60.00", color: color(255, 255, 255) },
      { kind: "unsupported", text: "ang:  unavailable (player absolute angles)", color: color(255, 255, 255) },
      { kind: "position", text: "vel:  13.00", color: color(255, 255, 255) },
    ])

    diagnostics.apply({ kind: "present", frame: frame(132) })
    expect(diagnostics.snapshot()).toMatchObject({ visible: false, lines: [], fps: { average: null, low: null, high: null } })
  })

  test("uses one fixed-width non-proportional VGUI panel across viewport and DPR schedules", () => {
    const { diagnostics, root } = mounted()
    diagnostics.apply({ kind: "present", frame: frame(100, { positionMode: 1 }) })
    const host = root.children[1]
    const panel = byName(root, "ClientDiagnostics")
    expect([panel.style.left, panel.style.top, panel.style.width, panel.style.height]).toEqual(["980px", "0px", "300px", "48px"])
    expect(host.style.getPropertyValue("--vgui-diagnostic-size")).toBe("10px")

    diagnostics.apply({ kind: "set-viewport", viewport: { width: 1920, height: 1080, devicePixelRatio: 3 } })
    expect([panel.style.left, panel.style.width, panel.style.height]).toEqual(["1620px", "300px", "48px"])
    expect(host.style.getPropertyValue("--vgui-diagnostic-size")).toBe("10px")
    diagnostics.apply({ kind: "set-viewport", viewport: { width: 854, height: 480, devicePixelRatio: 1 } })
    expect([panel.style.left, panel.style.width, panel.style.height]).toEqual(["554px", "300px", "48px"])
  })

  test("replaces roots and destroys repeated panels with no nodes, listeners, observers, or timers", () => {
    for (let iteration = 0; iteration < 25; iteration += 1) {
      const document = new FakeDocument()
      const { diagnostics, root } = mounted(document)
      diagnostics.apply({ kind: "present", frame: frame(iteration + 1, { fpsMode: 1, positionMode: 2 }) })
      const replacement = createRoot(document)
      expect(diagnostics.apply({ kind: "replace-root", root: replacement as unknown as HTMLElement }).ok).toBe(true)
      expect(root.children).toHaveLength(0)
      expect(diagnostics.snapshot().ownedResources).toEqual({ nodes: 6, listeners: 0, observers: 0, timers: 0 })
      diagnostics.apply({ kind: "destroy" })
      expect(replacement.children).toHaveLength(0)
      expect(diagnostics.snapshot()).toMatchObject({
        lifecycle: "destroyed",
        visible: false,
        ownedResources: { nodes: 0, listeners: 0, observers: 0, timers: 0 },
      })
      expect(diagnostics.apply({ kind: "destroy" }).ok).toBe(true)
      expect(diagnostics.apply({ kind: "present", frame: frame(100) })).toEqual({ ok: false, code: "Destroyed" })
    }
  })
})
