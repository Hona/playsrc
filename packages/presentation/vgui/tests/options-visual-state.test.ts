import { describe, expect, test } from "bun:test"
import {
  initializeVguiRuntime,
  type VguiOperation,
  type VguiRuntime,
  type VguiRuntimeLimits,
  type VguiScheme,
} from "../src"
import { byName, createRoot, descendants, FakeDocument } from "./fake-dom"

const limits: VguiRuntimeLimits = Object.freeze({
  maxPanels: 64, maxHierarchyDepth: 8, maxChildrenPerPanel: 32, maxResourceNodes: 128, maxResourceDepth: 8,
  maxPropertiesPerPanel: 64, maxStringCodeUnits: 1_024, maxTextCodeUnits: 4_096, maxDialogVariables: 32,
  maxLocalizationTokens: 32, maxSchemeColors: 64, maxSchemeSettings: 64, maxSchemeBorders: 16, maxSchemeImages: 16,
  maxAnimationScripts: 4, maxAnimationSequences: 32, maxAnimationCommands: 64, maxActiveAnimations: 32,
  maxDelayedCommands: 32, maxQueuedMessages: 64, maxDiagnostics: 128, maxDomNodes: 128, maxListeners: 32,
})

const scheme: VguiScheme = Object.freeze({
  identity: "resource/options-visual-audit.res",
  revision: "options-visual-audit-1",
  tag: "SourceScheme",
  colors: Object.freeze([
    { name: "Panel.FgColor", value: "1 2 3 255" },
    { name: "Panel.BgColor", value: "4 5 6 255" },
    { name: "Label.TextColor", value: "11 12 13 255" },
    { name: "Label.DisabledFgColor1", value: "14 15 16 255" },
    { name: "Label.DisabledFgColor2", value: "17 18 19 255" },
    { name: "Label.BgColor", value: "0 0 0 0" },
    { name: "Button.TextColor", value: "21 22 23 255" },
    { name: "Button.BgColor", value: "24 25 26 255" },
    { name: "Button.ArmedTextColor", value: "31 32 33 255" },
    { name: "Button.ArmedBgColor", value: "34 35 36 255" },
    { name: "Button.DepressedTextColor", value: "41 42 43 255" },
    { name: "Button.DepressedBgColor", value: "44 45 46 255" },
    { name: "Button.SelectedTextColor", value: "51 52 53 255" },
    { name: "Button.SelectedBgColor", value: "54 55 56 255" },
    { name: "Button.DisabledTextColor", value: "61 62 63 255" },
    { name: "Button.DisabledBgColor", value: "64 65 66 255" },
    { name: "SectionedListPanel.TextColor", value: "71 72 73 255" },
    { name: "SectionedListPanel.BrightTextColor", value: "74 75 76 255" },
    { name: "SectionedListPanel.SelectedTextColor", value: "77 78 79 255" },
    { name: "SectionedListPanel.SelectedBgColor", value: "80 81 82 255" },
    { name: "TextEntry.TextColor", value: "81 82 83 255" },
    { name: "TextEntry.BgColor", value: "84 85 86 255" },
    { name: "Menu.TextColor", value: "91 92 93 255" },
    { name: "Menu.BgColor", value: "94 95 96 120" },
    { name: "Menu.ArmedTextColor", value: "101 102 103 255" },
    { name: "Menu.ArmedBgColor", value: "104 105 106 255" },
  ]),
  settings: Object.freeze([]),
  fonts: Object.freeze([{
    name: "Default", cssFamily: "Options Audit", sizePx: 12, lineHeightPx: 14, weight: 400,
    style: "normal", available: true, measure: (text: string) => Object.freeze({ width: text.length * 7, height: 14 }),
  }]),
  borders: Object.freeze([{
    kind: "line", name: "MenuBorder", inset: { left: 1, top: 1, right: 1, bottom: 1 }, backgroundType: 0, paintFirst: false,
    sides: {
      left: [{ color: [111, 112, 113, 255], startOffset: 0, endOffset: 0 }],
      top: [{ color: [111, 112, 113, 255], startOffset: 0, endOffset: 0 }],
      right: [{ color: [114, 115, 116, 255], startOffset: 0, endOffset: 0 }],
      bottom: [{ color: [114, 115, 116, 255], startOffset: 0, endOffset: 0 }],
    },
  }]),
  images: Object.freeze([]),
})

