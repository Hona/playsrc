import { describe, expect, test } from "bun:test"
import {
  isVguiGenericResourcePropertySupported,
  VGUI_GENERIC_CONTROL_NAMES,
  type VguiControlRegistration,
  type VguiImagePresentation,
  type VguiResourceDocument,
  type VguiResourceNode,
  type VguiScheme,
} from "@playsrc/vgui"
import { initializeTf2GameUiIntegration } from "../../src/gameui-integration"
import { initializeTf2HudIntegration } from "../../src/hud-integration"
import { initializeTf2BrowserSettings, initializeTf2OptionsPresentation } from "../../src/settings-integration"
import { TF2_HUD_DYNAMIC_IMAGES, tf2HudUnavailable, type SessionHudContext, type SessionSimulationPublication } from "../../src/hud"
import type { Tf2VguiResources } from "../../src/ui-integration"
import { tf2UiResources, type Tf2UiResourceNode } from "../../src/ui-resources"
import { FakeDocument, createRoot, descendants } from "../../../../../packages/presentation/vgui/tests/fake-dom"

const generic = new Set<string>(VGUI_GENERIC_CONTROL_NAMES)

function convert(value: Tf2UiResourceNode): VguiResourceNode {
  const control = value.children.find((child) => child.name.toLowerCase() === "controlname" && child.value !== null)?.value
  return Object.freeze({
    name: value.name,
    value: value.value,
    condition: value.condition?.symbol ?? null,
    children: Object.freeze(value.children.filter((child) =>
      !/(?:_lodef|_minmode)$/iu.test(child.name)
      && !/^(?:border|border_override|background-texture|paintbackgroundtype|drawcolor|frame|image)$/iu.test(child.name)
      && !(child.value !== null && control && generic.has(control) && !isVguiGenericResourcePropertySupported(control as never, child.name)),
    ).map(convert)),
  })
}

function scheme(): VguiScheme {
  const images = [...Map.groupBy(tf2UiResources.images, (image) => image.configuredValue.toLowerCase()).values()]
    .map((items) => items[0]!)
  const presentedImages: VguiImagePresentation[] = images.map((image, index) => Object.freeze({
    name: image.configuredValue,
    logicalIdentity: `materials/vgui/symptom-loop/${index}.vtf`,
    revision: image.material?.sha256 ?? image.identity,
    browserUrl: "data:image/png;base64,AA==",
    width: image.textures[0]?.width ?? 1,
    height: image.textures[0]?.height ?? 1,
    frames: image.textures[0]?.frames ?? 1,
    hardwareFiltered: false,
  }))
  const presentedNames = new Set(presentedImages.map((image) => image.name.toLowerCase()))
  for (const [index, name] of TF2_HUD_DYNAMIC_IMAGES.entries()) {
    if (presentedNames.has(name.toLowerCase())) continue
    presentedImages.push(Object.freeze({
      name, logicalIdentity: `materials/vgui/symptom-loop/dynamic-${index}.vtf`, revision: `dynamic-${index}`,
      browserUrl: "data:image/png;base64,AA==", width: 128, height: 128, frames: 1, hardwareFiltered: false,
    }))
  }
  for (let corner = 1; corner <= 4; corner += 1) presentedImages.push(Object.freeze({
    name: `vgui/hud/8x800corner${corner}`,
    logicalIdentity: `materials/vgui/symptom-loop/corner${corner}.vtf`, revision: `corner${corner}`,
    browserUrl: "data:image/png;base64,AA==", width: 8, height: 800, frames: 1, hardwareFiltered: false,
  }))
  for (const [index, name] of ["../console/background_2fort", "../console/background_2fort_widescreen"].entries()) presentedImages.push(Object.freeze({
    name, logicalIdentity: `materials/console/background_2fort${index ? "_widescreen" : ""}.vmt`, revision: `background-${index}`,
    browserUrl: "data:image/png;base64,AA==", width: 1, height: 1, frames: 1, hardwareFiltered: false,
  }))
  return Object.freeze({
    identity: "tf2-symptom-loop",
    revision: tf2UiResources.identity,
    tag: "ClientScheme",
    colors: Object.freeze([
      { name: "TanLight", value: "235 226 202 255" },
      { name: "TanDark", value: "117 107 94 255" },
      { name: "Black", value: "46 43 42 255" },
      { name: "TransparentBlack", value: "0 0 0 196" },
    ]),
    settings: Object.freeze([]),
    fonts: Object.freeze([...new Set(tf2UiResources.schemes.flatMap((value) => value.fontDefinitions.map((font) => font.name)))].map((name) => Object.freeze({
      name, cssFamily: "sans-serif", sizePx: 12, lineHeightPx: 12, weight: 400, style: "normal" as const, available: true,
      measure: () => Object.freeze({ width: 0, height: 12 }),
    }))),
    borders: Object.freeze([]),
    images: Object.freeze(presentedImages),
  })
}

