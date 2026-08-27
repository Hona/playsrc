import { expect, test } from "bun:test"
import { initializeVguiRuntime, type VguiResourceNode } from "@playsrc/vgui"
import { tf2CustomControls } from "../../src/ui-integration/resources"
import { tf2UiResources } from "../../src/ui-resources"
import { createRoot, FakeDocument } from "../../../../../packages/presentation/vgui/tests/fake-dom"

test("the production HUD registration compiles the authored count offset as a proportional float", () => {
  const registration = tf2CustomControls(tf2UiResources).find(control => control.name === "CTFHudElement")!
  expect(registration.animationVariables).toContainEqual({ name: "x_offset", converter: "proportional_float", defaultValue: "0" })
  const source = tf2UiResources.panels.find(panel => panel.source.logicalPath === "resource/ui/huditemeffectmeter_sniper.res")!
  const authored = source.roots[0]!.children.find(node => node.name === "HudItemEffectMeter")!.children.find(node => node.name === "x_offset")!
  expect(authored.value).toBe("40")
  const scalar = (name: string, value: string): VguiResourceNode => ({ name, value, condition: null, children: [] })
  for (const height of [720, 960]) {
    const initialized = initializeVguiRuntime({ runtimeIdentity: "count-offset", root: createRoot(new FakeDocument()) as unknown as HTMLElement,
      rootControl: { control: "EditablePanel", name: "Viewport" }, viewport: { width: 1280, height, devicePixelRatio: 1 },
      limits: { maxPanels: 64, maxHierarchyDepth: 16, maxChildrenPerPanel: 32, maxResourceNodes: 128, maxResourceDepth: 16,
        maxPropertiesPerPanel: 64, maxStringCodeUnits: 2048, maxTextCodeUnits: 2048, maxDialogVariables: 64,
        maxLocalizationTokens: 64, maxSchemeColors: 64, maxSchemeSettings: 64, maxSchemeBorders: 64, maxSchemeImages: 64,
        maxAnimationScripts: 16, maxAnimationSequences: 16, maxAnimationCommands: 64, maxActiveAnimations: 64,
        maxDelayedCommands: 64, maxQueuedMessages: 64, maxDiagnostics: 64, maxDomNodes: 256, maxListeners: 256 },
      clock: { nowSeconds: () => 0 }, random: { nextUnit: () => 0.5 },
      scheme: { identity: "empty", revision: "1", tag: "ClientScheme", colors: [], settings: [], borders: [], images: [], fonts: [] },
      localization: { identity: "empty", revision: "1", language: "english", tokens: [] },
      animationScripts: { identity: "empty", revision: "1", scripts: [], activeConditions: [] },
      customControls: [registration], reducedMotion: true, onRequest() {} })
    if (!initialized.ok) throw new Error(`${initialized.diagnostic.code}:${initialized.diagnostic.subject}`)
    const runtime = initialized.runtime
    expect(runtime.apply({ kind: "set-panel-state", panel: 1, proportional: true }).ok).toBe(true)
    const result = runtime.apply({ kind: "replace-resource", parent: 1, selection: { activeConditions: ["WIN32"], resolutionSuffixes: [] },
      document: { logicalIdentity: source.source.logicalPath, revision: source.source.sha256!, root: { name: "root", value: null, condition: null,
        children: [{ name: "Count", value: null, condition: null, children: [scalar("ControlName", "CTFHudElement"), scalar("fieldName", "Count"),
          scalar("x_offset", authored.value!)] }] } } })
    if (!result.ok) throw new Error(`${result.diagnostic.code}:${result.diagnostic.subject}`)
    expect(runtime.snapshot().panels.find(panel => panel.name === "Count")!.animationVariables.x_offset).toBe(height / 480 * 40)
    runtime.destroy()
  }
})
