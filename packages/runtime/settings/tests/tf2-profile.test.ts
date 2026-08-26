import { describe, expect, test } from "bun:test"
import {
  SOURCE_CONVAR_FLAGS,
  TF2_KEYBOARD_ACTIONS,
  TF2_OPTIONS_AUTHORITY,
  TF2_SELECTED_OPTIONS,
  TF2_BALANCED_VIDEO_SETTINGS,
  tf2VideoConfiguration,
  tf2VideoConvars,
  tf2VideoSettingsFromConvars,
  createSettingsState,
  decodeSettingsPersistence,
  encodeSettingsPersistence,
} from "../src"

describe("configured TF2 Options profile", () => {
  test("maps the balanced video preset to its complete effective Source configuration", () => {
    const state = createSettingsState({ catalog: TF2_SELECTED_OPTIONS })
    expect(state.snapshot().current).toMatchObject(TF2_BALANCED_VIDEO_SETTINGS)
    expect(tf2VideoConvars(tf2VideoConfiguration(state.snapshot().current))).toEqual({
      r_rootlod: 1, mat_picmip: 1, r_shadowrendertotexture: 1,
      r_flashlightdepthtexture: 0, mat_reducefillrate: 1, mat_hdr_level: 0,
      mat_antialias: 0, mat_aaquality: 0, mat_trilinear: 0, mat_forceaniso: 1,
      r_waterforceexpensive: 0, r_waterforcereflectentities: 0, mat_vsync: 0,
      mat_queue_mode: -1, mat_colorcorrection: 0, mat_motion_blur_enabled: 0,
    })
  })

  test("maps every selectable model, texture, shadow, filtering, water, and HDR quality", () => {
    for (const [value, rootLod] of [["low", 2], ["medium", 1], ["high", 0]] as const)
      expect(tf2VideoConfiguration({ "video.model-detail": value }).rootLod).toBe(rootLod)
    for (const [value, picmip] of [["low", 2], ["medium", 1], ["high", 0], ["ultra", -1]] as const)
      expect(tf2VideoConfiguration({ "video.texture-detail": value }).picmip).toBe(picmip)
    for (const [value, render, depth] of [["low", 0, 0], ["medium", 1, 0], ["high", 1, 1]] as const)
      expect(tf2VideoConfiguration({ "video.shadow-detail": value })).toMatchObject({ shadowRenderToTexture: render, flashlightDepthTexture: depth })
    for (const [value, anisotropy] of [["bilinear", 1], ["trilinear", 1], ["anisotropic-2", 2], ["anisotropic-4", 4], ["anisotropic-8", 8], ["anisotropic-16", 16]] as const)
      expect(tf2VideoConfiguration({ "video.filtering": value })).toMatchObject({ anisotropy, trilinear: Number(value === "trilinear") })
    for (const [value, expensive, entities] of [["no-reflections", 0, 0], ["reflect-world", 1, 0], ["reflect-all", 1, 1]] as const)
      expect(tf2VideoConfiguration({ "video.water-detail": value })).toMatchObject({ waterForceExpensive: expensive, waterReflectEntities: entities })
    for (const level of [0, 1, 2] as const) expect(tf2VideoConfiguration({ "video.hdr": level }).hdrLevel).toBe(level)
    expect(() => tf2VideoConfiguration({ "video.antialias-samples": 2 })).toThrow("unsupported")
    const configuration = tf2VideoConfiguration(TF2_BALANCED_VIDEO_SETTINGS)
    expect(tf2VideoSettingsFromConvars(configuration, { r_rootlod: 2, mat_picmip: -1, mat_forceaniso: 8 }))
      .toMatchObject({ "video.model-detail": "low", "video.texture-detail": "ultra", "video.filtering": "anisotropic-8" })
    expect(() => tf2VideoSettingsFromConvars(configuration, { mat_forceaniso: 3 })).toThrow("filtering")
  })
  test("retains fixed SDK, content, page, and physical-profile identities", () => {
    expect(TF2_OPTIONS_AUTHORITY).toMatchObject({
      sdkRevision: "88fa198fba3fb85d46d4c95018254693fdc3af0a",
      publicBuild: 24_245_096,
      patch: 10_828_683,
      optionsScript: { settings: 88, sha256: "d8f4cf14cebc84082b555fe4277f05aa5959cbd2b2e9f920252f43fe70b6b020" },
      actionList: { actions: 65, sha256: "403843357e1e3dd4212fba5f4d253db92096531d224595a47eadc07cb6b1f8da" },
    })
    expect(TF2_SELECTED_OPTIONS.identity).toBe("tf2.options.build-24245096.patch-10828683")
    expect(TF2_SELECTED_OPTIONS.settings).toHaveLength(203)
    expect(TF2_SELECTED_OPTIONS.bindingProfile).toMatchObject({
      identity: "tf2.desktop-keyboard-mouse",
      conflictPolicy: "replace",
      modifierMask: 7,
    })
    expect(TF2_SELECTED_OPTIONS.bindingProfile?.inputs).toHaveLength(110)
  })

  test("covers the exact 88 Advanced records by configured value kind", () => {
    const advanced = TF2_SELECTED_OPTIONS.settings.filter((setting) => setting.page === "advanced")
    const counts = new Map<string, number>()
    for (const setting of advanced) counts.set(setting.kind, (counts.get(setting.kind) ?? 0) + 1)
    expect(advanced).toHaveLength(88)
    expect(Object.fromEntries(counts)).toEqual({ boolean: 55, float: 13, enum: 12, integer: 5, string: 3 })
    expect(advanced.find((setting) => setting.id === "viewmodel_fov")).toMatchObject({
      kind: "float",
      defaultValue: 54,
      minimum: 54,
      maximum: 70,
    })
    expect(advanced.find((setting) => setting.id === "tf_contract_competitive_show")).toMatchObject({
      kind: "enum",
      defaultValue: 2,
    })
    expect(advanced.find((setting) => setting.id === "replay_maxscreenshotsperreplay")).toMatchObject({
      kind: "integer",
      defaultValue: 8,
      minimum: 1,
      maximum: 1_000,
    })
    expect(advanced.find((setting) => setting.id === "hud_combattext_green")).toMatchObject({
      kind: "float",
      defaultValue: 0,
      minimum: 1,
      maximum: 255,
    })
  })

  test("keeps declaration defaults separate from page coercions and visibility", () => {
    const byName = (name: string) => TF2_SELECTED_OPTIONS.convars.find((convar) => convar.name === name)
    expect(byName("voice_modenable")).toMatchObject({ defaultValue: "1", flags: SOURCE_CONVAR_FLAGS.ARCHIVE | SOURCE_CONVAR_FLAGS.CLIENT_COMMAND_CAN_EXECUTE })
    expect(byName("hud_combattext")).toMatchObject({ defaultValue: "1" })
    expect(byName("tf_scoreboard_mouse_mode")).toMatchObject({ defaultValue: "2" })
    expect(byName("replay_enableeventbasedscreenshots")).toMatchObject({ defaultValue: "0" })
    expect(byName("cl_cloud_settings")).toMatchObject({ defaultValue: "1", visibility: "hidden" })
    expect(TF2_SELECTED_OPTIONS.settings.find((setting) => setting.id === "cl_cloud_settings"))
      .toMatchObject({ kind: "boolean", defaultValue: true, visibility: "visible" })
  })

  test("retains archived unbounded TF2 crosshair declarations and exact Multiplayer edit ranges", () => {
    for (const [name, defaultValue] of [
      ["cl_crosshair_red", "200"],
      ["cl_crosshair_green", "200"],
      ["cl_crosshair_blue", "200"],
      ["cl_crosshair_scale", "32.0"],
      ["cl_crosshair_file", ""],
    ] as const) {
      const convar = TF2_SELECTED_OPTIONS.convars.find((candidate) => candidate.name === name)
      expect(convar).toMatchObject({ defaultValue, flags: SOURCE_CONVAR_FLAGS.ARCHIVE })
      expect(convar?.minimum).toBeUndefined()
      expect(convar?.maximum).toBeUndefined()
    }
    for (const channel of ["red", "green", "blue"]) {
      expect(TF2_SELECTED_OPTIONS.settings.find((candidate) => candidate.id === `multiplayer.crosshair-${channel}`))
        .toMatchObject({ kind: "float", defaultValue: 200, minimum: 0, maximum: 255, owner: "game" })
    }
    expect(TF2_SELECTED_OPTIONS.settings.find((candidate) => candidate.id === "multiplayer.crosshair-scale"))
      .toMatchObject({ kind: "float", defaultValue: 32, minimum: 16, maximum: 48, owner: "game" })
    expect(TF2_SELECTED_OPTIONS.settings.find((candidate) => candidate.id === "multiplayer.crosshair-file"))
      .toMatchObject({ kind: "string", defaultValue: "", owner: "game" })
  })

  test("retains all displayed keyboard actions and exact known defaults", () => {
    const bindings = TF2_SELECTED_OPTIONS.settings.filter((setting) => setting.kind === "binding")
    expect(bindings).toHaveLength(65)
    expect(bindings.map((binding) => binding.action)).toEqual([...TF2_KEYBOARD_ACTIONS])
    expect(bindings.find((binding) => binding.action === "+jump")).toMatchObject({ defaultValue: { code: "SPACE", modifiers: 0 } })
    expect(bindings.find((binding) => binding.action === "dropitem")).toMatchObject({ defaultValue: { code: "l", modifiers: 0 } })
    expect(bindings.find((binding) => binding.action === "quit")).toMatchObject({ defaultValue: null })
    expect(bindings.find((binding) => binding.action === "toggleconsole")).toMatchObject({ defaultValue: { code: "`", modifiers: 0 } })
  })

  test("round-trips the complete default profile with separate volume and mute", () => {
    const state = createSettingsState({ catalog: TF2_SELECTED_OPTIONS })
    expect(state.snapshot().current).toMatchObject({
      "audio.effect-volume": 1,
      "audio.music-volume": 1,
      "audio.master-muted": false,
    })
    const bytes = encodeSettingsPersistence(TF2_SELECTED_OPTIONS, state.snapshot().current)
    const decoded = decodeSettingsPersistence(TF2_SELECTED_OPTIONS, bytes)
    expect(decoded).toMatchObject({ ok: true, decoded: { values: {
      "audio.effect-volume": 1,
      "audio.music-volume": 1,
      "audio.master-muted": false,
    } } })
  })
})