function apply(runtime: VguiRuntime, operation: VguiOperation): number | undefined {
  const result = runtime.apply(operation)
  if (!result.ok) throw new Error(`${result.diagnostic.code}:${result.diagnostic.subject}`)
  return result.panel
}

function setup() {
  const root = createRoot(new FakeDocument())
  const initialized = initializeVguiRuntime({
    runtimeIdentity: "options-visual-audit", root: root as unknown as HTMLElement,
    rootControl: { control: "EditablePanel", name: "Root" }, viewport: { width: 1_280, height: 720, devicePixelRatio: 1 },
    limits, clock: { nowSeconds: () => 0 }, random: { nextUnit: () => 0.5 }, scheme,
    localization: { identity: "resource/options-audit-english.txt", revision: "1", language: "english", tokens: [] },
    animationScripts: { identity: "scripts/options-audit", revision: "1", scripts: [], activeConditions: [] },
    customControls: [], reducedMotion: true, onRequest() {},
  })
  if (!initialized.ok) throw new Error(initialized.diagnostic.code)
  return { root, runtime: initialized.runtime }
}

describe("resource-derived Options control visual states", () => {
  test("inherits Label alignment and uses disabled Button scheme colors", () => {
    const { root, runtime } = setup()
    apply(runtime, { kind: "create-panel", parent: 1, control: "Label", name: "SettingLabel", properties: [
      { name: "labelText", value: "Mouse sensitivity" }, { name: "font", value: "Default" },
      { name: "textAlignment", value: "west" }, { name: "wide", value: "180" }, { name: "tall", value: "24" },
    ] })
    const button = apply(runtime, { kind: "create-panel", parent: 1, control: "Button", name: "SettingValue", properties: [
      { name: "labelText", value: "Enabled" }, { name: "font", value: "Default" },
      { name: "textAlignment", value: "west" }, { name: "wide", value: "180" }, { name: "tall", value: "24" },
    ] })!
    apply(runtime, { kind: "set-panel-state", panel: button, enabled: false })

    const label = byName(root, "SettingLabel")
    const value = byName(root, "SettingValue")
    expect(label.style.justifyContent).toBe("flex-start")
    expect(value.style.justifyContent).toBe("flex-start")
    expect(value.style.color).toBe("rgba(17, 18, 19, 1)")
    expect(value.style.textShadow).toBe("1px 1px 0 rgba(14, 15, 16, 1)")
    expect(value.style.backgroundColor).toBe("rgba(24, 25, 26, 1)")
  })

  test("treats SectionedList BRIGHT as color rather than centered alignment", () => {
    const { root, runtime } = setup()
    const list = apply(runtime, { kind: "create-panel", parent: 1, control: "SectionedListPanel", name: "Bindings", properties: [
      { name: "wide", value: "400" }, { name: "tall", value: "120" }, { name: "linespacing", value: "20" },
    ] })!
    apply(runtime, { kind: "mutate-control", panel: list, mutation: {
      sections: [{ id: 1, name: "Movement", alwaysVisible: true, minimumHeight: 0, columns: [
        { name: "Action", text: "Action", flags: 0x04, width: 260 },
        { name: "Key", text: "Key", flags: 0x04, width: 120 },
      ] }],
      sectionedItems: [{ id: 1, section: 1, cells: { Action: "Move forward", Key: "W" }, enabled: true }],
    } })
    const cells = descendants(root).filter((element) => element.getAttribute("role") === "gridcell")
    expect(cells.map((cell) => cell.style.textAlign)).toEqual(["left", "left"])
    expect(cells.map((cell) => cell.style.color)).toEqual(["rgba(74, 75, 76, 1)", "rgba(74, 75, 76, 1)"])
  })

  test("opens one positioned ComboBox popup with distinct selected, armed and disabled rows", () => {
    const { root, runtime } = setup()
    const combo = apply(runtime, { kind: "create-panel", parent: 1, control: "ComboBox", name: "Quality", properties: [
      { name: "xpos", value: "100" }, { name: "ypos", value: "50" }, { name: "wide", value: "180" }, { name: "tall", value: "24" },
      { name: "editable", value: "0" }, { name: "numLines", value: "5" }, { name: "font", value: "Default" },
    ] })!
    apply(runtime, { kind: "mutate-control", panel: combo, mutation: {
      items: [
        { id: 1, text: "Low", enabled: true },
        { id: 2, text: "High", enabled: true, checked: true },
        { id: 3, text: "Unavailable", enabled: false },
      ],
      activeIndex: 1,
      editable: false,
    } })
    apply(runtime, { kind: "pointer-press", button: "left", x: 110, y: 60, pointerId: 1, clicks: 1 })
    apply(runtime, { kind: "frame", timeSeconds: 0 })
    const popup = descendants(root).find((element) => element.dataset.vguiComboPopup === "Quality")!
    expect(popup).toBeDefined()
    expect([popup.style.left, popup.style.top, popup.style.width, popup.style.height, popup.style.display]).toEqual(["100px", "75px", "180px", "60px", "block"])
    expect(popup.style.backgroundColor).toBe("rgba(94, 95, 96, 1)")
    expect(popup.children).toHaveLength(3)
    expect(popup.children[1]!.dataset).toMatchObject({ selected: "true", armed: "true", checked: "true" })
    expect(popup.children[1]!.style.color).toBe("rgba(101, 102, 103, 1)")
    expect(popup.children[2]!.style.color).toBe("rgba(17, 18, 19, 1)")

    apply(runtime, { kind: "pointer-move", x: 110, y: 85, pointerId: 1 })
    apply(runtime, { kind: "frame", timeSeconds: 0 })
    expect(runtime.snapshot().panels.find((panel) => panel.id === combo)?.state).toMatchObject({ activeIndex: 1, highlightedIndex: 0 })
    apply(runtime, { kind: "pointer-press", button: "left", x: 110, y: 85, pointerId: 1, clicks: 1 })
    apply(runtime, { kind: "pointer-release", button: "left", x: 110, y: 85, pointerId: 1 })
    apply(runtime, { kind: "frame", timeSeconds: 0 })
    expect(runtime.snapshot().panels.find((panel) => panel.id === combo)).toMatchObject({ text: "Low", state: { activeIndex: 0, highlightedIndex: null } })
    expect(popup.style.display).toBe("none")
  })
  test("a scrolled map combo hits the displayed row rather than the same row in the first page",()=>{
    const {root,runtime}=setup()
    const combo=apply(runtime,{kind:"create-panel",parent:1,control:"ComboBox",name:"Maps",properties:[
      {name:"xpos",value:"100"},{name:"ypos",value:"50"},{name:"wide",value:"180"},{name:"tall",value:"24"},{name:"numLines",value:"5"},
    ]})!
    apply(runtime,{kind:"mutate-control",panel:combo,mutation:{items:Array.from({length:15},(_,id)=>({id,text:`map_${id}`,enabled:true})),activeIndex:0,editable:false}})
    apply(runtime,{kind:"pointer-press",button:"left",x:110,y:60,pointerId:1,clicks:1});apply(runtime,{kind:"frame",timeSeconds:0})
    const popup=descendants(root).find(element=>element.dataset.vguiComboPopup==="Maps")!
    popup.scrollTop=200 // Browser scrollbar or scrolling an option into view.
    apply(runtime,{kind:"pointer-move",x:110,y:145,pointerId:1});apply(runtime,{kind:"frame",timeSeconds:0})
    expect(runtime.snapshot().panels.find(panel=>panel.id===combo)?.state.highlightedIndex).toBe(13)
    apply(runtime,{kind:"pointer-press",button:"left",x:110,y:145,pointerId:1,clicks:1})
    apply(runtime,{kind:"pointer-release",button:"left",x:110,y:145,pointerId:1});apply(runtime,{kind:"frame",timeSeconds:0})
    expect(runtime.snapshot().panels.find(panel=>panel.id===combo)).toMatchObject({text:"map_13",state:{activeIndex:13}})
    apply(runtime,{kind:"pointer-press",button:"left",x:110,y:60,pointerId:1,clicks:1});apply(runtime,{kind:"frame",timeSeconds:0})
    expect(popup.scrollTop).toBe(200)
    apply(runtime,{kind:"pointer-wheel",delta:1,x:110,y:145});apply(runtime,{kind:"frame",timeSeconds:0})
    expect(popup.scrollTop).toBe(180)
    apply(runtime,{kind:"pointer-wheel",delta:-100,x:110,y:145});apply(runtime,{kind:"frame",timeSeconds:0})
    expect(popup.scrollTop).toBe(200)
  })
})
