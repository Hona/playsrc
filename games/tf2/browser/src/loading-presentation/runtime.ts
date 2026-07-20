import {
  initializeVguiRuntime,
  type VguiOperation,
  type VguiPanelId,
  type VguiRequest,
  type VguiResourceDocument,
  type VguiResourceNode,
  type VguiRuntime,
  type VguiRuntimeLimits,
  type VguiRuntimeSnapshot,
  type VguiViewport,
} from "@playsrc/vgui"
import type { Tf2VguiResources } from "../ui-integration"
import type { Tf2LoadingPresentationRequest, Tf2LoadingPresentationSnapshot } from "./presentation"

const LIMITS: VguiRuntimeLimits = Object.freeze({
  maxPanels: 64,
  maxHierarchyDepth: 8,
  maxChildrenPerPanel: 32,
  maxResourceNodes: 256,
  maxResourceDepth: 8,
  maxPropertiesPerPanel: 256,
  maxStringCodeUnits: 4_095,
  maxTextCodeUnits: 65_535,
  maxDialogVariables: 32,
  maxLocalizationTokens: 4_096,
  maxSchemeColors: 1_024,
  maxSchemeSettings: 2_048,
  maxSchemeBorders: 512,
  maxSchemeImages: 2_048,
  maxAnimationScripts: 16,
  maxAnimationSequences: 1_024,
  maxAnimationCommands: 8_192,
  maxActiveAnimations: 2_048,
  maxDelayedCommands: 2_048,
  maxQueuedMessages: 64,
  maxDiagnostics: 256,
  maxDomNodes: 128,
  maxListeners: 32,
})

export type Tf2LoadingVguiRuntime = Readonly<{
  apply(snapshot: Tf2LoadingPresentationSnapshot): void
  snapshot(): VguiRuntimeSnapshot
  setViewport(viewport: VguiViewport): void
  frame(timeSeconds: number): void
  destroy(): void
}>

export type Tf2LoadingVguiRuntimeInput = Readonly<{
  root: HTMLElement
  resources: Tf2VguiResources
  viewport: VguiViewport
  reducedMotion: boolean
  clock: Readonly<{ nowSeconds(): number }>
  random: Readonly<{ nextUnit(): number }>
  onRequest(request: Tf2LoadingPresentationRequest): void
}>

const scalar = (node: VguiResourceNode, name: string): string | null =>
  node.children.find((child) => child.name.toLowerCase() === name.toLowerCase() && child.value !== null)?.value ?? null
const clone = (node: VguiResourceNode, children = node.children): VguiResourceNode => Object.freeze({
  name: node.name,
  value: node.value,
  condition: node.condition,
  children: Object.freeze(children),
})

function withoutFrame(document: VguiResourceDocument): VguiResourceDocument {
  return Object.freeze({
    logicalIdentity: `${document.logicalIdentity}/loading-children`,
    revision: document.revision,
    root: clone(document.root, document.root.children.filter((node) => (scalar(node, "fieldName") ?? node.name).toLowerCase() !== "loadingdialog")),
  })
}

function mustApply(runtime: VguiRuntime, operation: VguiOperation): VguiPanelId | undefined {
  const result = runtime.apply(operation)
  if (!result.ok) throw new Error(`${result.diagnostic.code}:${result.diagnostic.subject}`)
  return result.panel
}