function resources(): Tf2VguiResources {
  const documents = new Map<string, VguiResourceDocument>()
  for (const panel of tf2UiResources.panels) {
    const root = panel.roots[0]
    if (!root || !panel.source.sha256) continue
    documents.set(panel.source.logicalPath, Object.freeze({
      logicalIdentity: panel.source.logicalPath,
      revision: panel.source.sha256,
      root: convert(root),
    }))
  }
  const accepted = new Map<string, Set<string>>()
  const visit = (value: Tf2UiResourceNode): void => {
    const control = value.children.find((child) => child.name.toLowerCase() === "controlname" && child.value !== null)?.value
    if (control) {
      const names = accepted.get(control) ?? new Set<string>()
      for (const child of value.children) if (child.value !== null && child.name.toLowerCase() !== "controlname") names.add(child.name)
      accepted.set(control, names)
    }
    for (const child of value.children) if (child.value === null) visit(child)
  }
  for (const panel of tf2UiResources.panels) for (const root of panel.roots) visit(root)
  const hudProperties = new Set(tf2UiResources.panels
    .filter((panel) => panel.domain === "hud")
    .flatMap((panel) => panel.roots.flatMap((root) => root.children.flatMap((block) => block.children.filter((child) => child.value !== null).map((child) => child.name)))))
  accepted.set("CTFHudElement", hudProperties)
  accepted.set("CTFHealthPanel", hudProperties)
  const menuProperties = new Set(tf2UiResources.panels
    .filter((panel) => panel.domain === "main-menu")
    .flatMap((panel) => panel.roots.flatMap((root) => root.children.flatMap((block) => block.children.filter((child) => child.value !== null).map((child) => child.name)))))
  accepted.set("CTFMatchmakingDashboard", menuProperties)
  accepted.set("CTFPlaylistPanel", menuProperties)
  accepted.set("CHudMainMenuOverride", new Set(["update_url", "blog_url", "button_x_offset", "button_y", "button_y_delta"]))
  const customControls: VguiControlRegistration[] = tf2UiResources.controls
    .filter((control) => !generic.has(control.name))
    .map((control) => Object.freeze({
      name: control.name,
      baseControl: /COptionsSubVideoAdvancedDlg|CGammaDialog/u.test(control.name) ? "Frame"
        : /CCvarSlider/u.test(control.name) ? "Slider"
          : /CLabeledCommandComboBox/u.test(control.name) ? "ComboBox"
            : /CCvar.*CheckButton/u.test(control.name) ? "CheckButton"
              : /button/iu.test(control.name) ? "Button" : /image|class/iu.test(control.name) ? "ImagePanel" : "EditablePanel",
      element: /button/iu.test(control.name) ? "button" : "div",
      role: null,
      focusable: /button|slider|combo|dialog/iu.test(control.name),
      animationVariables: Object.freeze([]),
      acceptedProperties: Object.freeze([
        ...(accepted.get(control.name) ?? []),
        ...(/COptionsSubVideoAdvancedDlg|CGammaDialog/u.test(control.name) ? ["sizeable", "moveable", "title"] : []),
      ]),
    }))
  for (const name of ["CTFHudElement", "CTFHealthPanel", "CHudMainMenuOverride", "CTFMatchmakingDashboard", "CTFPlaylistPanel"]) {
    if (customControls.some((control) => control.name === name)) continue
    customControls.push(Object.freeze({
      name, baseControl: "EditablePanel", element: "div", role: null, focusable: false,
      animationVariables: Object.freeze([]), acceptedProperties: Object.freeze([...(accepted.get(name) ?? [])]),
    }))
  }
  const clientScheme = scheme()
  return Object.freeze({
    identity: tf2UiResources.identity,
    descriptor: tf2UiResources,
    clientScheme,
    sourceScheme: clientScheme,
    localization: Object.freeze({ identity: "tf2-test", revision: "1", language: "english", tokens: Object.freeze([]) }),
    animations: Object.freeze({ identity: "tf2-test", revision: "1", scripts: Object.freeze([]), activeConditions: Object.freeze([]) }),
    activeConditions: Object.freeze(["WIN32", "OSX", "POSIX"]),
    customControls: Object.freeze(customControls),
    gameUiBackground: Object.freeze({
      identity: "tf2-gameui-background-test", contentBuild: tf2UiResources.contentBuild,
      source: Object.freeze({ logicalPath: "scripts/chapterbackgrounds.txt", byteLength: 1, sha256: "0".repeat(64) }),
      defaultChapter: 1 as const, backgroundName: "background_2fort",
      variants: Object.freeze(["standard", "widescreen"].map((aspect, index) => Object.freeze({
        aspect: aspect as "standard" | "widescreen", image: `../console/background_2fort${index ? "_widescreen" : ""}`,
        material: `materials/console/background_2fort${index ? "_widescreen" : ""}.vmt`, materialSha256: `${index}`.repeat(64),
        texture: `materials/console/background_2fort${index ? "_widescreen" : ""}.vtf`, textureSha256: `${index}`.repeat(64), width: 1, height: 1,
      }))),
    }),
    diagnostics: Object.freeze([]),
    document(logicalPath) {
      const result = documents.get(logicalPath.toLowerCase())
      if (!result) throw new Error(`missing configured document ${logicalPath}`)
      return result
    },
    panelDocument(logicalPath) {
      const result = tf2UiResources.panels.find((panel) => panel.source.logicalPath === logicalPath.toLowerCase())
      if (!result) throw new Error(`missing configured panel ${logicalPath}`)
      return result
    },
    destroy() {},
  })
}

