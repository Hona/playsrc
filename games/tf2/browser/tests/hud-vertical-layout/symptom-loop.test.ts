import { describe, expect, test } from "bun:test"
import type {
  VguiControlRegistration,
  VguiImagePresentation,
  VguiResourceDocument,
  VguiResourceNode,
  VguiScheme,
} from "@playsrc/vgui"
import { FakeDocument, createRoot } from "../../../../../packages/presentation/vgui/tests/fake-dom"
import { initializeTf2HudIntegration } from "../../src/hud-integration"
import { TF2_HUD_DYNAMIC_IMAGES } from "../../src/hud"
import type { Tf2VguiResources } from "../../src/ui-integration"
import { tf2UiResources } from "../../src/ui-resources"

const scalar = (name: string, value: string): VguiResourceNode => Object.freeze({ name, value, condition: null, children: Object.freeze([]) })
const object = (name: string, children: readonly VguiResourceNode[]): VguiResourceNode => Object.freeze({ name, value: null, condition: null, children })
const panel = (name: string, geometry: Readonly<{ xpos: string; ypos: string; wide: string; tall: string }>): VguiResourceNode =>
  object(name, [
    scalar("ControlName", "CTFHudElement"),
    scalar("fieldName", name),
    scalar("xpos", geometry.xpos),
    scalar("ypos", geometry.ypos),
    scalar("wide", geometry.wide),
    scalar("tall", geometry.tall),
  ])

const documents = new Map<string, VguiResourceDocument>([
  ["scripts/hudlayout.res", Object.freeze({
    logicalIdentity: "scripts/hudlayout.res",
    revision: "1f18cb73d9ef54ff79ea208c9996db0655ac731b2ee8e9a82ff63a4b697f400f",
    root: object("HudLayout", [
      panel("HudPlayerStatus", { xpos: "0", ypos: "0", wide: "f0", tall: "480" }),
      object("HudWeaponAmmo", [
        scalar("ControlName", "CTFHudElement"),
        scalar("fieldName", "HudWeaponAmmo"),
        scalar("xpos", "r95"),
        scalar("ypos", "r55"),
        scalar("wide", "94"),
        scalar("tall", "45"),
      ]),
      panel("HudWeaponSelection", { xpos: "0", ypos: "0", wide: "f0", tall: "480" }),
      panel("HudCrosshair", { xpos: "0", ypos: "0", wide: "640", tall: "480" }),
    ]),
  })],
  ["resource/ui/hudplayerclass.res", Object.freeze({
    logicalIdentity: "resource/ui/hudplayerclass.res",
    revision: "10181165d10a81821672fd8e104d798e18cf896ca1156cf92df8ce0a07f8c89d",
    root: object("Resource", [panel("HudPlayerClass", { xpos: "0", ypos: "0", wide: "f0", tall: "480" })]),
  })],
  ["resource/ui/hudplayerhealth.res", Object.freeze({
    logicalIdentity: "resource/ui/hudplayerhealth.res",
    revision: "31fabca97c196eb2cff565b18ff8b3a17aa8806c8e1a980c9376782be7fa4774",
    root: object("Resource", [panel("HudPlayerHealth", { xpos: "0", ypos: "r120", wide: "250", tall: "120" })]),
  })],
  ["resource/ui/hudammoweapons.res", Object.freeze({
    logicalIdentity: "resource/ui/hudammoweapons.res",
    revision: "a23a98f009dd34ac8c94e7149b1ded56eb9ed66e03d583fcd9c2ab68c3cb7734",
    root: object("Resource", []),
  })],
  ["resource/ui/hudweaponselection.res", Object.freeze({
    logicalIdentity: "resource/ui/hudweaponselection.res",
    revision: "7a6f02c7eab4f0befdac5c69082c9334b0a03975738e1fc6d598ba6c91967138",
    root: object("Resource", []),
  })],
])

const customControls: readonly VguiControlRegistration[] = Object.freeze(["CTFHudElement", "CTFHealthPanel"].map((name) => Object.freeze({
  name,
  baseControl: "EditablePanel" as const,
  element: "div" as const,
  role: null,
  focusable: false,
  animationVariables: Object.freeze([]),
  acceptedProperties: Object.freeze([]),
})))

