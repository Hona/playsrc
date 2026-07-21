import { describe, expect, test } from "bun:test"
import { initializeVguiRuntime, type VguiResourceNode, type VguiRuntimeLimits, type VguiScheme } from "../../src"
import { FakeDocument, createRoot } from "../fake-dom"

const limits: VguiRuntimeLimits = Object.freeze({
  maxPanels: 32,
  maxHierarchyDepth: 8,
  maxChildrenPerPanel: 16,
  maxResourceNodes: 64,
  maxResourceDepth: 8,
  maxPropertiesPerPanel: 16,
  maxStringCodeUnits: 255,
  maxTextCodeUnits: 255,
  maxDialogVariables: 8,
  maxLocalizationTokens: 8,
  maxSchemeColors: 8,
  maxSchemeSettings: 8,
  maxSchemeBorders: 8,
  maxSchemeImages: 8,
  maxAnimationScripts: 1,
  maxAnimationSequences: 1,
  maxAnimationCommands: 1,
  maxActiveAnimations: 1,
  maxDelayedCommands: 1,
  maxQueuedMessages: 8,
  maxDiagnostics: 16,
  maxDomNodes: 64,
  maxListeners: 16,
})

const scheme: VguiScheme = Object.freeze({
  identity: "resource/client-scheme.res",
  revision: "vertical-layout-1",
  tag: "ClientScheme",
  colors: Object.freeze([]),
  settings: Object.freeze([]),
  fonts: Object.freeze([]),
  borders: Object.freeze([]),
  images: Object.freeze([]),
})

const scalar = (name: string, value: string): VguiResourceNode => Object.freeze({ name, value, condition: null, children: Object.freeze([]) })
const object = (name: string, children: readonly VguiResourceNode[]): VguiResourceNode => Object.freeze({ name, value: null, condition: null, children })

describe("VGUI vertical viewport symptom loop", () => {
  test("distinguishes screen-relative and explicit parent-relative resource geometry", () => {
    const root = createRoot(new FakeDocument())
    const initialized = initializeVguiRuntime({
      runtimeIdentity: "vertical-layout-loop",
      root: root as unknown as HTMLElement,
      rootControl: { control: "EditablePanel", name: "HudViewport" },
      viewport: { width: 1280, height: 720, devicePixelRatio: 1 },
      limits,
      clock: { nowSeconds: () => 0 },
      random: { nextUnit: () => 0.5 },
      scheme,
      localization: { identity: "resource/tf_english.txt", revision: "vertical-layout-1", language: "english", tokens: Object.freeze([]) },
      animationScripts: { identity: "scripts/hudanimations-manifest.txt", revision: "vertical-layout-1", scripts: Object.freeze([]), activeConditions: Object.freeze([]) },
      customControls: Object.freeze([]),
      reducedMotion: true,
      onRequest() {},
    })
    if (!initialized.ok) throw new Error(initialized.diagnostic.code)
    const runtime = initialized.runtime
    const proportional = runtime.apply({ kind: "set-panel-state", panel: 1, proportional: true })
    if (!proportional.ok) throw new Error(proportional.diagnostic.code)
    const create = runtime.apply({
      kind: "create-panel",
      parent: 1,
      control: "EditablePanel",
      name: "BottomHudParent",
      properties: [
        { name: "xpos", value: "20" },
        { name: "ypos", value: "100" },
        { name: "wide", value: "400" },
        { name: "tall", value: "300" },
      ],
    })
    if (!create.ok || create.panel === undefined) throw new Error("parent creation failed")
    const parent = create.panel
    const replaced = runtime.apply({
      kind: "replace-resource",
      parent,
      document: {
        logicalIdentity: "resource/ui/hud-bottom-vector.res",
        revision: "vertical-layout-1",
        root: object("Resource", [object("ScreenBottomPanel", [
          scalar("ControlName", "Panel"),
          scalar("xpos", "r60"),
          scalar("ypos", "r40"),
          scalar("wide", "50"),
          scalar("tall", "30"),
        ]), object("ParentBottomPanel", [
          scalar("ControlName", "Panel"),
          scalar("proportionalToParent", "1"),
          scalar("xpos", "r60"),
          scalar("ypos", "r40"),
          scalar("wide", "50"),
          scalar("tall", "30"),
        ])]),
      },
      selection: { activeConditions: Object.freeze([]), resolutionSuffixes: Object.freeze([]) },
    })
    if (!replaced.ok) throw new Error(replaced.diagnostic.code)

    const screenPanel = () => runtime.snapshot().panels.find((candidate) => candidate.name === "ScreenBottomPanel")!
    const parentPanel = () => runtime.snapshot().panels.find((candidate) => candidate.name === "ParentBottomPanel")!
    expect(screenPanel().bounds).toEqual({ x: 1190, y: 660, width: 75, height: 45 })
    expect(parentPanel().bounds).toEqual({ x: 510, y: 390, width: 75, height: 45 })

    const resized = runtime.apply({ kind: "set-viewport", viewport: { width: 1024, height: 768, devicePixelRatio: 1 } })
    if (!resized.ok) throw new Error(resized.diagnostic.code)
    expect(screenPanel().bounds).toEqual({ x: 928, y: 704, width: 80, height: 48 })
    expect(parentPanel().bounds).toEqual({ x: 544, y: 416, width: 80, height: 48 })
  })
})