export function initializeTf2LoadingVguiRuntime(input: Tf2LoadingVguiRuntimeInput): Tf2LoadingVguiRuntime {
  let destroyed = false
  let viewport = input.viewport
  const initialized = initializeVguiRuntime({
    runtimeIdentity: "tf2-loading-presentation",
    root: input.root,
    rootControl: { control: "EditablePanel", name: "LoadingPresentationRoot" },
    viewport,
    limits: LIMITS,
    clock: input.clock,
    random: input.random,
    scheme: input.resources.clientScheme,
    localization: input.resources.localization,
    animationScripts: input.resources.animations,
    customControls: input.resources.customControls,
    reducedMotion: input.reducedMotion,
    onRequest: (request: VguiRequest) => {
      if (request.kind !== "command") return
      if (request.command === "Cancel") input.onRequest(Object.freeze({ kind: "disconnect" }))
      else if (request.command === "Close") input.onRequest(Object.freeze({ kind: "dismiss-failure" }))
    },
  })
  if (!initialized.ok) throw new Error(`${initialized.diagnostic.code}:${initialized.diagnostic.subject}`)
  const runtime = initialized.runtime
  const mapInfo = mustApply(runtime, {
    kind: "create-panel",
    parent: 1,
    control: "EditablePanel",
    name: "MapInfo",
    properties: [
      { name: "xpos", value: "0" }, { name: "ypos", value: "0" },
      { name: "wide", value: String(viewport.width) }, { name: "tall", value: String(viewport.height) },
      { name: "paintbackground", value: "1" }, { name: "bgcolor_override", value: "46 43 42 255" },
    ],
  })!
  const background = mustApply(runtime, {
    kind: "create-panel",
    parent: mapInfo,
    control: "ImagePanel",
    name: "Background",
    properties: [
      { name: "xpos", value: "0" }, { name: "ypos", value: "0" },
      { name: "wide", value: String(Math.trunc(viewport.height * (4 / 3))) }, { name: "tall", value: String(viewport.height) },
      { name: "image", value: "stamp_background_map" }, { name: "scaleImage", value: "1" },
    ],
  })!
  const dialog = mustApply(runtime, { kind: "create-panel", parent: 1, control: "Frame", name: "LoadingDialog" })!
  mustApply(runtime, { kind: "set-panel-state", panel: 1, visible: false, mouseInput: false, keyboardInput: false })

  const panel = (name: string): VguiPanelId => {
    const found = runtime.snapshot().panels.find((candidate) => candidate.name.toLowerCase() === name.toLowerCase())
    if (!found) throw new Error(`TF2 loading control ${name} is missing`)
    return found.id
  }
  const layoutBackground = (snapshot: Tf2LoadingPresentationSnapshot): void => {
    const width = snapshot.background?.backgroundWidth ?? Math.trunc(viewport.height * (4 / 3))
    mustApply(runtime, { kind: "set-bounds", panel: mapInfo, bounds: { x: 0, y: 0, width: viewport.width, height: viewport.height } })
    mustApply(runtime, { kind: "set-bounds", panel: background, bounds: { x: 0, y: 0, width, height: viewport.height } })
  }

  return Object.freeze({
    apply: (snapshot) => {
      if (destroyed) return
      layoutBackground(snapshot)
      for (const operation of snapshot.operations) {
        if (operation.kind === "mount") {
          const document = input.resources.document(operation.resource.source.logicalPath)
          mustApply(runtime, {
            kind: "replace-resource",
            parent: dialog,
            document: withoutFrame(document),
            selection: { activeConditions: input.resources.activeConditions, resolutionSuffixes: ["_hidef"] },
          })
          mustApply(runtime, { kind: "set-panel-state", panel: 1, visible: true, mouseInput: true, keyboardInput: true })
          mustApply(runtime, { kind: "set-panel-state", panel: dialog, visible: true, popup: true, mouseInput: true, keyboardInput: true })
          mustApply(runtime, { kind: "set-application-modal", panel: dialog })
          mustApply(runtime, { kind: "move-to-front", panel: dialog })
        } else if (operation.kind === "bounds") {
          mustApply(runtime, { kind: "set-bounds", panel: dialog, bounds: { x: operation.x, y: operation.y, width: operation.width, height: operation.height } })
        } else if (operation.kind === "status") {
          mustApply(runtime, { kind: "mutate-control", panel: panel(operation.control), mutation: { text: operation.text } })
        } else if (operation.kind === "progress") {
          mustApply(runtime, { kind: "mutate-control", panel: panel(operation.control), mutation: { progress: operation.value } })
        } else if (operation.kind === "button") {
          mustApply(runtime, { kind: "mutate-control", panel: panel(operation.control), mutation: { text: operation.text, command: operation.command } })
        } else if (operation.kind === "failure-layout") {
          const info = runtime.snapshot().panels.find((candidate) => candidate.name === "InfoLabel")
          if (!info) throw new Error("TF2 failure InfoLabel is missing")
          const height = info.bounds.y + info.bounds.height + operation.contentBottomPadding
          const button = panel("CancelButton")
          mustApply(runtime, { kind: "set-bounds", panel: dialog, bounds: { x: (viewport.width - 380) / 2, y: (viewport.height - height) / 2, width: 380, height } })
          const buttonBounds = runtime.snapshot().panels.find((candidate) => candidate.id === button)!.bounds
          mustApply(runtime, { kind: "set-bounds", panel: button, bounds: { ...buttonBounds, y: info.bounds.y + info.bounds.height + operation.buttonGap } })
        } else {
          mustApply(runtime, { kind: "set-application-modal", panel: null })
          mustApply(runtime, { kind: "set-panel-state", panel: 1, visible: false, mouseInput: false, keyboardInput: false })
        }
      }
    },
    snapshot: () => runtime.snapshot(),
    setViewport: (next) => { viewport = next; mustApply(runtime, { kind: "set-viewport", viewport: next }) },
    frame: (timeSeconds) => { if (!destroyed) mustApply(runtime, { kind: "frame", timeSeconds }) },
    destroy: () => { if (!destroyed) { destroyed = true; mustApply(runtime, { kind: "destroy" }) } },
  })
}
