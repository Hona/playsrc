import { consoleLimits, consoleResources } from "../../../../apps/web/tf2/src/console-resources"
import { tf2UiResources } from "../../../../games/tf2/browser/src/ui-resources/descriptor"
import {
  initializeDeveloperConsole,
  initializeVguiRuntime,
  type DeveloperConsole,
  type VguiOperation,
  type VguiRequest,
  type VguiResourceNode,
  type VguiRuntime,
  type VguiRuntimeLimits,
  type VguiScheme,
} from "../src"

const limits: VguiRuntimeLimits = Object.freeze({
  maxPanels: 128, maxHierarchyDepth: 16, maxChildrenPerPanel: 64, maxResourceNodes: 512, maxResourceDepth: 16,
  maxPropertiesPerPanel: 128, maxStringCodeUnits: 2_048, maxTextCodeUnits: 4_096, maxDialogVariables: 64,
  maxLocalizationTokens: 128, maxSchemeColors: 128, maxSchemeSettings: 128, maxSchemeBorders: 32, maxSchemeImages: 32,
  maxAnimationScripts: 8, maxAnimationSequences: 128, maxAnimationCommands: 512, maxActiveAnimations: 128,
  maxDelayedCommands: 128, maxQueuedMessages: 256, maxDiagnostics: 512, maxDomNodes: 256, maxListeners: 32,
})

const scheme: VguiScheme = Object.freeze({
  identity: "resource/clientscheme.res",
  revision: "1d071b99def0405cbf73d97642a396e6dcbad1a7488f12696ca5dd62893c604c",
  tag: "ClientScheme",
  colors: Object.freeze([
    { name: "Panel.FgColor", value: "235 226 202 255" },
    { name: "Panel.BgColor", value: "0 0 0 0" },
    { name: "Label.TextColor", value: "235 226 202 255" },
    { name: "Label.BgColor", value: "0 0 0 0" },
    { name: "TanLight", value: "235 226 202 255" },
    { name: "Black", value: "46 43 42 255" },
    { name: "TransparentBlack", value: "0 0 0 196" },
    { name: "Button.TextColor", value: "235 226 202 255" },
    { name: "Button.BgColor", value: "64 71 77 255" },
    { name: "Button.ArmedTextColor", value: "235 226 202 255" },
    { name: "Button.ArmedBgColor", value: "85 95 103 255" },
    { name: "Frame.BgColor", value: "48 56 64 255" },
    { name: "Frame.OutOfFocusBgColor", value: "40 45 50 255" },
  ]),
  settings: Object.freeze([]),
  fonts: Object.freeze([
    { name: "Default", cssFamily: '"TF2"', sizePx: 18, lineHeightPx: 20, weight: 500, style: "normal", available: true },
    { name: "HudFontGiantBold", cssFamily: '"TF2 Build"', sizePx: 44, lineHeightPx: 44, weight: 500, style: "normal", available: true },
    { name: "HudFontMediumSmall", cssFamily: '"TF2"', sizePx: 18, lineHeightPx: 18, weight: 500, style: "normal", available: true },
  ]),
  borders: Object.freeze([]),
  images: Object.freeze([]),
})

const clock = { nowSeconds: () => performance.now() / 1_000 }
const random = { nextUnit: () => 0.25 }
const localization = { identity: "resource/vgui_english.txt", revision: "configured-ordering", language: "english", tokens: [] }
const animationScripts = { identity: "scripts/ordering-empty", revision: "configured-ordering", scripts: [], activeConditions: [] }
const customControls = Object.freeze([{
  name: "CExLabel", baseControl: "Label" as const, element: "div" as const, role: null,
  focusable: false, animationVariables: [], acceptedProperties: ["fgcolor"],
}])

const workspace = document.getElementById("workspace")
const ammoMount = document.getElementById("ammo")
const optionsMount = document.getElementById("options")
const developerMount = document.getElementById("developer")
if (!workspace || !ammoMount || !optionsMount || !developerMount) throw new Error("ordering evidence mounts are missing")

function apply(runtime: VguiRuntime, operation: VguiOperation): number | undefined {
  const result = runtime.apply(operation)
  if (!result.ok) throw new Error(`${result.diagnostic.code}:${result.diagnostic.subject}`)
  return result.panel
}

