import { describe, expect, test } from "bun:test"

import {
  VGUI_GENERIC_CONTROL_NAMES,
  initializeVguiRuntime,
  type VguiAnimationScriptSet,
  type VguiOperation,
  type VguiOperationResult,
  type VguiRequest,
  type VguiResourceNode,
  type VguiRuntime,
  type VguiRuntimeConfiguration,
  type VguiRuntimeLimits,
  type VguiScheme,
} from "../src"
import { FakeDocument, FakeEvent, createRoot, descendants } from "./fake-dom"

const limits: VguiRuntimeLimits = Object.freeze({
  maxPanels: 128,
  maxHierarchyDepth: 16,
  maxChildrenPerPanel: 64,
  maxResourceNodes: 512,
  maxResourceDepth: 16,
  maxPropertiesPerPanel: 128,
  maxStringCodeUnits: 2048,
  maxTextCodeUnits: 4096,
  maxDialogVariables: 64,
  maxLocalizationTokens: 128,
  maxSchemeColors: 128,
  maxSchemeSettings: 128,
  maxSchemeBorders: 32,
  maxSchemeImages: 32,
  maxAnimationScripts: 8,
  maxAnimationSequences: 128,
  maxAnimationCommands: 512,
  maxActiveAnimations: 128,
  maxDelayedCommands: 128,
  maxQueuedMessages: 256,
  maxDiagnostics: 512,
  maxDomNodes: 256,
  maxListeners: 32,
})

const scheme: VguiScheme = Object.freeze({
  identity: "resource/testscheme.res",
  revision: "test-scheme-1",
  tag: "TestScheme",
  colors: Object.freeze([
    { name: "Panel.FgColor", value: "235 235 235 255" },
    { name: "Panel.BgColor", value: "20 30 40 255" },
    { name: "Button.TextColor", value: "255 255 255 255" },
    { name: "Button.BgColor", value: "50 60 70 255" },
    { name: "Button.ArmedTextColor", value: "255 255 255 255" },
    { name: "Button.ArmedBgColor", value: "70 80 90 255" },
    { name: "Button.DepressedTextColor", value: "0 0 0 255" },
    { name: "Button.DepressedBgColor", value: "200 180 100 255" },
    { name: "Frame.BgColor", value: "10 20 30 255" },
    { name: "Frame.OutOfFocusBgColor", value: "5 10 15 255" },
    { name: "Slider.NobColor", value: "255 255 255 255" },
    { name: "Slider.TrackColor", value: "80 80 80 255" },
    { name: "ProgressBar.FgColor", value: "100 220 100 255" },
    { name: "ProgressBar.BgColor", value: "0 0 0 255" },
  ]),
  settings: Object.freeze([]),
  fonts: Object.freeze([
    { name: "Default", cssFamily: "Test Sans", sizePx: 16, lineHeightPx: 18, weight: 500, style: "normal" as const, available: true },
  ]),
  borders: Object.freeze([
    {
      kind: "line",
      name: "BaseBorder",
      inset: { left: 1, top: 2, right: 3, bottom: 4 },
      backgroundType: 0,
      paintFirst: false,
      sides: {
        left: [{ color: [255, 0, 0, 255], startOffset: 0, endOffset: 0 }],
        top: [{ color: [0, 255, 0, 255], startOffset: 0, endOffset: 0 }],
        right: [{ color: [0, 0, 255, 255], startOffset: 0, endOffset: 0 }],
        bottom: [{ color: [255, 255, 0, 255], startOffset: 0, endOffset: 0 }],
      },
    },
  ]),
  images: Object.freeze([
    {
      name: "test/icon",
      logicalIdentity: "materials/vgui/test/icon.vtf",
      revision: "test-image-1",
      browserUrl: "data:image/png;base64,AA==",
      width: 32,
      height: 16,
      frames: 1,
      hardwareFiltered: false,
    },
  ]),
})

const emptyAnimations: VguiAnimationScriptSet = Object.freeze({
  identity: "scripts/empty",
  revision: "empty-1",
  scripts: Object.freeze([]),
  activeConditions: Object.freeze([]),
})

function scalar(name: string, value: string, condition: string | null = null): VguiResourceNode {
  return Object.freeze({ name, value, condition, children: Object.freeze([]) })
}

function object(name: string, children: readonly VguiResourceNode[], condition: string | null = null): VguiResourceNode {
  return Object.freeze({ name, value: null, condition, children: Object.freeze(children) })
}

function operation(runtime: VguiRuntime, value: VguiOperation): Extract<VguiOperationResult, { ok: true }> {
  const result = runtime.apply(value)
  if (!result.ok) throw new Error(`${result.diagnostic.code}: ${result.diagnostic.subject}`)
  return result
}

function setup(animationScripts = emptyAnimations, customControls: VguiRuntimeConfiguration["customControls"] = [], initialTime = 0) {
  const document = new FakeDocument()
  const root = createRoot(document)
  const requests: VguiRequest[] = []
  let now = initialTime
  const initialized = initializeVguiRuntime({
    runtimeIdentity: "test-runtime",
    root: root as unknown as HTMLElement,
    rootControl: { control: "EditablePanel", name: "Root" },
    viewport: { width: 640, height: 480, devicePixelRatio: 1 },
    limits,
    clock: { nowSeconds: () => now },
    random: { nextUnit: () => 0.25 },
    scheme,
    localization: {
      identity: "resource/test_english.txt",
      revision: "test-localization-1",
      language: "english",
      tokens: [{ name: "OK", value: "&Accept %name%" }, { name: "URL", value: "https://example.test/tf2" }],
    },
    animationScripts,
    customControls,
    reducedMotion: false,
    onRequest: (request) => requests.push(request),
  })
  if (!initialized.ok) throw new Error(initialized.diagnostic.code)
  return {
    document,
    root,
    runtime: initialized.runtime,
    requests,
    time(value: number) { now = value },
  }
}