function compact(
  tick: bigint,
  classIdentity: 3 | 4,
  team: 2 | 3,
  weapon: 1 | 2 | 3,
  clip: number,
  reserve: number,
  reload: 0 | 1 | 2 | 3 = 0,
  conditions: readonly [number, number, number, number, number] = Object.freeze([0, 0, 0, 0, 0]),
): SessionSimulationPublication {
  const maximumClip = weapon === 3 ? 8 : 4
  const maximumReserve = weapon === 3 ? 24 : 20
  const snapshot = Object.freeze({
    tick,
    class: classIdentity,
    team,
    weapon,
    health: classIdentity === 3 ? 200 : 175,
    maximumHealth: classIdentity === 3 ? 200 : 175,
    lifecycle: 1 as const,
    conditions: Object.freeze([...conditions]) as readonly [number, number, number, number, number],
    loadout: Object.freeze([Object.freeze({ weapon, reload, clip, reserve, maximumClip, maximumReserve })]),
    events: Object.freeze([]),
    lifecycleEvents: Object.freeze([]),
    projectileEvents: Object.freeze([]),
  })
  return Object.freeze({ eventBatches: Object.freeze([Object.freeze({ snapshot })]), snapshot })
}

const context: SessionHudContext = Object.freeze({
  playerIdentity: 1,
  liveHudSuppressed: false,
  respawnAllowed: true,
  weaponSelection: Object.freeze({ open: false, selectedWeapon: tf2HudUnavailable<number>("not-applicable") }),
  crosshair: Object.freeze({
    configured: true, weaponAllows: true, loadingImage: false, paused: false, clientModeAllows: true,
    frozen: false, localViewEntity: true, vguiInput: false, observerMode: "none", observerCrosshair: false,
    tfSuppressed: false, countdownHidden: false, texture: "crosshair_default", color: Object.freeze([255, 255, 255, 255]),
    scale: 32, weaponScale: 1,
  }),
  scoreboard: tf2HudUnavailable("not-produced"),
  freezePanel: tf2HudUnavailable("not-produced"),
  playerClassUsePlayerModel: false,
})

const visible = (panels: readonly { name: string; effectivelyVisible: boolean }[], names: readonly string[]) =>
  panels.filter((panel) => names.includes(panel.name) && panel.effectivelyVisible).map((panel) => panel.name)

const contextWithModel = (playerClassUsePlayerModel: boolean): SessionHudContext =>
  Object.freeze({ ...context, playerClassUsePlayerModel })