function initialize(identity: string, root: HTMLElement): VguiRuntime {
  const result = initializeVguiRuntime({
    runtimeIdentity: identity, root, rootControl: { control: "EditablePanel", name: `${identity}-root` },
    viewport: { width: 1_280, height: 720, devicePixelRatio: 1 }, limits, clock, random, scheme,
    localization, animationScripts, customControls, reducedMotion: true,
    onRequest(request) { if (identity === "ordering-options") optionsRequests.push(request) },
  })
  if (!result.ok) throw new Error(`${result.diagnostic.code}:${result.diagnostic.subject}`)
  return result.runtime
}

function node(source: { name: string; value: string | null; children: readonly { name: string; value: string | null; children: readonly unknown[] }[] }): VguiResourceNode {
  return Object.freeze({
    name: source.name,
    value: source.value,
    condition: null,
    children: Object.freeze(source.children.filter((child) => !/_minmode$|_hidef$|_lodef$/iu.test(child.name)).map((child) => Object.freeze({
      name: child.name,
      value: child.value,
      condition: null,
      children: Object.freeze([]),
    }))),
  })
}

let ammo: VguiRuntime
let options: VguiRuntime
let developer: DeveloperConsole
let frame = 0
let applyButton = 0
const optionsRequests: VguiRequest[] = []
let ammoMode: "clip" | "no-clip" = "clip"
let status: Readonly<{ ready: boolean; error: string | null }> = Object.freeze({ ready: false, error: null })

async function prepare(): Promise<void> {
  for (const [family, url] of [["TF2 Build", "/fonts/tf2build.ttf"], ["TF2", "/fonts/tf2.ttf"]] as const) {
    const face = await new FontFace(family, `url(${url})`).load()
    document.fonts.add(face)
  }
  await document.fonts.ready

  ammo = initialize("ordering-ammo", ammoMount)
  const ammoPanel = apply(ammo, {
    kind: "create-panel", parent: 1, control: "EditablePanel", name: "HudWeaponAmmo",
    properties: [{ name: "xpos", value: "100" }, { name: "ypos", value: "80" }, { name: "wide", value: "240" }, { name: "tall", value: "90" }],
  })!
  apply(ammo, { kind: "set-dialog-variable", panel: ammoPanel, name: "Ammo", value: "88" })
  apply(ammo, { kind: "set-dialog-variable", panel: ammoPanel, name: "AmmoInReserve", value: "88" })
  const source = tf2UiResources.panels.find((candidate) => candidate.source.logicalPath === "resource/ui/hudammoweapons.res")
  if (!source || source.source.sha256 !== "a23a98f009dd34ac8c94e7149b1ded56eb9ed66e03d583fcd9c2ab68c3cb7734") {
    throw new Error("configured ammo resource identity differs")
  }
  const names = new Set(["AmmoInClip", "AmmoInClipShadow", "AmmoInReserve", "AmmoInReserveShadow", "AmmoNoClip", "AmmoNoClipShadow"])
  const root = source.roots[0]
  if (!root) throw new Error("configured ammo resource root is missing")
  apply(ammo, {
    kind: "replace-resource", parent: ammoPanel,
    document: {
      logicalIdentity: source.source.logicalPath,
      revision: source.source.sha256,
      root: Object.freeze({
        name: root.name,
        value: null,
        condition: null,
        children: Object.freeze(root.children.filter((child) => names.has(child.name)).map(node)),
      }),
    },
    selection: { activeConditions: [], resolutionSuffixes: [] },
  })
  for (const panel of ammo.snapshot().panels) {
    if (names.has(panel.name)) apply(ammo, { kind: "set-panel-state", panel: panel.id, visible: !panel.name.startsWith("AmmoNoClip") })
  }

  options = initialize("ordering-options", optionsMount)
  frame = apply(options, { kind: "create-panel", parent: 1, control: "Frame", name: "OptionsDialog", properties: [{ name: "title", value: "Options" }] })!
  apply(options, { kind: "set-bounds", panel: frame, bounds: { x: 360, y: 140, width: 512, height: 406 } })
  applyButton = apply(options, {
    kind: "create-panel", parent: frame, control: "Button", name: "ApplyButton",
    properties: [
      { name: "labelText", value: "Apply" }, { name: "command", value: "Apply" }, { name: "actionsignallevel", value: "1" },
      { name: "xpos", value: "400" }, { name: "ypos", value: "374" }, { name: "wide", value: "72" }, { name: "tall", value: "24" },
    ],
  })!
  apply(options, { kind: "set-panel-state", panel: 1, visible: false })

  const initialized = initializeDeveloperConsole({
    runtimeIdentity: "ordering-console", limits: consoleLimits, resources: consoleResources,
    catalog: { revision: "ordering", items: [] }, viewport: { width: 1_280, height: 720, devicePixelRatio: 1 },
    reducedMotion: true, onRequest() {},
  })
  if (!initialized.ok) throw new Error(initialized.diagnostic.code)
  developer = initialized.console
  const mounted = developer.apply({ kind: "mount", root: developerMount })
  if (!mounted.ok) throw new Error(mounted.diagnostic.code)

  status = Object.freeze({ ready: true, error: null })
}