describe("generic Source VGUI runtime", () => {
  test("leaves foreign keyboard and input events to their owning DOM context", () => {
    const { document, root, runtime } = setup()
    const entry = operation(runtime, {
      kind: "create-panel",
      parent: 1,
      control: "TextEntry",
      name: "Entry",
      properties: [{ name: "wide", value: "100" }, { name: "tall", value: "24" }],
    }).panel!
    const foreign = document.createElement("input")
    foreign.dataset.vguiPanel = String(entry)
    foreign.value = "foreign text"

    for (const [key, code] of [[" ", "Space"], ["Backspace", "Backspace"]] as const) {
      const event = new FakeEvent("keydown", { key, code })
      event.target = foreign
      document.dispatchEvent(event)
      expect(event.defaultPrevented).toBeFalse()
    }
    const input = new FakeEvent("input")
    input.target = foreign
    document.dispatchEvent(input)

    expect(runtime.snapshot().input.downKeys).toEqual([])
    expect(runtime.snapshot().panels.find((panel) => panel.id === entry)?.text).toBe("")

    const owned = descendants(root).find((element) => element.dataset.vguiPanel === String(entry))!
    const backspace = new FakeEvent("keydown", { key: "Backspace", code: "Backspace" })
    backspace.target = owned
    document.dispatchEvent(backspace)
    expect(backspace.defaultPrevented).toBeTrue()
  })

  test("defers presentation and static frames retain mounted DOM", () => {
    const { root, runtime, time } = setup()
    let panelsDuringBatch = 0
    let mountedDuringBatch = 0
    runtime.deferPresentation(() => {
      const first = operation(runtime, { kind: "create-panel", parent: 1, control: "Label", name: "First" }).panel!
      operation(runtime, { kind: "set-bounds", panel: first, bounds: { x: 10, y: 20, width: 100, height: 30 } })
      operation(runtime, { kind: "create-panel", parent: 1, control: "Button", name: "Second" })
      panelsDuringBatch = runtime.snapshot().panels.length
      mountedDuringBatch = descendants(root).filter((node) => node.dataset.vguiPanel !== undefined).length
    })
    expect(panelsDuringBatch).toBe(3)
    expect(mountedDuringBatch).toBe(1)
    expect(descendants(root).filter((node) => node.dataset.vguiPanel !== undefined)).toHaveLength(3)
    const appendCalls = descendants(root).reduce((total, node) => total + node.appendCalls, 0)
    time(1)
    operation(runtime, { kind: "frame", timeSeconds: 1 })
    expect(descendants(root).reduce((total, node) => total + node.appendCalls, 0)).toBe(appendCalls)
    expect(runtime.snapshot().frame).toBe(1)
  })

  test("admits stale callback timestamps without reversing the shared monotonic frame clock", () => {
    const { root, runtime, time } = setup(emptyAnimations, [], 10)
    const entry = operation(runtime, {
      kind: "create-panel", parent: 1, control: "TextEntry", name: "ClockEntry",
      properties: [{ name: "wide", value: "100" }, { name: "tall", value: "24" }],
    }).panel!

    operation(runtime, { kind: "frame", timeSeconds: 9 })
    expect(runtime.snapshot().timeSeconds).toBe(10)
    operation(runtime, { kind: "frame", timeSeconds: 9 })
    expect(runtime.snapshot().timeSeconds).toBe(10)

    const element = descendants(root).find((candidate) => candidate.dataset.vguiPanel === String(entry))!
    time(11)
    element.dispatchEvent(new FakeEvent("pointerdown", { bubbles: true, pointerId: 1, clientX: 4, clientY: 4 }))
    expect(runtime.snapshot().timeSeconds).toBe(11)
    operation(runtime, { kind: "frame", timeSeconds: 10 })
    expect(runtime.snapshot().timeSeconds).toBe(11)

    for (const value of [9.5, -1, Number.NaN, Number.POSITIVE_INFINITY]) {
      const rejected = runtime.apply({ kind: "frame", timeSeconds: value })
      expect(rejected).toMatchObject({ ok: false, diagnostic: { code: "MalformedValue", subject: "frame-time" } })
      expect(runtime.snapshot().timeSeconds).toBe(11)
    }

    operation(runtime, { kind: "set-panel-state", panel: 1, visible: false })
    time(25)
    operation(runtime, { kind: "set-panel-state", panel: 1, visible: true })
    operation(runtime, { kind: "frame", timeSeconds: 24 })
    expect(runtime.snapshot().timeSeconds).toBe(24)
  })

  test("keeps independently initialized, hidden, resumed, and replacement roots on one monotonic clock", () => {
    const document = new FakeDocument()
    let now = 100
    const create = (identity: string) => {
      const initialized = initializeVguiRuntime({
        runtimeIdentity: identity,
        root: createRoot(document) as unknown as HTMLElement,
        rootControl: { control: "EditablePanel", name: "Root" },
        viewport: { width: 640, height: 480, devicePixelRatio: 1 },
        limits,
        clock: { nowSeconds: () => now },
        random: { nextUnit: () => 0.25 },
        scheme,
        localization: { identity: "resource/shared-clock.txt", revision: "1", language: "english", tokens: [] },
        animationScripts: emptyAnimations,
        customControls: [],
        reducedMotion: false,
        onRequest() {},
      })
      if (!initialized.ok) throw new Error(initialized.diagnostic.code)
      return initialized.runtime
    }
    const startup = create("startup")
    now = 101
    const loading = create("loading")
    const hud = create("hud")
    const options = create("options")
    for (const runtime of [startup, loading, hud, options]) operation(runtime, { kind: "frame", timeSeconds: 99 })
    expect([startup, loading, hud, options].map((runtime) => runtime.snapshot().timeSeconds)).toEqual([100, 101, 101, 101])

    operation(loading, { kind: "set-panel-state", panel: 1, visible: false })
    operation(options, { kind: "set-panel-state", panel: 1, visible: false })
    for (const runtime of [startup, hud]) operation(runtime, { kind: "frame", timeSeconds: 120 })
    operation(loading, { kind: "set-panel-state", panel: 1, visible: true })
    operation(options, { kind: "set-panel-state", panel: 1, visible: true })
    for (const runtime of [loading, options]) operation(runtime, { kind: "frame", timeSeconds: 120 })
    expect([startup, loading, hud, options].map((runtime) => runtime.snapshot().timeSeconds)).toEqual([120, 120, 120, 120])

    operation(hud, { kind: "destroy" })
    now = 121
    const replacement = create("replacement-hud")
    operation(replacement, { kind: "frame", timeSeconds: 120 })
    expect(replacement.snapshot().timeSeconds).toBe(121)
    operation(replacement, { kind: "frame", timeSeconds: 122 })
    expect(replacement.apply({ kind: "frame", timeSeconds: 121 })).toMatchObject({
      ok: false,
      diagnostic: { code: "MalformedValue", subject: "frame-time" },
    })
    expect(replacement.snapshot().timeSeconds).toBe(122)
    for (const runtime of [startup, loading, options, replacement]) operation(runtime, { kind: "destroy" })
  })

  test("consumes a focus request invalidated before its frame", () => {
    const { root, runtime, time } = setup()
    const first = operation(runtime, { kind: "create-panel", parent: 1, control: "Button", name: "FirstFocus" }).panel!
    const second = operation(runtime, { kind: "create-panel", parent: 1, control: "Button", name: "SecondFocus" }).panel!
    operation(runtime, { kind: "request-focus", panel: first })
    time(1)
    operation(runtime, { kind: "frame", timeSeconds: 1 })
    operation(runtime, { kind: "request-focus", panel: second })
    operation(runtime, { kind: "set-panel-state", panel: second, visible: false })
    time(2)
    operation(runtime, { kind: "frame", timeSeconds: 2 })
    time(3)
    operation(runtime, { kind: "frame", timeSeconds: 3 })
    const host = descendants(root).find((node) => node.dataset.vguiRuntime === "test-runtime")!
    expect(host.dataset.vguiFrameWork).toBe("static")
    expect(runtime.snapshot().input.keyFocus).toBe(first)
  })

  test("instantiates every selected generic factory with one stable DOM owner and cleans up", () => {
    const { root, runtime } = setup()
    for (const [index, control] of VGUI_GENERIC_CONTROL_NAMES.entries()) {
      operation(runtime, { kind: "create-panel", parent: 1, control, name: `${control}${index}` })
    }
    const snapshot = runtime.snapshot()
    expect(snapshot.panels).toHaveLength(VGUI_GENERIC_CONTROL_NAMES.length + 3)
    expect(new Set(snapshot.panels.map((panel) => panel.id)).size).toBe(snapshot.panels.length)
    expect(descendants(root).filter((node) => node.dataset.vguiPanel !== undefined)).toHaveLength(snapshot.panels.length)
    expect(snapshot.panels.find((panel) => panel.control === "CheckButton")?.role).toBe("checkbox")
    expect(snapshot.panels.find((panel) => panel.control === "QueryBox")?.role).toBe("dialog")
    expect(snapshot.ownedResources.listeners).toBe(13)

    operation(runtime, { kind: "destroy" })
    expect(runtime.snapshot().ownedResources).toEqual({ nodes: 0, listeners: 0, observers: 0, timers: 0 })
    expect(root.children).toHaveLength(0)
  })

  test("atomically reuses code controls, selects conditions and suffixes, localizes variables, and rejects unknown controls", () => {
    const { runtime } = setup()
    operation(runtime, { kind: "set-dialog-variable", panel: 1, name: "name", value: "Engineer" })
    const existing = operation(runtime, {
      kind: "create-panel",
      parent: 1,
      control: "Button",
      name: "Action",
      properties: [{ name: "wide", value: "10" }, { name: "tall", value: "10" }],
    }).panel!
    operation(runtime, {
      kind: "replace-resource",
      parent: 1,
      document: {
        logicalIdentity: "resource/test.res",
        revision: "resource-1",
        root: object("Test", [
          object("Action", [
            scalar("fieldName", "AcceptButton"),
            scalar("xpos", "c-60"),
            scalar("ypos", "20"),
            scalar("wide", "80"),
            scalar("wide_minmode", "120"),
            scalar("tall", "30"),
            scalar("labelText", "#OK"),
            scalar("font", "Default"),
            scalar("border", "BaseBorder"),
            scalar("visible", "0", "[$HIDDEN]"),
          ]),
          object("Icon", [
            scalar("ControlName", "ImagePanel"),
            scalar("xpos", "10"), scalar("ypos", "60"), scalar("wide", "32"), scalar("tall", "16"),
            scalar("image", "test/icon"),
          ]),
        ]),
      },
      selection: { activeConditions: ["WIN32"], resolutionSuffixes: ["_minmode"] },
    })
    const snapshot = runtime.snapshot()
    const button = snapshot.panels.find((panel) => panel.name === "AcceptButton")!
    expect(button.id).toBe(existing)
    expect(button.bounds).toEqual({ x: 260, y: 20, width: 120, height: 30 })
    expect(button.text).toBe("Accept Engineer")
    expect(button.inset).toEqual({ left: 1, top: 2, right: 3, bottom: 4 })
    expect(snapshot.panels.find((panel) => panel.name === "Icon")?.resourceOwner).toBe("resource/test.res")

    const before = runtime.snapshot().panels.map((panel) => [panel.id, panel.name])
    const failure = runtime.apply({
      kind: "replace-resource",
      parent: 1,
      document: {
        logicalIdentity: "resource/bad.res",
        revision: "bad-1",
        root: object("Bad", [object("Unknown", [scalar("ControlName", "NotAControl")])]),
      },
      selection: { activeConditions: [], resolutionSuffixes: [] },
    })
    expect(failure.ok).toBeFalse()
    if (!failure.ok) expect(failure.diagnostic.code).toBe("UnknownControl")
    expect(runtime.snapshot().panels.map((panel) => [panel.id, panel.name])).toEqual(before)
  })

  test("routes capture, checkbox/radio, slider, text, menu and typed commands deterministically", () => {
    const { runtime, requests } = setup()
    const check = operation(runtime, {
      kind: "create-panel", parent: 1, control: "CheckButton", name: "Check",
      properties: [
        { name: "xpos", value: "10" }, { name: "ypos", value: "10" }, { name: "wide", value: "100" }, { name: "tall", value: "30" },
        { name: "labelText", value: "Check" }, { name: "command", value: "check_changed" }, { name: "tabPosition", value: "1" },
      ],
    }).panel!
    const radioA = operation(runtime, {
      kind: "create-panel", parent: 1, control: "RadioButton", name: "RadioA",
      properties: [{ name: "xpos", value: "10" }, { name: "ypos", value: "50" }, { name: "wide", value: "100" }, { name: "tall", value: "30" }, { name: "TabPosition", value: "2" }, { name: "selected", value: "0" }],
    }).panel!
    const radioB = operation(runtime, {
      kind: "create-panel", parent: 1, control: "RadioButton", name: "RadioB",
      properties: [{ name: "xpos", value: "10" }, { name: "ypos", value: "90" }, { name: "wide", value: "100" }, { name: "tall", value: "30" }, { name: "TabPosition", value: "2" }, { name: "selected", value: "0" }],
    }).panel!
    const slider = operation(runtime, {
      kind: "create-panel", parent: 1, control: "Slider", name: "Slider",
      properties: [{ name: "xpos", value: "150" }, { name: "ypos", value: "10" }, { name: "wide", value: "100" }, { name: "tall", value: "20" }, { name: "rangeMin", value: "0" }, { name: "rangeMax", value: "10" }, { name: "value", value: "5" }, { name: "tabPosition", value: "3" }],
    }).panel!
    const entry = operation(runtime, {
      kind: "create-panel", parent: 1, control: "TextEntry", name: "Entry",
      properties: [{ name: "xpos", value: "150" }, { name: "ypos", value: "50" }, { name: "wide", value: "120" }, { name: "tall", value: "24" }, { name: "NumericInputOnly", value: "1" }, { name: "unicode", value: "1" }, { name: "tabPosition", value: "4" }],
    }).panel!
    const menu = operation(runtime, {
      kind: "create-panel", parent: 1, control: "Menu", name: "Menu",
      properties: [{ name: "xpos", value: "300" }, { name: "ypos", value: "10" }, { name: "wide", value: "120" }, { name: "tall", value: "100" }, { name: "tabPosition", value: "5" }],
    }).panel!
    operation(runtime, { kind: "mutate-control", panel: menu, mutation: { items: [{ id: 7, text: "Apply", command: "apply_options" }] } })
    operation(runtime, { kind: "set-panel-state", panel: menu, visible: true, popup: true })

    operation(runtime, { kind: "pointer-press", button: "left", x: 20, y: 20, pointerId: 1, clicks: 1 })
    operation(runtime, { kind: "frame", timeSeconds: 0 })
    expect(runtime.snapshot().panels.find((panel) => panel.id === check)?.state.checked).toBeTrue()
    expect(requests.some((request) => request.kind === "command" && request.command === "check_changed")).toBeTrue()

    operation(runtime, { kind: "pointer-press", button: "left", x: 20, y: 60, pointerId: 1, clicks: 1 })
    operation(runtime, { kind: "frame", timeSeconds: 0.01 })
    operation(runtime, { kind: "pointer-press", button: "left", x: 20, y: 100, pointerId: 1, clicks: 1 })
    operation(runtime, { kind: "frame", timeSeconds: 0.02 })
    const radios = runtime.snapshot().panels.filter((panel) => panel.id === radioA || panel.id === radioB)
    expect(radios.map((panel) => panel.state.selected)).toEqual([false, true])

    operation(runtime, { kind: "request-focus", panel: slider })
    operation(runtime, { kind: "frame", timeSeconds: 0.03 })
    operation(runtime, { kind: "key-typed", key: "ArrowRight", shift: false, control: false, alt: false, meta: false })
    operation(runtime, { kind: "frame", timeSeconds: 0.04 })
    expect(runtime.snapshot().panels.find((panel) => panel.id === slider)?.state.value).toBe(6)

    operation(runtime, { kind: "request-focus", panel: entry })
    operation(runtime, { kind: "frame", timeSeconds: 0.05 })
    operation(runtime, { kind: "text-input", text: "12x.5" })
    operation(runtime, { kind: "frame", timeSeconds: 0.06 })
    expect(runtime.snapshot().panels.find((panel) => panel.id === entry)?.text).toBe("12.5")

    operation(runtime, { kind: "request-focus", panel: menu })
    operation(runtime, { kind: "frame", timeSeconds: 0.07 })
    operation(runtime, { kind: "key-typed", key: "ArrowDown", shift: false, control: false, alt: false, meta: false })
    operation(runtime, { kind: "key-typed", key: "Enter", shift: false, control: false, alt: false, meta: false })
    operation(runtime, { kind: "frame", timeSeconds: 0.08 })
    expect(requests.some((request) => request.kind === "command" && request.command === "apply_options")).toBeTrue()
  })

  test("routes a code-created button command to its declared action-signal ancestor", () => {
    const { runtime, requests } = setup()
    const frame = operation(runtime, {
      kind: "create-panel", parent: 1, control: "Frame", name: "Options", properties: [
        { name: "xpos", value: "100" }, { name: "ypos", value: "100" }, { name: "wide", value: "300" }, { name: "tall", value: "200" },
      ],
    }).panel!
    operation(runtime, { kind: "set-panel-state", panel: frame, visible: true })
    operation(runtime, {
      kind: "create-panel", parent: frame, control: "Button", name: "Apply", properties: [
        { name: "xpos", value: "20" }, { name: "ypos", value: "20" }, { name: "wide", value: "80" }, { name: "tall", value: "24" },
        { name: "command", value: "Apply" }, { name: "actionsignallevel", value: "1" },
      ],
    })
    operation(runtime, { kind: "pointer-press", button: "left", x: 130, y: 130, pointerId: 1, clicks: 1 })
    operation(runtime, { kind: "pointer-release", button: "left", x: 130, y: 130, pointerId: 1 })
    operation(runtime, { kind: "frame", timeSeconds: 0.01 })
    expect(requests).toContainEqual({ kind: "command", panel: 3, command: "Apply" })
  })

  test("preserves current equal-z sibling order through default insertion, z changes, and front/back movement", () => {
    const { root, runtime } = setup()
    const create = (name: string, z?: number) => operation(runtime, {
      kind: "create-panel", parent: 1, control: "Button", name,
      properties: [{ name: "wide", value: "80" }, { name: "tall", value: "40" }, ...(z === undefined ? [] : [{ name: "zpos", value: String(z) }])],
    }).panel!
    const first = create("First", 5)
    const second = create("Second", 5)
    const zero = create("Zero")
    const laterZero = create("LaterZero")
    const names = () => runtime.snapshot().panels.find((panel) => panel.id === 1)!.children.map((id) => runtime.snapshot().panels.find((panel) => panel.id === id)!.name)
    const domNames = () => descendants(root).find((element) => element.dataset.vguiPanel === "1")!.children.map((element) => element.dataset.vguiName).filter(Boolean)

    expect(names()).toEqual(["Zero", "LaterZero", "Second", "First"])
    expect(domNames()).toEqual(names())
    operation(runtime, { kind: "set-panel-state", panel: zero, z: 5 })
    expect(names()).toEqual(["LaterZero", "Zero", "Second", "First"])
    expect(domNames()).toEqual(names())
    operation(runtime, { kind: "set-panel-state", panel: first, z: -3 })
    expect(names()).toEqual(["First", "LaterZero", "Zero", "Second"])
    operation(runtime, { kind: "move-to-front", panel: zero })
    expect(names()).toEqual(["First", "LaterZero", "Second", "Zero"])
    operation(runtime, { kind: "move-to-back", panel: zero })
    expect(names()).toEqual(["First", "LaterZero", "Zero", "Second"])
    expect(domNames()).toEqual(names())
    operation(runtime, { kind: "pointer-move", x: 4, y: 4, pointerId: 1 })
    operation(runtime, { kind: "frame", timeSeconds: 0 })
    expect(runtime.snapshot().input.mouseOver).toBe(second)
    expect(laterZero).toBeGreaterThan(zero)
  })

  test("projects independent popup stack order without using authored child z values", () => {
    const { root, runtime } = setup()
    operation(runtime, { kind: "set-panel-state", panel: 1, z: 200 })
    const first = operation(runtime, {
      kind: "create-panel", parent: 1, control: "Frame", name: "FirstWindow",
      properties: [{ name: "xpos", value: "10" }, { name: "ypos", value: "10" }, { name: "wide", value: "160" }, { name: "tall", value: "100" }, { name: "zpos", value: "100" }],
    }).panel!
    expect(runtime.snapshot().panels.find((panel) => panel.id === first)?.visible).toBeFalse()
    const second = operation(runtime, {
      kind: "create-panel", parent: 1, control: "Frame", name: "SecondWindow",
      properties: [{ name: "xpos", value: "10" }, { name: "ypos", value: "10" }, { name: "wide", value: "160" }, { name: "tall", value: "100" }, { name: "zpos", value: "-100" }],
    }).panel!
    operation(runtime, { kind: "set-panel-state", panel: first, visible: true })
    operation(runtime, { kind: "set-panel-state", panel: second, visible: true })
    const host = descendants(root).find((element) => element.dataset.vguiRuntime === "test-runtime")!
    const windowNames = () => host.children.map((element) => element.dataset.vguiName).filter(Boolean)
    expect(windowNames()).toEqual(["Root", "FirstWindow", "SecondWindow"])
    expect(host.children[1]!.style.zIndex).toBe(host.children[2]!.style.zIndex)
    operation(runtime, { kind: "pointer-move", x: 20, y: 50, pointerId: 1 })
    expect(runtime.snapshot().input.mouseOver).toBe(second)
    operation(runtime, { kind: "move-to-front", panel: first })
    expect(windowNames()).toEqual(["Root", "SecondWindow", "FirstWindow"])
    operation(runtime, { kind: "pointer-move", x: 20, y: 50, pointerId: 1 })
    expect(runtime.snapshot().input.mouseOver).toBe(first)
    operation(runtime, { kind: "set-panel-state", panel: second, topmostPopup: true })
    expect(windowNames()).toEqual(["Root", "FirstWindow", "SecondWindow"])
    operation(runtime, { kind: "pointer-move", x: 20, y: 50, pointerId: 1 })
    expect(runtime.snapshot().input.mouseOver).toBe(second)
  })

  test("solves z order, clipping, popups, modal scope and focus loss-before-gain", () => {
    const { runtime } = setup()
    const back = operation(runtime, {
      kind: "create-panel", parent: 1, control: "Button", name: "Back",
      properties: [{ name: "xpos", value: "0" }, { name: "ypos", value: "0" }, { name: "wide", value: "100" }, { name: "tall", value: "100" }, { name: "zpos", value: "0" }, { name: "tabPosition", value: "1" }],
    }).panel!
    const front = operation(runtime, {
      kind: "create-panel", parent: 1, control: "Button", name: "Front",
      properties: [{ name: "xpos", value: "50" }, { name: "ypos", value: "50" }, { name: "wide", value: "100" }, { name: "tall", value: "100" }, { name: "zpos", value: "0" }, { name: "tabPosition", value: "2" }],
    }).panel!
    const popup = operation(runtime, {
      kind: "create-panel", parent: back, control: "Menu", name: "Popup",
      properties: [{ name: "xpos", value: "300" }, { name: "ypos", value: "200" }, { name: "wide", value: "80" }, { name: "tall", value: "80" }],
    }).panel!
    operation(runtime, { kind: "set-panel-state", panel: popup, popup: true, visible: true })
    operation(runtime, { kind: "pointer-move", x: 75, y: 75, pointerId: 1 })
    operation(runtime, { kind: "frame", timeSeconds: 0 })
    expect(runtime.snapshot().input.mouseOver).toBe(front)
    operation(runtime, { kind: "move-to-front", panel: back })
    operation(runtime, { kind: "pointer-move", x: 75, y: 75, pointerId: 1 })
    operation(runtime, { kind: "frame", timeSeconds: 0.01 })
    expect(runtime.snapshot().input.mouseOver).toBe(back)
    expect(runtime.snapshot().panels.find((panel) => panel.id === popup)?.clip).toEqual({ x: 300, y: 200, width: 80, height: 80 })

    operation(runtime, { kind: "set-application-modal", panel: popup })
    operation(runtime, { kind: "pointer-move", x: 75, y: 75, pointerId: 1 })
    operation(runtime, { kind: "frame", timeSeconds: 0.02 })
    expect(runtime.snapshot().input.mouseOver).toBeNull()
    operation(runtime, { kind: "set-application-modal", panel: null })
    operation(runtime, { kind: "request-focus", panel: back })
    operation(runtime, { kind: "frame", timeSeconds: 0.03 })
    operation(runtime, { kind: "request-focus", panel: front })
    operation(runtime, { kind: "frame", timeSeconds: 0.04 })
    const focusTrace = runtime.snapshot().trace.filter((entry) => entry.phase === "message" && (entry.detail === "KillFocus" || entry.detail === "SetFocus"))
    expect(focusTrace.slice(-2).map((entry) => entry.detail)).toEqual(["KillFocus", "SetFocus"])
  })

  test("uses first animation declaration, virtual time, exact command order, reduced motion and viewport reload", () => {
    const animations: VguiAnimationScriptSet = {
      identity: "scripts/test-set",
      revision: "animations-1",
      activeConditions: [],
      scripts: [
        {
          logicalIdentity: "scripts/base.txt",
          revision: "base-1",
          sequences: [{
            name: "Move",
            condition: null,
            commands: [
              { kind: "Animate", panel: "Target", variable: "XPos", target: "100", interpolator: "Linear", parameter: 0, delaySeconds: 0, durationSeconds: 1, condition: null },
              { kind: "SetVisible", panel: "Target", visible: false, delaySeconds: 0.75, condition: null },
              { kind: "FireCommand", command: "animation_done", delaySeconds: 1, condition: null },
            ],
          }],
        },
        {
          logicalIdentity: "scripts/override.txt",
          revision: "override-1",
          sequences: [{
            name: "Move",
            condition: null,
            commands: [{ kind: "Animate", panel: "Target", variable: "XPos", target: "200", interpolator: "Linear", parameter: 0, delaySeconds: 0, durationSeconds: 1, condition: null }],
          }],
        },
      ],
    }
    const { runtime, requests } = setup(animations)
    const target = operation(runtime, {
      kind: "create-panel", parent: 1, control: "Panel", name: "Target",
      properties: [{ name: "xpos", value: "0" }, { name: "ypos", value: "0" }, { name: "wide", value: "20" }, { name: "tall", value: "20" }],
    }).panel!
    operation(runtime, { kind: "start-animation-sequence", parent: 1, sequence: "Move", cancelable: true })
    operation(runtime, { kind: "frame", timeSeconds: 0.5 })
    expect(runtime.snapshot().panels.find((panel) => panel.id === target)?.bounds.x).toBe(50)
    operation(runtime, { kind: "frame", timeSeconds: 0.75 })
    expect(runtime.snapshot().panels.find((panel) => panel.id === target)?.visible).toBeFalse()
    operation(runtime, { kind: "frame", timeSeconds: 1 })
    expect(runtime.snapshot().panels.find((panel) => panel.id === target)?.bounds.x).toBe(100)
    expect(requests.some((request) => request.kind === "command" && request.command === "animation_done")).toBeTrue()

    operation(runtime, { kind: "set-panel-state", panel: target, visible: true })
    operation(runtime, { kind: "set-bounds", panel: target, bounds: { x: 0, y: 0, width: 20, height: 20 } })
    operation(runtime, { kind: "set-reduced-motion", reduced: true })
    operation(runtime, { kind: "start-animation-sequence", parent: 1, sequence: "Move", cancelable: true })
    operation(runtime, { kind: "frame", timeSeconds: 1.1 })
    expect(runtime.snapshot().panels.find((panel) => panel.id === target)?.bounds.x).toBe(100)
    expect(runtime.snapshot().activeAnimations).toBe(1)
    operation(runtime, { kind: "set-viewport", viewport: { width: 1280, height: 960, devicePixelRatio: 2 } })
    expect(runtime.snapshot().activeAnimations).toBe(0)
    expect(runtime.snapshot().viewport).toEqual({ width: 1280, height: 960, devicePixelRatio: 2 })
  })

  test("covers frame, modal query, URL, scrollbar, list and property-page composite state", () => {
    const { runtime, requests } = setup()
    const frame = operation(runtime, {
      kind: "create-panel", parent: 1, control: "Frame", name: "Options",
      properties: [{ name: "xpos", value: "100" }, { name: "ypos", value: "100" }, { name: "wide", value: "240" }, { name: "tall", value: "180" }, { name: "title", value: "Options" }],
    }).panel!
    operation(runtime, { kind: "set-panel-state", panel: frame, visible: true })
    operation(runtime, { kind: "pointer-press", button: "left", x: 150, y: 110, pointerId: 9, clicks: 1 })
    operation(runtime, { kind: "frame", timeSeconds: 0 })
    expect(runtime.snapshot().input.capture).toBe(frame)
    operation(runtime, { kind: "pointer-move", x: 200, y: 140, pointerId: 9 })
    operation(runtime, { kind: "frame", timeSeconds: 0.01 })
    expect(runtime.snapshot().panels.find((panel) => panel.id === frame)?.bounds).toEqual({ x: 150, y: 130, width: 240, height: 180 })
    operation(runtime, { kind: "pointer-release", button: "left", x: 200, y: 140, pointerId: 9 })
    operation(runtime, { kind: "frame", timeSeconds: 0.02 })
    expect(runtime.snapshot().input.capture).toBeNull()

    const query = operation(runtime, {
      kind: "create-panel", parent: 1, control: "QueryBox", name: "Confirm",
      properties: [{ name: "xpos", value: "200" }, { name: "ypos", value: "150" }, { name: "wide", value: "220" }, { name: "tall", value: "120" }, { name: "title", value: "Disconnect?" }, { name: "cancelcommand", value: "cancel_disconnect" }],
    }).panel!
    operation(runtime, { kind: "set-panel-state", panel: query, visible: true })
    operation(runtime, { kind: "set-application-modal", panel: query })
    operation(runtime, { kind: "request-focus", panel: query })
    operation(runtime, { kind: "frame", timeSeconds: 0.03 })
    operation(runtime, { kind: "key-typed", key: "Escape", shift: false, control: false, alt: false, meta: false })
    operation(runtime, { kind: "frame", timeSeconds: 0.04 })
    expect(runtime.snapshot().panels.find((panel) => panel.id === query)?.visible).toBeFalse()
    expect(runtime.snapshot().input.applicationModal).toBeNull()
    expect(runtime.snapshot().input.keyFocus).toBe(frame)
    expect(requests.some((request) => request.kind === "command" && request.command === "cancel_disconnect")).toBeTrue()

    const url = operation(runtime, {
      kind: "create-panel", parent: 1, control: "URLLabel", name: "Help",
      properties: [{ name: "xpos", value: "20" }, { name: "ypos", value: "220" }, { name: "wide", value: "160" }, { name: "tall", value: "24" }, { name: "labelText", value: "Help" }, { name: "URLText", value: "#URL" }],
    }).panel!
    operation(runtime, { kind: "pointer-press", button: "left", x: 30, y: 230, pointerId: 1, clicks: 1 })
    operation(runtime, { kind: "frame", timeSeconds: 0.05 })
    expect(requests.some((request) => request.kind === "external-open" && request.panel === url && request.url === "https://example.test/tf2")).toBeTrue()

    const scroll = operation(runtime, {
      kind: "create-panel", parent: 1, control: "ScrollBar_Vertical", name: "Scroll",
      properties: [{ name: "xpos", value: "500" }, { name: "ypos", value: "20" }, { name: "wide", value: "20" }, { name: "tall", value: "100" }, { name: "rangeMin", value: "0" }, { name: "rangeMax", value: "100" }, { name: "rangeWindow", value: "20" }, { name: "value", value: "0" }],
    }).panel!
    operation(runtime, { kind: "pointer-press", button: "left", x: 510, y: 90, pointerId: 2, clicks: 1 })
    operation(runtime, { kind: "frame", timeSeconds: 0.06 })
    expect(runtime.snapshot().panels.find((panel) => panel.id === scroll)?.state.value).toBe(20)

    const list = operation(runtime, {
      kind: "create-panel", parent: 1, control: "ListPanel", name: "Servers",
      properties: [{ name: "xpos", value: "20" }, { name: "ypos", value: "270" }, { name: "wide", value: "200" }, { name: "tall", value: "80" }, { name: "linespacing", value: "20" }],
    }).panel!
    operation(runtime, { kind: "mutate-control", panel: list, mutation: { items: [{ id: 10, text: "One" }, { id: 11, text: "Two" }] } })
    operation(runtime, { kind: "pointer-press", button: "left", x: 30, y: 295, pointerId: 3, clicks: 1 })
    operation(runtime, { kind: "frame", timeSeconds: 0.07 })
    expect(runtime.snapshot().panels.find((panel) => panel.id === list)?.state.activeIndex).toBe(1)

    const sheet = operation(runtime, { kind: "create-panel", parent: 1, control: "PropertySheet", name: "Sheet" }).panel!
    const pageA = operation(runtime, { kind: "create-panel", parent: sheet, control: "PropertyPage", name: "Video" }).panel!
    const pageB = operation(runtime, { kind: "create-panel", parent: sheet, control: "PropertyPage", name: "Audio" }).panel!
    operation(runtime, { kind: "mutate-control", panel: sheet, mutation: { items: [{ id: pageA, text: "Video" }, { id: pageB, text: "Audio" }] } })
    expect(runtime.snapshot().panels.find((panel) => panel.id === pageA)?.visible).toBeTrue()
    expect(runtime.snapshot().panels.find((panel) => panel.id === pageB)?.visible).toBeFalse()
    operation(runtime, { kind: "mutate-control", panel: sheet, mutation: { activeIndex: 1 } })
    expect(runtime.snapshot().panels.find((panel) => panel.id === pageA)?.visible).toBeFalse()
    expect(runtime.snapshot().panels.find((panel) => panel.id === pageB)?.visible).toBeTrue()
    expect(runtime.snapshot().panels.find((panel) => panel.id === sheet)?.text).toBe("")
  })

  test("executes registered converters and all nonlinear interpolator families", () => {
    const interpolators = ["Linear", "Accel", "Deaccel", "Spline", "Pulse", "Flicker", "Bias", "Gain", "Bounce"] as const
    const sequences = interpolators.map((interpolator, index) => ({
      name: interpolator,
      condition: null,
      commands: [{
        kind: "Animate" as const,
        panel: "Custom",
        variable: "Level",
        target: "100",
        interpolator,
        parameter: interpolator === "Pulse" ? 1 : interpolator === "Flicker" ? 0.5 : 0.25,
        delaySeconds: 0,
        durationSeconds: 1,
        condition: null,
      }],
    }))
    const animations: VguiAnimationScriptSet = {
      identity: "scripts/converters",
      revision: "converters-1",
      activeConditions: [],
      scripts: [{
        logicalIdentity: "scripts/converters.txt",
        revision: "converters-script-1",
        sequences: [
          ...sequences,
          {
            name: "SetTyped",
            condition: null,
            commands: [
              { kind: "SetString", panel: "Custom", variable: "Caption", value: "ready", delaySeconds: 0, condition: null },
              { kind: "SetFont", panel: "Custom", variable: "DisplayFont", font: "Default", delaySeconds: 0, condition: null },
              { kind: "SetTexture", panel: "Custom", variable: "Icon", texture: "test/icon", delaySeconds: 0, condition: null },
              { kind: "Animate", panel: "Custom", variable: "Count", target: "3", interpolator: "Linear", parameter: 0, delaySeconds: 0, durationSeconds: 0, condition: null },
              { kind: "Animate", panel: "Custom", variable: "Tint", target: "10 20 30 40", interpolator: "Linear", parameter: 0, delaySeconds: 0, durationSeconds: 0, condition: null },
              { kind: "Animate", panel: "Custom", variable: "Enabled", target: "0", interpolator: "Linear", parameter: 0, delaySeconds: 0, durationSeconds: 0, condition: null },
              { kind: "Animate", panel: "Custom", variable: "Scaled", target: "20", interpolator: "Linear", parameter: 0, delaySeconds: 0, durationSeconds: 0, condition: null },
            ],
          },
        ],
      }],
    }
    const { runtime } = setup(animations, [{
      name: "CustomControl",
      baseControl: "Panel",
      element: "div",
      role: "status",
      focusable: false,
      acceptedProperties: [],
      animationVariables: [
        { name: "Level", converter: "float", defaultValue: "0" },
        { name: "Count", converter: "int", defaultValue: "0" },
        { name: "Tint", converter: "Color", defaultValue: "255 255 255 255" },
        { name: "Enabled", converter: "bool", defaultValue: "true" },
        { name: "Code", converter: "char", defaultValue: "x", maximumCodeUnits: 4 },
        { name: "Caption", converter: "string", defaultValue: "idle", maximumCodeUnits: 32 },
        { name: "DisplayFont", converter: "HFont", defaultValue: "Default" },
        { name: "QualifiedFont", converter: "vgui::HFont", defaultValue: "Default" },
        { name: "ScaledFloat", converter: "proportional_float", defaultValue: "2.5" },
        { name: "Scaled", converter: "proportional_int", defaultValue: "10" },
        { name: "ScaledX", converter: "proportional_xpos", defaultValue: "12" },
        { name: "ScaledY", converter: "proportional_ypos", defaultValue: "13" },
        { name: "ScaledWidth", converter: "proportional_width", defaultValue: "14" },
        { name: "ScaledHeight", converter: "proportional_height", defaultValue: "15" },
        { name: "Icon", converter: "textureid", defaultValue: "test/icon" },
      ],
    }])
    const custom = operation(runtime, { kind: "create-panel", parent: 1, control: "CustomControl", name: "Custom" }).panel!
    operation(runtime, { kind: "start-animation-sequence", parent: 1, sequence: "SetTyped", cancelable: true })
    operation(runtime, { kind: "frame", timeSeconds: 0 })
    let values = runtime.snapshot().panels.find((panel) => panel.id === custom)!.animationVariables
    expect(values.Caption).toBe("ready")
    expect(values.DisplayFont).toBe("Default")
    expect(values.Icon).toBe("test/icon")
    expect(values.Count).toBe(3)
    expect(values.Tint).toEqual([10, 20, 30, 40])
    expect(values.Enabled).toBeFalse()
    expect(values.Scaled).toBe(20)
    expect(values.ScaledFloat).toBe(2)
    expect(values.ScaledX).toBe(12)
    expect(values.ScaledY).toBe(13)
    expect(values.ScaledWidth).toBe(14)
    expect(values.ScaledHeight).toBe(15)

    operation(runtime, { kind: "delete-panel", panel: custom, deferred: false })
    const samples: Record<string, number> = {}
    for (let index = 0; index < interpolators.length; index += 1) {
      const name = interpolators[index]
      const samplePanel = operation(runtime, { kind: "create-panel", parent: 1, control: "CustomControl", name: "Custom" }).panel!
      operation(runtime, { kind: "frame", timeSeconds: index * 2 })
      operation(runtime, { kind: "start-animation-sequence", parent: 1, sequence: name, cancelable: true })
      operation(runtime, { kind: "frame", timeSeconds: index * 2 + 0.5 })
      values = runtime.snapshot().panels.find((panel) => panel.id === samplePanel)!.animationVariables
      samples[name] = values.Level as number
      operation(runtime, { kind: "frame", timeSeconds: index * 2 + 1 })
      operation(runtime, { kind: "delete-panel", panel: samplePanel, deferred: false })
    }
    expect(samples.Linear).toBe(50)
    expect(samples.Accel).toBe(25)
    expect(samples.Deaccel).toBeCloseTo(Math.sqrt(0.5) * 100)
    expect(samples.Spline).toBe(50)
    expect(samples.Flicker).toBe(100)
    expect(Number.isFinite(samples.Bias)).toBeTrue()
    expect(Number.isFinite(samples.Gain)).toBeTrue()
    expect(Number.isFinite(samples.Bounce)).toBeTrue()
  })

  test("executes all delayed HUD-animation command forms with scoped cancellation", () => {
    const animations: VguiAnimationScriptSet = {
      identity: "scripts/commands",
      revision: "commands-1",
      activeConditions: [],
      scripts: [{
        logicalIdentity: "scripts/commands.txt",
        revision: "commands-script-1",
        sequences: [
          { name: "Long", condition: null, commands: [
            { kind: "Animate", panel: "Parent", variable: "alpha", target: "0", interpolator: "Linear", parameter: 0, delaySeconds: 0, durationSeconds: 10, condition: null },
            { kind: "Animate", panel: "Parent", variable: "XPos", target: "100", interpolator: "Linear", parameter: 0, delaySeconds: 0, durationSeconds: 10, condition: null },
          ] },
          { name: "Never", condition: null, commands: [{ kind: "SetVisible", panel: "Child", visible: true, delaySeconds: 5, condition: null }] },
          { name: "Nested", condition: null, commands: [{ kind: "PlaySound", sound: "ui/test.wav", delaySeconds: 0, condition: null }] },
          { name: "ChildWork", condition: null, commands: [{ kind: "SetVisible", panel: "Child", visible: false, delaySeconds: 0, condition: null }] },
          { name: "Commands", condition: null, commands: [
            { kind: "RunEvent", sequence: "Nested", delaySeconds: 0, condition: null },
            { kind: "RunEventChild", child: "Parent", sequence: "ChildWork", delaySeconds: 0, condition: null },
            { kind: "StopEvent", sequence: "Never", delaySeconds: 0, condition: null },
            { kind: "StopAnimation", panel: "Parent", variable: "alpha", delaySeconds: 0, condition: null },
            { kind: "StopPanelAnimations", panel: "Parent", delaySeconds: 0, condition: null },
            { kind: "SetInputEnabled", panel: "Parent", enabled: false, delaySeconds: 0, condition: null },
          ] },
        ],
      }],
    }
    const { runtime, requests } = setup(animations)
    const parent = operation(runtime, { kind: "create-panel", parent: 1, control: "Panel", name: "Parent", properties: [{ name: "wide", value: "100" }, { name: "tall", value: "100" }] }).panel!
    const child = operation(runtime, { kind: "create-panel", parent, control: "Panel", name: "Child", properties: [{ name: "wide", value: "20" }, { name: "tall", value: "20" }] }).panel!
    operation(runtime, { kind: "start-animation-sequence", parent: 1, sequence: "Long", cancelable: true })
    operation(runtime, { kind: "start-animation-sequence", parent: 1, sequence: "Never", cancelable: true })
    operation(runtime, { kind: "start-animation-sequence", parent: 1, sequence: "Commands", cancelable: true })
    operation(runtime, { kind: "frame", timeSeconds: 0 })
    const snapshot = runtime.snapshot()
    expect(snapshot.activeAnimations).toBe(0)
    expect(snapshot.delayedCommands).toBe(0)
    expect(snapshot.panels.find((panel) => panel.id === child)?.visible).toBeFalse()
    expect(snapshot.panels.find((panel) => panel.id === parent)?.mouseInput).toBeFalse()
    expect(snapshot.panels.find((panel) => panel.id === parent)?.keyboardInput).toBeFalse()
    expect(requests).toContainEqual({ kind: "sound", panel: 1, logicalIdentity: "ui/test.wav" })
  })

  test("enforces hierarchy, message, modal, clipboard and deferred-cleanup boundaries", () => {
    const { runtime, requests } = setup()
    const container = operation(runtime, { kind: "create-panel", parent: 1, control: "EditablePanel", name: "Container", properties: [{ name: "wide", value: "200" }, { name: "tall", value: "200" }] }).panel!
    const entry = operation(runtime, { kind: "create-panel", parent: container, control: "TextEntry", name: "Clipboard", properties: [{ name: "wide", value: "100" }, { name: "tall", value: "24" }] }).panel!
    const cycle = runtime.apply({ kind: "reparent-panel", panel: container, parent: entry })
    expect(cycle.ok).toBeFalse()
    if (!cycle.ok) expect(cycle.diagnostic.code).toBe("HierarchyCycle")

    operation(runtime, { kind: "request-focus", panel: entry })
    operation(runtime, { kind: "frame", timeSeconds: 0 })
    operation(runtime, { kind: "key-press", key: "v", shift: false, control: true, alt: false, meta: false, repeat: false })
    operation(runtime, { kind: "frame", timeSeconds: 0.01 })
    const clipboard = requests.find((request): request is Extract<VguiRequest, { kind: "clipboard-read" }> => request.kind === "clipboard-read")
    expect(clipboard?.panel).toBe(entry)
    operation(runtime, { kind: "clipboard-result", requestId: clipboard!.requestId, result: "success", text: "copied" })
    expect(runtime.snapshot().panels.find((panel) => panel.id === entry)?.text).toBe("copied")

    operation(runtime, { kind: "set-modal-subtree", panel: container, restrictToSubtree: true, outsideClickListener: 1 })
    operation(runtime, { kind: "pointer-press", button: "left", x: 400, y: 400, pointerId: 3, clicks: 1 })
    operation(runtime, { kind: "frame", timeSeconds: 0.02 })
    expect(requests.some((request) => request.kind === "message" && request.message.name === "UnhandledMouseClick")).toBeTrue()

    operation(runtime, { kind: "post-message", target: entry, source: container, message: { name: "Late", fields: { value: 1 } }, delaySeconds: 1 })
    operation(runtime, { kind: "delete-panel", panel: entry, deferred: true })
    expect(runtime.snapshot().queuedMessages).toBe(1)
    operation(runtime, { kind: "frame", timeSeconds: 0.03 })
    expect(runtime.snapshot().panels.some((panel) => panel.id === entry)).toBeFalse()
    expect(runtime.snapshot().queuedMessages).toBe(0)
    expect(runtime.snapshot().input.keyFocus).toBeNull()
  })

  test("resolves all HUD-animation relative alignment identities", () => {
    const alignments = [
      ["nw", 10, 20], ["n", 20, 20], ["ne", 40, 20],
      ["w", 10, 30], ["c", 20, 30], ["e", 40, 30],
      ["sw", 10, 60], ["s", 20, 60], ["se", 40, 60],
    ] as const
    const animations: VguiAnimationScriptSet = {
      identity: "scripts/alignments",
      revision: "alignments-1",
      activeConditions: [],
      scripts: [{
        logicalIdentity: "scripts/alignments.txt",
        revision: "alignments-script-1",
        sequences: alignments.map(([alignment]) => ({
          name: alignment,
          condition: null,
          commands: [{ kind: "Animate", panel: "Moving", variable: "Position", target: "0 0", interpolator: "Linear", parameter: 0, delaySeconds: 0, durationSeconds: 0, relative: { panel: "Anchor", alignment }, condition: null }],
        })),
      }],
    }
    const { runtime } = setup(animations)
    operation(runtime, { kind: "create-panel", parent: 1, control: "Panel", name: "Anchor", properties: [{ name: "xpos", value: "10" }, { name: "ypos", value: "20" }, { name: "wide", value: "30" }, { name: "tall", value: "40" }] })
    const moving = operation(runtime, { kind: "create-panel", parent: 1, control: "Panel", name: "Moving", properties: [{ name: "wide", value: "5" }, { name: "tall", value: "5" }] }).panel!
    for (let index = 0; index < alignments.length; index += 1) {
      const [alignment, x, y] = alignments[index]
      operation(runtime, { kind: "set-bounds", panel: moving, bounds: { x: 0, y: 0, width: 5, height: 5 } })
      operation(runtime, { kind: "start-animation-sequence", parent: 1, sequence: alignment, cancelable: true })
      operation(runtime, { kind: "frame", timeSeconds: index })
      expect(runtime.snapshot().panels.find((panel) => panel.id === moving)?.bounds).toEqual({ x, y, width: 5, height: 5 })
    }
  })

  test("applies parent-relative geometry and the established auto-resize pin bug", () => {
    const { runtime } = setup()
    const parent = operation(runtime, { kind: "create-panel", parent: 1, control: "EditablePanel", name: "ResizeParent", properties: [{ name: "wide", value: "200" }, { name: "tall", value: "200" }] }).panel!
    const stretch = operation(runtime, {
      kind: "create-panel", parent, control: "Panel", name: "Stretch",
      properties: [{ name: "xpos", value: "10" }, { name: "ypos", value: "10" }, { name: "wide", value: "50" }, { name: "tall", value: "20" }, { name: "AutoResize", value: "3" }, { name: "PinCorner", value: "0" }],
    }).panel!
    const pinned = operation(runtime, {
      kind: "create-panel", parent, control: "Panel", name: "Pinned",
      properties: [{ name: "xpos", value: "150" }, { name: "ypos", value: "160" }, { name: "wide", value: "40" }, { name: "tall", value: "30" }, { name: "AutoResize", value: "0" }, { name: "PinCorner", value: "3" }],
    }).panel!
    const centered = operation(runtime, {
      kind: "create-panel", parent, control: "Panel", name: "Centered",
      properties: [{ name: "proportionalToParent", value: "1" }, { name: "xpos", value: "c0" }, { name: "ypos", value: "c0" }, { name: "wide", value: "10" }, { name: "tall", value: "10" }],
    }).panel!
    expect(runtime.snapshot().panels.find((panel) => panel.id === centered)?.bounds).toEqual({ x: 100, y: 100, width: 10, height: 10 })
    operation(runtime, { kind: "set-bounds", panel: parent, bounds: { x: 0, y: 0, width: 300, height: 300 } })
    const snapshot = runtime.snapshot()
    expect(snapshot.panels.find((panel) => panel.id === stretch)?.bounds).toEqual({ x: 10, y: 10, width: 150, height: 120 })
    expect(snapshot.panels.find((panel) => panel.id === pinned)?.bounds).toEqual({ x: 250, y: 260, width: 40, height: 30 })
  })

  test("inherits proportional state and reapplies screen-relative resource geometry on resize", () => {
    const { runtime } = setup()
    operation(runtime, { kind: "set-panel-state", panel: 1, proportional: true })
    const hud = operation(runtime, { kind: "create-panel", parent: 1, control: "EditablePanel", name: "HudAmmo" }).panel!
    operation(runtime, {
      kind: "replace-resource",
      parent: 1,
      document: {
        logicalIdentity: "resource/hud-layout.res",
        revision: "hud-layout-1",
        root: object("HudLayout", [object("HudAmmo", [
          scalar("fieldName", "HudAmmo"),
          scalar("xpos", "r131"), scalar("ypos", "r77"), scalar("wide", "94"), scalar("tall", "45"),
        ])]),
      },
      selection: { activeConditions: [], resolutionSuffixes: [] },
    })
    const child = operation(runtime, {
      kind: "create-panel", parent: hud, control: "Panel", name: "Child",
      properties: [
        { name: "xpos", value: "75" }, { name: "ypos", value: "35" },
        { name: "wide", value: "51" }, { name: "tall", value: "51" },
      ],
    }).panel!
    let snapshot = runtime.snapshot()
    expect(snapshot.panels.find((panel) => panel.id === hud)).toMatchObject({
      proportional: true,
      bounds: { x: 509, y: 403, width: 94, height: 45 },
    })
    expect(snapshot.panels.find((panel) => panel.id === child)).toMatchObject({
      proportional: true,
      bounds: { x: 75, y: 35, width: 51, height: 51 },
    })

    operation(runtime, { kind: "set-viewport", viewport: { width: 1280, height: 720, devicePixelRatio: 1 } })
    snapshot = runtime.snapshot()
    expect(snapshot.panels.find((panel) => panel.id === hud)?.bounds).toEqual({ x: 1084, y: 605, width: 141, height: 67 })
    expect(snapshot.panels.find((panel) => panel.id === child)?.bounds).toEqual({ x: 112, y: 52, width: 76, height: 76 })

    operation(runtime, { kind: "set-panel-state", panel: hud, proportional: false })
    snapshot = runtime.snapshot()
    expect(snapshot.panels.find((panel) => panel.id === hud)?.proportional).toBeFalse()
    expect(snapshot.panels.find((panel) => panel.id === child)?.proportional).toBeFalse()
    expect(snapshot.panels.find((panel) => panel.id === 1)?.proportional).toBeTrue()
  })

  test("reselects supplied Source font metrics only when viewport height changes", () => {
    const { root, runtime } = setup()
    const heights: number[] = []
    operation(runtime, {
      kind: "replace-scheme",
      scheme: {
        ...scheme,
        fonts: [{
          ...scheme.fonts[0]!,
          metricsForViewport(height) {
            heights.push(height)
            return { sizePx: height < 768 ? 32 : 44, lineHeightPx: height < 768 ? 34 : 46 }
          },
        }],
      },
    })
    operation(runtime, {
      kind: "create-panel", parent: 1, control: "Label", name: "ViewportFont",
      properties: [{ name: "font", value: "Default" }, { name: "labelText", value: "200" }],
    })
    const label = descendants(root).find((element) => element.dataset.vguiName === "ViewportFont")!
    expect(label.style.fontSize).toBe("16px")
    operation(runtime, { kind: "set-viewport", viewport: { width: 1024, height: 720, devicePixelRatio: 1 } })
    expect(label.style.fontSize).toBe("32px")
    expect(label.style.lineHeight).toBe("34px")
    operation(runtime, { kind: "set-viewport", viewport: { width: 1280, height: 720, devicePixelRatio: 2 } })
    expect(heights).toEqual([720])
    operation(runtime, { kind: "set-viewport", viewport: { width: 1024, height: 768, devicePixelRatio: 1 } })
    expect(label.style.fontSize).toBe("44px")
    expect(label.style.lineHeight).toBe("46px")
    expect(heights).toEqual([720, 768])
  })

  test("rejects malformed terminal colors and relevant base-setting cycles atomically", () => {
    const { runtime } = setup()
    const initial = runtime.snapshot().schemeIdentity
    const malformed: VguiScheme = {
      ...scheme,
      identity: "resource/malformed-scheme.res",
      revision: "malformed-1",
      colors: scheme.colors.map((entry) => entry.name === "Panel.BgColor" ? { ...entry, value: "AnotherColor" } : entry),
    }
    const malformedResult = runtime.apply({ kind: "replace-scheme", scheme: malformed })
    expect(malformedResult.ok).toBeFalse()
    if (!malformedResult.ok) expect(malformedResult.diagnostic.code).toBe("MalformedScheme")
    expect(runtime.snapshot().schemeIdentity).toBe(initial)

    const cycle: VguiScheme = {
      ...scheme,
      identity: "resource/cycle-scheme.res",
      revision: "cycle-1",
      colors: scheme.colors.filter((entry) => entry.name !== "Panel.BgColor"),
      settings: [{ name: "Panel.BgColor", value: "CycleA" }, { name: "CycleA", value: "CycleB" }, { name: "CycleB", value: "CycleA" }],
    }
    const cycleResult = runtime.apply({ kind: "replace-scheme", scheme: cycle })
    expect(cycleResult.ok).toBeFalse()
    if (!cycleResult.ok) expect(cycleResult.diagnostic.code).toBe("MalformedScheme")
    expect(runtime.snapshot().schemeIdentity).toBe(initial)
  })

  test("reuses a code-created custom panel without consulting its resource ControlName", () => {
    const { runtime } = setup(emptyAnimations, [{
      name: "TfCustomPanel",
      baseControl: "EditablePanel",
      element: "div",
      role: "region",
      focusable: false,
      animationVariables: [],
      acceptedProperties: ["game_property"],
    }])
    const custom = operation(runtime, { kind: "create-panel", parent: 1, control: "TfCustomPanel", name: "CustomPanel" }).panel!
    operation(runtime, {
      kind: "replace-resource",
      parent: 1,
      document: {
        logicalIdentity: "resource/custom.res",
        revision: "custom-1",
        root: object("Custom", [object("CustomPanel", [scalar("ControlName", "Frame"), scalar("game_property", "handled")])]),
      },
      selection: { activeConditions: [], resolutionSuffixes: [] },
    })
    const panel = runtime.snapshot().panels.find((candidate) => candidate.id === custom)
    expect(panel?.control).toBe("TfCustomPanel")
    expect(panel?.resourceOwner).toBeNull()
  })

  test("implements every configured extended generic control without aliases", () => {
    const { root, runtime } = setup()
    const continuous = operation(runtime, { kind: "create-panel", parent: 1, control: "ContinuousProgressBar", name: "Continuous" }).panel!
    operation(runtime, { kind: "mutate-control", panel: continuous, mutation: { progress: 0.75, previousProgress: 0.5 } })
    const divider = operation(runtime, { kind: "create-panel", parent: 1, control: "Divider", name: "Divider" }).panel!
    const system = operation(runtime, { kind: "create-panel", parent: 1, control: "FrameSystemButton", name: "frame_menu" }).panel!
    const html = operation(runtime, { kind: "create-panel", parent: 1, control: "HTML", name: "Banner" }).panel!
    const scalable = operation(runtime, { kind: "create-panel", parent: 1, control: "ScalableImagePanel", name: "Scalable" }).panel!
    const scrollable = operation(runtime, { kind: "create-panel", parent: 1, control: "ScrollableEditablePanel", name: "Scroller" }).panel!
    const sectioned = operation(runtime, { kind: "create-panel", parent: 1, control: "SectionedListPanel", name: "Bindings" }).panel!
    operation(runtime, {
      kind: "mutate-control",
      panel: sectioned,
      mutation: {
        sections: [{ id: 1, name: "Movement", alwaysVisible: true, minimumHeight: 0, columns: [
          { name: "action", text: "Action", flags: 0, width: 180 },
          { name: "binding", text: "Key", flags: 2, width: 80 },
        ] }],
        sectionedItems: [{ id: 7, section: 1, cells: { action: "Move forward", binding: "W" }, enabled: true }],
      },
    })
    const snapshot = runtime.snapshot()
    expect(snapshot.panels.find((panel) => panel.id === continuous)?.state).toMatchObject({ progress: 0.75, previousProgress: 0.5 })
    expect(snapshot.panels.find((panel) => panel.id === divider)?.bounds).toMatchObject({ width: 128, height: 2 })
    expect(snapshot.panels.find((panel) => panel.id === system)).toMatchObject({ control: "FrameSystemButton", role: "button", enabled: false })
    expect(snapshot.panels.find((panel) => panel.id === html)).toMatchObject({ control: "HTML", role: "document" })
    expect(snapshot.panels.find((panel) => panel.id === scalable)).toMatchObject({ control: "ScalableImagePanel", role: "img" })
    expect(snapshot.panels.find((panel) => panel.parent === scrollable && panel.name === "VerticalScrollBar")?.control).toBe("ScrollBar_Vertical")
    expect(snapshot.panels.find((panel) => panel.parent === sectioned && panel.name === "SectionedScrollBar")?.control).toBe("ScrollBar_Vertical")
    expect(snapshot.panels.find((panel) => panel.id === sectioned)?.state.sectionedItems[0]).toEqual({ id: 7, section: 1, cells: { action: "Move forward", binding: "W" }, enabled: true })
    operation(runtime, { kind: "frame", timeSeconds: 0.01 })
    operation(runtime, { kind: "frame", timeSeconds: 0.02 })
    expect(descendants(root).some((node) => node.dataset.vguiItem === "7")).toBeTrue()
  })

  test("applies Source CExLabel resource foreground colors after its inherited Label scheme", () => {
    const { root, runtime } = setup(emptyAnimations, [{
      name: "CExLabel",
      baseControl: "Label",
      element: "div",
      role: null,
      focusable: false,
      animationVariables: [],
      acceptedProperties: ["fgcolor"],
    }])
    for (const [name, color, override] of [
      ["Named", "Button.BgColor", null],
      ["Literal", "10 20 30 128", null],
      ["Missing", "MissingColor", null],
      ["Override", "Button.BgColor", "1 2 3 255"],
    ] as const) {
      operation(runtime, {
        kind: "create-panel",
        parent: 1,
        control: "CExLabel",
        name,
        properties: [
          { name: "fgcolor", value: color },
          ...(override === null ? [] : [{ name: "fgcolor_override", value: override }]),
        ],
      })
    }
    const color = (name: string) => descendants(root).find((element) => element.dataset.vguiName === name)!.style.color
    expect(color("Named")).toBe("rgba(50, 60, 70, 1)")
    expect(color("Literal")).toBe("rgba(10, 20, 30, 0.5019607843137255)")
    expect(color("Missing")).toBe("rgba(0, 255, 0, 1)")
    expect(color("Override")).toBe("rgba(1, 2, 3, 1)")
  })

  test("paints authored equal-z foreground labels above their later offset shadows", () => {
    const { root, runtime } = setup()
    operation(runtime, {
      kind: "replace-resource",
      parent: 1,
      document: {
        logicalIdentity: "resource/ui/hudammoweapons.res",
        revision: "configured-order",
        root: object("Resource", [
          object("AmmoInClip", [scalar("ControlName", "Label"), scalar("zpos", "5"), scalar("xpos", "20"), scalar("ypos", "20")]),
          object("AmmoInClipShadow", [scalar("ControlName", "Label"), scalar("zpos", "5"), scalar("xpos", "21"), scalar("ypos", "21")]),
          object("AmmoInReserve", [scalar("ControlName", "Label"), scalar("zpos", "7"), scalar("xpos", "60"), scalar("ypos", "20")]),
          object("AmmoInReserveShadow", [scalar("ControlName", "Label"), scalar("zpos", "7"), scalar("xpos", "61"), scalar("ypos", "21")]),
        ]),
      },
      selection: { activeConditions: [], resolutionSuffixes: [] },
    })
    const names = descendants(root).map((element) => element.dataset.vguiName).filter(Boolean)
    expect(names.indexOf("AmmoInClipShadow")).toBeLessThan(names.indexOf("AmmoInClip"))
    expect(names.indexOf("AmmoInReserveShadow")).toBeLessThan(names.indexOf("AmmoInReserve"))
  })

  test("executes an explicitly registered custom control through its Source base control", () => {
    const { runtime, requests } = setup(emptyAnimations, [{
      name: "CExButton",
      baseControl: "Button",
      element: "button",
      role: "button",
      focusable: true,
      animationVariables: [],
      acceptedProperties: [],
    }])
    operation(runtime, {
      kind: "create-panel",
      parent: 1,
      control: "CExButton",
      name: "Options",
      properties: [{ name: "command", value: "OpenOptionsDialog" }],
    })
    operation(runtime, { kind: "pointer-press", button: "left", x: 1, y: 1, pointerId: 1, clicks: 1 })
    operation(runtime, { kind: "pointer-release", button: "left", x: 1, y: 1, pointerId: 1 })
    operation(runtime, { kind: "frame", timeSeconds: 0 })
    expect(requests.some((request) => request.kind === "command" && request.command === "OpenOptionsDialog")).toBeTrue()
  })
})