const images: readonly VguiImagePresentation[] = Object.freeze(TF2_HUD_DYNAMIC_IMAGES.map((name, index) => Object.freeze({
  name,
  logicalIdentity: `materials/vgui/hud-vertical-layout/${index}.vtf`,
  revision: `hud-vertical-layout-${index}`,
  browserUrl: "data:image/png;base64,AA==",
  width: 1,
  height: 1,
  frames: 1,
  hardwareFiltered: false,
})))

const scheme: VguiScheme = Object.freeze({
  identity: "resource/clientscheme.res",
  revision: "2701e270ea1da7e03b21dc780f12b7ea743868c612e3fbe4b8ab69e7dd8879de",
  tag: "ClientScheme",
  colors: Object.freeze([]),
  settings: Object.freeze([]),
  fonts: Object.freeze([]),
  borders: Object.freeze([]),
  images,
})

const resources: Tf2VguiResources = Object.freeze({
  identity: "tf2-hud-vertical-layout-loop",
  descriptor: tf2UiResources,
  clientScheme: scheme,
  sourceScheme: scheme,
  localization: Object.freeze({ identity: "resource/tf_english.txt", revision: "hud-vertical-layout", language: "english", tokens: Object.freeze([]) }),
  animations: Object.freeze({ identity: "scripts/hudanimations-manifest.txt", revision: "hud-vertical-layout", scripts: Object.freeze([]), activeConditions: Object.freeze([]) }),
  activeConditions: Object.freeze(["WIN32"]),
  customControls,
  diagnostics: Object.freeze([]),
  document(logicalPath) {
    const result = documents.get(logicalPath.toLowerCase())
    if (!result) throw new Error(`missing ${logicalPath}`)
    return result
  },
  panelDocument(logicalPath) {
    const result = tf2UiResources.panels.find((value) => value.source.logicalPath === logicalPath.toLowerCase())
    if (!result) throw new Error(`missing ${logicalPath}`)
    return result
  },
  destroy() {},
})

const viewports = Object.freeze([
  Object.freeze({ width: 1280, height: 720, devicePixelRatio: 1 }),
  Object.freeze({ width: 1024, height: 768, devicePixelRatio: 1 }),
  Object.freeze({ width: 1600, height: 900, devicePixelRatio: 1 }),
  Object.freeze({ width: 2560, height: 1080, devicePixelRatio: 1 }),
  Object.freeze({ width: 390, height: 844, devicePixelRatio: 1 }),
  Object.freeze({ width: 844, height: 390, devicePixelRatio: 1 }),
  Object.freeze({ width: 1280, height: 720, devicePixelRatio: 2 }),
  Object.freeze({ width: 1280, height: 720, devicePixelRatio: 1 }),
])

const scaled = (value: number, height: number) => Math.trunc(value * height / 480)

describe("configured TF2 HUD vertical viewport symptom loop", () => {
  test("keeps the HUD viewport and bottom panels on every admitted viewport transition", () => {
    const hud = initializeTf2HudIntegration({
      root: createRoot(new FakeDocument()) as unknown as HTMLElement,
      resources,
      viewport: viewports[0]!,
      reducedMotion: true,
      clock: { nowSeconds: () => 0 },
      random: { nextUnit: () => 0.5 },
      onCommand() {},
    })

    for (const viewport of viewports) {
      hud.setViewport(viewport)
      const snapshot = hud.snapshot().vgui
      const named = (name: string) => snapshot.panels.find((candidate) => candidate.name === name)!
      expect(snapshot.viewport).toEqual(viewport)
      expect(named("HudViewport").bounds).toEqual({ x: 0, y: 0, width: viewport.width, height: viewport.height })
      expect(named("HudPlayerStatus").bounds).toEqual({ x: 0, y: 0, width: viewport.width, height: viewport.height })
      expect(named("HudPlayerHealth").bounds).toEqual({
        x: 0,
        y: viewport.height - scaled(120, viewport.height),
        width: scaled(250, viewport.height),
        height: scaled(120, viewport.height),
      })
      expect(named("HudWeaponAmmo").bounds).toEqual({
        x: viewport.width - scaled(95, viewport.height),
        y: viewport.height - scaled(55, viewport.height),
        width: scaled(94, viewport.height),
        height: scaled(45, viewport.height),
      })
    }

    const beforeRepeat = hud.snapshot().vgui
    hud.setViewport(viewports.at(-1)!)
    expect(hud.snapshot().vgui).toEqual(beforeRepeat)
  })
})