declare global {
  interface Window {
    vguiOrderingEvidence: Readonly<{
      status(): Readonly<{ ready: boolean; error: string | null }>
      ammoSnapshot(): unknown
      setPairVisibility(foreground: boolean, shadow: boolean): void
      setAmmoMode(mode: "clip" | "no-clip"): void
      showConsole(): void
      showOptions(): void
      hideOptions(): void
      setOptionsModal(modal: boolean): void
      optionRequests(): readonly VguiRequest[]
      windowState(): unknown
    }>
  }
}

window.vguiOrderingEvidence = Object.freeze({
  status: () => status,
  ammoSnapshot: () => ammo.snapshot(),
  setPairVisibility: (foreground, shadow) => {
    for (const panel of ammo.snapshot().panels) {
      const active = ammoMode === "clip" ? !panel.name.startsWith("AmmoNoClip") : panel.name.startsWith("AmmoNoClip")
      if (["AmmoInClip", "AmmoInReserve", "AmmoNoClip"].includes(panel.name)) {
        apply(ammo, { kind: "set-panel-state", panel: panel.id, visible: active && foreground })
      }
      if (["AmmoInClipShadow", "AmmoInReserveShadow", "AmmoNoClipShadow"].includes(panel.name)) {
        apply(ammo, { kind: "set-panel-state", panel: panel.id, visible: active && shadow })
      }
    }
  },
  setAmmoMode: (mode) => {
    ammoMode = mode
    window.vguiOrderingEvidence.setPairVisibility(true, true)
  },
  showConsole: () => {
    const result = developer.apply({ kind: "activate" })
    if (!result.ok) throw new Error(result.diagnostic.code)
  },
  showOptions: () => {
    apply(options, { kind: "set-panel-state", panel: 1, visible: true })
    apply(options, { kind: "set-panel-state", panel: frame, visible: true })
    apply(options, { kind: "request-focus", panel: applyButton })
    apply(options, { kind: "frame", timeSeconds: performance.now() / 1_000 })
  },
  hideOptions: () => apply(options, { kind: "set-panel-state", panel: 1, visible: false }),
  setOptionsModal: (modal) => apply(options, { kind: "set-application-modal", panel: modal ? frame : null }),
  optionRequests: () => Object.freeze([...optionsRequests]),
  windowState: () => {
    const button = document.querySelector<HTMLElement>('[data-vgui-name="ApplyButton"]')!
    const rect = button.getBoundingClientRect()
    const x = Math.floor(rect.x + rect.width / 2)
    const y = Math.floor(rect.y + rect.height / 2)
    const element = document.elementFromPoint(x, y)
    return {
      x, y,
      target: element?.closest<HTMLElement>("[data-vgui-name]")?.dataset.vguiName ?? null,
      optionsLayer: getComputedStyle(optionsMount).zIndex,
      developerLayer: getComputedStyle(developerMount).zIndex,
      optionsVisible: options.snapshot().panels.find((panel) => panel.id === frame)?.effectivelyVisible,
      consoleVisible: developer.snapshot().visible,
      developerInert: developerMount.querySelector<HTMLElement>(".playsrc-vgui-root")?.inert ?? false,
    }
  },
})

void prepare().catch((error: unknown) => {
  status = Object.freeze({ ready: false, error: error instanceof Error ? error.message : String(error) })
})