describe("TF2 HUD and pause headed symptom loop", () => {
  test("rejects an image closure that would preserve a configured Scout fallback", () => {
    const complete = resources()
    const incomplete: Tf2VguiResources = Object.freeze({
      ...complete,
      clientScheme: Object.freeze({
        ...complete.clientScheme,
        images: Object.freeze(complete.clientScheme.images.filter((image) => image.name !== "../hud/class_soldierred")),
      }),
    })
    expect(() => initializeTf2HudIntegration({
      root: createRoot(new FakeDocument()) as unknown as HTMLElement,
      resources: incomplete, viewport: { width: 1280, height: 720, devicePixelRatio: 1 }, reducedMotion: true,
      clock: { nowSeconds: () => 0 }, random: { nextUnit: () => 0.5 }, onCommand() {},
    })).toThrow("TF2 HUD dynamic images are unavailable: ../hud/class_soldierred")
  })

  test("renders one exact class/team/ammo identity and no inactive condition icon", () => {
    const root = createRoot(new FakeDocument())
    const hud = initializeTf2HudIntegration({
      root: root as unknown as HTMLElement,
      resources: resources(), viewport: { width: 1280, height: 720, devicePixelRatio: 1 }, reducedMotion: true,
      clock: { nowSeconds: () => 0 }, random: { nextUnit: () => 0.5 }, onCommand() {},
      playerClassUsePlayerModel: false,
    } as never)
    hud.publish(compact(1n, 3, 2, 1, 3, 20), context)
    const first = hud.snapshot().vgui.panels
    expect(visible(first, ["PlayerStatusClassImage", "classmodelpanel"])).toEqual(["PlayerStatusClassImage"])
    expect(first.filter((panel) => panel.name === "PlayerStatusClassImage")).toHaveLength(1)
    expect(first.find((panel) => panel.name === "PlayerStatusClassImage")?.state.image).toBe("../hud/class_soldierred")
    expect(first.filter((panel) => panel.name === "HudWeaponAmmo")).toHaveLength(1)
    expect(first.find((panel) => panel.name === "HudWeaponAmmo")?.state.scalarProperties).toMatchObject({ reloadPhase: 0 })
    expect(first.find((panel) => panel.name === "CarryingWeapon")?.effectivelyVisible).toBe(false)
    const hudElement = (name: string) => descendants(root).find((element) => element.dataset.vguiName === name)!
    expect(hudElement("AmmoInClip").style.color).toBe("rgba(235, 226, 202, 1)")
    expect(hudElement("AmmoInClipShadow").style.color).toBe("rgba(46, 43, 42, 1)")
    expect(hudElement("AmmoInReserve").style.color).toBe("rgba(235, 226, 202, 1)")
    expect(hudElement("AmmoInReserveShadow").style.color).toBe("rgba(0, 0, 0, 0.7686274509803922)")
    expect(first.filter((panel) => /scout|spy|unknown/iu.test(`${panel.name}:${panel.state.image ?? ""}`) && panel.effectivelyVisible)).toEqual([])
    expect(first.filter((panel) => /bleed|milk|marked|slowed|gas|resist|buff|rune|parachute|wheel|skull|poison/iu.test(panel.name) && panel.effectivelyVisible)).toEqual([])
    expect(first.find((panel) => panel.name === "PlayerStatusClassImageBG")?.state.image).toBe("../hud/character_red_bg")
    expect(first.find((panel) => panel.name === "HudWeaponAmmoBG")?.state.image).toBe("../hud/ammo_red_bg")
    expect(first.find((panel) => panel.name === "HudWeaponAmmo")?.resourceOwner).toBeNull()
    expect(first.find((panel) => panel.name === "HudWeaponAmmoBG")?.resourceOwner ?? "").toContain("hudammoweapons.res")
    hud.setPlayerClassUsePlayerModel(true)
    expect(visible(hud.snapshot().vgui.panels, ["PlayerStatusClassImage", "classmodelpanel"])).toEqual(["classmodelpanel"])
    expect(hud.snapshot().binding?.facts.player).toMatchObject({ value: { playerClassUsePlayerModel: true } })
    hud.setPlayerClassUsePlayerModel(false)
    expect(visible(hud.snapshot().vgui.panels, ["PlayerStatusClassImage", "classmodelpanel"])).toEqual(["PlayerStatusClassImage"])

    const secondBinding = hud.publish(compact(2n, 4, 3, 3, 7, 24, 2), context)
    const second = hud.snapshot().vgui.panels
    expect(second.find((panel) => panel.name === "PlayerStatusClassImage")?.state.image).toBe("../hud/class_demoblue")
    expect(second.filter((panel) => /class_.*red/iu.test(panel.state.image ?? "") && panel.effectivelyVisible)).toEqual([])
    expect(second.filter((panel) => panel.name === "HudWeaponAmmo" && panel.effectivelyVisible)).toHaveLength(1)
    expect(second.find((panel) => panel.name === "HudWeaponAmmoBG")?.state.image).toBe("../hud/ammo_blue_bg")
    expect(second.find((panel) => panel.name === "HudWeaponAmmo")?.state.scalarProperties.reloadPhase).toBe(2)
    expect(secondBinding.values).toContainEqual({ kind: "dialog-variable", panel: "classmodelpanel", variable: "weaponName", value: { kind: "available", value: "Stickybomb Launcher" } })

    const modelBinding = hud.publish(compact(3n, 3, 3, 2, 4, 20), contextWithModel(true))
    const model = hud.snapshot().vgui.panels
    expect(visible(model, ["PlayerStatusClassImage", "classmodelpanel"])).toEqual(["classmodelpanel"])
    expect(model.find((panel) => panel.name === "classmodelpanel")?.state.scalarProperties).toMatchObject({ class: 3, team: 3, skin: 1, weaponIdentity: 2 })
    expect(model.find((panel) => panel.name === "classmodelpanelBG")?.state.image).toBe("../hud/character_blue_bg_clipped")
    expect(modelBinding.values).toContainEqual({ kind: "dialog-variable", panel: "classmodelpanel", variable: "modelIdentity", value: { kind: "available", value: "models/player/soldier.mdl" } })
    expect(modelBinding.values).toContainEqual({ kind: "dialog-variable", panel: "classmodelpanel", variable: "weaponName", value: { kind: "available", value: "Original" } })

    hud.publish(compact(4n, 3, 2, 1, 4, 20), contextWithModel(false))
    expect(visible(hud.snapshot().vgui.panels, ["PlayerStatusClassImage", "classmodelpanel"])).toEqual(["PlayerStatusClassImage"])

    const remaining = [
      { tick: 5n, classIdentity: 3 as const, team: 2 as const, model: true, image: "../hud/class_soldierred", identity: "models/player/soldier.mdl", skin: 0 },
      { tick: 6n, classIdentity: 3 as const, team: 3 as const, model: false, image: "../hud/class_soldierblue", identity: "models/player/soldier.mdl", skin: 1 },
      { tick: 7n, classIdentity: 4 as const, team: 2 as const, model: false, image: "../hud/class_demored", identity: "models/player/demo.mdl", skin: 0 },
      { tick: 8n, classIdentity: 4 as const, team: 2 as const, model: true, image: "../hud/class_demored", identity: "models/player/demo.mdl", skin: 0 },
      { tick: 9n, classIdentity: 4 as const, team: 3 as const, model: true, image: "../hud/class_demoblue", identity: "models/player/demo.mdl", skin: 1 },
    ]
    for (const item of remaining) {
      const binding = hud.publish(compact(item.tick, item.classIdentity, item.team, item.classIdentity === 3 ? 1 : 3, 4, 20), contextWithModel(item.model))
      const panels = hud.snapshot().vgui.panels
      expect(visible(panels, ["PlayerStatusClassImage", "classmodelpanel"]), `${item.classIdentity}:${item.team}:${item.model}`).toEqual([item.model ? "classmodelpanel" : "PlayerStatusClassImage"])
      if (item.model) {
        expect(binding.values).toContainEqual({ kind: "dialog-variable", panel: "classmodelpanel", variable: "modelIdentity", value: { kind: "available", value: item.identity } })
        expect(panels.find((panel) => panel.name === "classmodelpanel")?.state.scalarProperties.skin).toBe(item.skin)
      } else {
        expect(panels.find((panel) => panel.name === "PlayerStatusClassImage")?.state.image).toBe(item.image)
      }
    }
    hud.reset("map-replaced")
    expect(visible(hud.snapshot().vgui.panels, ["HudPlayerStatus", "HudWeaponAmmo", "PlayerStatusClassImage", "classmodelpanel"])).toEqual([])
    expect(hud.action({ kind: "select-weapon", weapon: 1 })).toEqual({ kind: "unavailable", reason: "initial" })
    hud.publish(compact(1n, 4, 2, 3, 8, 24), contextWithModel(false))
    expect(hud.snapshot().vgui.panels.find((panel) => panel.name === "PlayerStatusClassImage")?.state.image).toBe("../hud/class_demored")
    hud.reset("disconnect")
    expect(visible(hud.snapshot().vgui.panels, ["HudPlayerStatus", "HudWeaponAmmo"])).toEqual([])
  })

  test("renders authored two-flag CTF status, scores, carried intelligence, and notifications", () => {
    const root = createRoot(new FakeDocument())
    const hud = initializeTf2HudIntegration({
      root: root as unknown as HTMLElement,
      resources: resources(), viewport: { width: 1280, height: 720, devicePixelRatio: 1 }, reducedMotion: true,
      clock: { nowSeconds: () => 0 }, random: { nextUnit: () => 0.5 }, onCommand() {},
    })
    const flag = (identity: number, team: 2 | 3, status: 0 | 1 | 2, carrier: number | null = null) => Object.freeze({
      identity, team, status, carrier, previousCarrier: carrier, initialCarrier: carrier,
      disabled: false, visibleWhenDisabled: false, shotClock: false, allowOwnerPickup: true,
      trailEnabled: true, captured: false, skin: team - 2 + (status === 1 ? 3 : 0),
      returnDeadline: status === 2 ? 60 : null, maximumReturnSeconds: status === 2 ? 60 : 0,
      ownerPickupDeadline: null, configuredReturnSeconds: 60,
      position: Object.freeze([0, 0, 0]) as readonly [number, number, number],
      home: Object.freeze([0, 0, 0]) as readonly [number, number, number],
      angles: Object.freeze([0, 0, 0]) as readonly [number, number, number],
      homeAngles: Object.freeze([0, 0, 0]) as readonly [number, number, number],
      model: "models/flag/briefcase.mdl", icon: "../hud/objectives_flagpanel_carried",
      paperEffect: "player_intel_papertrail", trailEffect: "flagtrail",
    })
    const publish = (tick: bigint, red: ReturnType<typeof flag>, blue: ReturnType<typeof flag>, redCaptures: number, events: readonly any[] = []) => {
      const base = compact(tick, 3, 2, 1, 4, 20)
      const objectives = Object.freeze({ redCaptures, blueCaptures: 0, redScore: 0, blueScore: 0, captureLimit: 3, winner: null, flags: Object.freeze([red, blue]), zones: Object.freeze([]), events: Object.freeze(events) })
      const snapshot = Object.freeze({ ...base.snapshot, objectives })
      return hud.publish(Object.freeze({ snapshot, eventBatches: Object.freeze([Object.freeze({ snapshot })]) }), context)
    }
    publish(1n, flag(10, 2, 0), flag(20, 3, 0), 0)
    let panels = hud.snapshot().vgui.panels
    expect(visible(panels, ["HudObjectiveStatus", "RedFlag", "BlueFlag"])).toEqual(["HudObjectiveStatus", "BlueFlag", "RedFlag"])
    expect(panels.filter((panel) => panel.name === "StatusIcon").map((panel) => panel.state.image)).toEqual([
      "../hud/objectives_flagpanel_ico_flag_home", "../hud/objectives_flagpanel_ico_flag_home",
    ])
    expect(visible(panels, ["CarriedImage", "CaptureFlag", "NotificationPanel"])).toEqual([])
    publish(2n, flag(10, 2, 0), flag(20, 3, 1, 1), 1)
    panels = hud.snapshot().vgui.panels
    expect(visible(panels, ["RedFlag", "BlueFlag"])).toEqual([])
    expect(visible(panels, ["CarriedImage", "CaptureFlag"])).toEqual(["CarriedImage", "CaptureFlag"])
    expect(panels.find((panel) => panel.name === "CarriedImage")?.state.image).toBe("../hud/objectives_flagpanel_carried_blue")
    publish(3n, flag(10, 2, 0), flag(20, 3, 2), 1, [Object.freeze({ kind: 4, detail: 5, team: 2, flags: 0, subject: 20, player: null, auxiliary: 0, value: 0 })])
    panels = hud.snapshot().vgui.panels
    expect(panels.filter((panel) => panel.name === "StatusIcon").map((panel) => panel.state.image)).toContain("../hud/objectives_flagpanel_ico_flag_dropped")
    expect(visible(panels, ["NotificationPanel"])).toEqual(["NotificationPanel"])
    expect(panels.find((panel) => panel.name === "Notification_Label")?.text).toBe("#TF_CTF_PlayerTeamDrop")
    publish(203n, flag(10, 2, 0), flag(20, 3, 0), 1)
    expect(visible(hud.snapshot().vgui.panels, ["NotificationPanel"])).toEqual([])
    hud.destroy()
  })

  test("paints an authored, centered crosshair instead of publishing eligibility alone", () => {
    const root = createRoot(new FakeDocument())
    const hud = initializeTf2HudIntegration({
      root: root as unknown as HTMLElement,
      resources: resources(), viewport: { width: 1280, height: 720, devicePixelRatio: 1 }, reducedMotion: true,
      clock: { nowSeconds: () => 0 }, random: { nextUnit: () => 0.5 }, onCommand() {},
    })
    hud.publish(compact(1n, 3, 2, 1, 4, 20), context)
    const authoredCrosshair = descendants(root).find((element) => element.dataset.tf2Crosshair === "authored")
    expect(authoredCrosshair).toBeDefined()
    expect(authoredCrosshair!.style.left).toBe("624px")
    expect(authoredCrosshair!.style.top).toBe("344px")
    expect(authoredCrosshair!.style.width).toBe("32px")
    expect(authoredCrosshair!.style.height).toBe("32px")
    expect(authoredCrosshair!.dataset.sourceTexture).toBe("materials/sprites/crosshairs.vtf")
    expect(authoredCrosshair!.style.backgroundImage).toContain("url(")
  })

  test("replaces authored style, tint, size, viewport, suppression, and map lifecycle atomically", () => {
    const root = createRoot(new FakeDocument())
    const hud = initializeTf2HudIntegration({
      root: root as unknown as HTMLElement,
      resources: resources(), viewport: { width: 1280, height: 720, devicePixelRatio: 1 }, reducedMotion: true,
      clock: { nowSeconds: () => 0 }, random: { nextUnit: () => 0.5 }, onCommand() {},
    })
    hud.publish(compact(1n, 3, 2, 1, 4, 20), context)
    const element = descendants(root).find((candidate) => candidate.dataset.tf2Crosshair === "authored")!
    hud.setCrosshair(Object.freeze({
      ...context.crosshair,
      texture: "vgui/crosshairs/crosshair5",
      color: Object.freeze([17, 33, 65, 1]),
      scale: 48,
    }))
    expect(element.dataset.crosshairStyle).toBe("crosshair5")
    expect(element.dataset.crosshairColor).toBe("17 33 65 255")
    expect(element.dataset.sourceTextureSha256).toBe("76567689515145389b2814b403f484625b0b5cb456f0a59ef582f060b541d0e3")
    expect([element.style.left, element.style.top, element.style.width, element.style.height]).toEqual([
      "592px", "312px", "96px", "96px",
    ])
    hud.setViewport({ width: 1025, height: 769, devicePixelRatio: 2 })
    expect([element.style.left, element.style.top]).toEqual(["465px", "337px"])
    hud.setCrosshair(Object.freeze({ ...context.crosshair, paused: true }))
    expect(element.style.display).toBe("none")
    hud.setCrosshair(Object.freeze({ ...context.crosshair }))
    expect(element.style.display).toBe("block")
    expect(element.dataset.crosshairStyle).toBe("stock")
    hud.reset("map-replaced")
    expect(element.style.display).toBe("none")
    hud.publish(compact(1n, 4, 3, 3, 8, 24), context)
    expect(element.dataset.sourceTexture).toBe("materials/sprites/crosshairs.vtf")
    expect(element.style.display).toBe("block")
    hud.destroy()
    expect(root.contains(element)).toBe(false)
  })

  test("previews exact Multiplayer drafts and commits only through Apply", async () => {
    const applied: unknown[] = []
    const persisted: Uint8Array[] = []
    const settings = initializeTf2BrowserSettings({
      persistence: null,
      owners: { renderer: "available", audio: "available", input: "available", game: "available", application: "available" },
      async apply(request) {
        applied.push(request)
        return Object.freeze({ requestId: request.requestId, status: "applied" as const })
      },
    })
    const root = createRoot(new FakeDocument())
    const options = initializeTf2OptionsPresentation({
      root: root as unknown as HTMLElement,
      resources: resources(), settings,
      viewport: { width: 1280, height: 720, devicePixelRatio: 1 }, reducedMotion: true,
      clock: { nowSeconds: () => 0 }, random: { nextUnit: () => 0.5 },
      onPersistence(bytes) { persisted.push(bytes) }, onApply() {}, onVisibility() {},
    })
    options.show("multiplayer")
    const selector = options.snapshot().vgui.panels.find((candidate) => candidate.name === "AdvCrosshairList")!
    expect(selector.state.items.map((item) => item.text)).toEqual([
      "None", "crosshair1", "crosshair2", "crosshair3", "crosshair4", "crosshair5", "crosshair6", "crosshair7", "default",
    ])
    const preview = descendants(root).find((candidate) => candidate.dataset.tf2Crosshair === "preview")!
    expect(preview.style.display).toBe("none")
    options.set("multiplayer.crosshair-file", "crosshair1")
    options.set("multiplayer.crosshair-red", 32)
    options.set("multiplayer.crosshair-green", 64)
    options.set("multiplayer.crosshair-blue", 128)
    options.set("multiplayer.crosshair-scale", 48)
    expect(preview.style.display).toBe("block")
    expect(preview.dataset.crosshairStyle).toBe("crosshair1")
    expect(preview.dataset.crosshairColor).toBe("32 64 128 255")
    expect([preview.style.left, preview.style.top, preview.style.width]).toEqual(["0px", "0px", "64px"])
    expect(settings.snapshot().settings.current).toMatchObject({
      "multiplayer.crosshair-file": "", "multiplayer.crosshair-red": 200,
    })
    expect(preview.dataset.sourceFrame).toBe("0")
    options.frame(0.2)
    expect(preview.dataset.sourceFrame).toBe("1")
    options.hide("cancel")
    expect(settings.snapshot().settings.current["multiplayer.crosshair-file"]).toBe("")
    expect(applied).toEqual([])
    options.show("multiplayer")
    expect(preview.style.display).toBe("none")
    options.set("multiplayer.crosshair-file", "crosshair7")
    options.set("multiplayer.crosshair-red", 9)
    const result = await options.apply()
    expect(result.lastApply?.complete).toBe(true)
    expect(settings.snapshot().settings.current).toMatchObject({
      "multiplayer.crosshair-file": "crosshair7", "multiplayer.crosshair-red": 9,
    })
    expect(applied).toHaveLength(1)
    expect(persisted).toHaveLength(1)
  })

  test("Escape exposes only pause controls and waits for Resume/Disconnect owner acknowledgement", () => {
    const requests: unknown[] = []
    const gameui = initializeTf2GameUiIntegration({
      root: createRoot(new FakeDocument()) as unknown as HTMLElement,
      resources: resources(), viewport: { width: 1280, height: 720, devicePixelRatio: 1 }, reducedMotion: true,
      clock: { nowSeconds: () => 0 }, random: { nextUnit: () => 0.5 },
      presentation: {
        random: {
          nextUnit: () => 0,
          nextInteger: (minimum: number) => minimum,
          snapshot: () => Object.freeze({ seed: 0, state: 0, current: 0, shuffle: Object.freeze([]), draws: 0 }),
          restore() {},
        },
        activeHoliday: "none", activeWar: null, activeOperation: false, freeTrial: false,
      },
      onRequest: (request) => requests.push(request),
    })
    gameui.dispatch({ kind: "loading-started", mapIdentity: "jump_beef" })
    gameui.dispatch({ kind: "loading-succeeded" })
    gameui.dispatch({ kind: "gameui-activated" })
    const pause = gameui.snapshot().panels
    expect(visible(pause, ["FindAGameButton", "ResumeButton", "DisconnectButton", "QuitButton", "CancelButton"]).sort()).toEqual(["DisconnectButton", "FindAGameButton", "ResumeButton"])
    expect(pause.filter((panel) => panel.name === "MainMenuOverride" && panel.effectivelyVisible)).toHaveLength(1)
    expect(visible(pause, ["TFCharacterImage"])).toEqual([])
    expect(visible(pause, ["CharacterSetupButton", "SettingsButton", "TF2SettingsButton", "NewUserForumsButton"]).sort()).toEqual(["NewUserForumsButton", "SettingsButton", "TF2SettingsButton"])
    const pauseButton = (name: string) => pause.find((panel) => panel.name === name)!.bounds
    expect(pauseButton("ResumeButton").x + pauseButton("ResumeButton").width).toBeLessThan(pauseButton("FindAGameButton").x)
    expect(pauseButton("FindAGameButton").x + pauseButton("FindAGameButton").width).toBeLessThan(pauseButton("DisconnectButton").x)
    expect(gameui.dispatch({ kind: "gameui-activated" })).toMatchObject({ disposition: "illegal", state: { kind: "pause" }, request: null })
    expect(gameui.dispatch({ kind: "teardown-confirmed" })).toMatchObject({ disposition: "illegal", state: { kind: "pause" }, request: null })
    gameui.dispatch({ kind: "activate-button", button: "resume" })
    expect(gameui.state().kind).toBe("pause")
    expect(requests.at(-1)).toEqual({ kind: "resume-game" })
    gameui.dispatch({ kind: "gameui-hidden" })
    expect(gameui.state().kind).toBe("in-game")
    expect(gameui.dispatch({ kind: "gameui-hidden" })).toMatchObject({ disposition: "illegal", state: { kind: "in-game" }, request: null })
    gameui.dispatch({ kind: "gameui-activated" })
    gameui.dispatch({ kind: "activate-button", button: "disconnect" })
    expect(gameui.state().kind).toBe("disconnecting")
    expect(requests.at(-1)).toEqual({ kind: "disconnect" })
    expect(gameui.snapshot().panels.find((panel) => panel.id === gameui.snapshot().rootPanel)?.visible).toBe(false)
    gameui.dispatch({ kind: "teardown-confirmed" })
    expect(gameui.state().kind).toBe("main-menu")
    expect(visible(gameui.snapshot().panels, ["QuitButton", "DisconnectButton", "ResumeButton"])).toEqual(["QuitButton"])
  })
})
