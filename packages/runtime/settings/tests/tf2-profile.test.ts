import { describe, expect, test } from "bun:test"
import {
  SOURCE_CONVAR_FLAGS,
  TF2_KEYBOARD_ACTIONS,
  TF2_OPTIONS_AUTHORITY,
  TF2_SELECTED_OPTIONS,
  createSettingsState,
  decodeSettingsPersistence,
  encodeSettingsPersistence,
} from "../src"

describe("configured TF2 Options profile", () => {
  test("retains fixed SDK, content, page, and physical-profile identities", () => {
    expect(TF2_OPTIONS_AUTHORITY).toMatchObject({
      sdkRevision: "88fa198fba3fb85d46d4c95018254693fdc3af0a",
      publicBuild: 24_207_079,
      patch: 10_822_003,
      optionsScript: { settings: 88, sha256: "d8f4cf14cebc84082b555fe4277f05aa5959cbd2b2e9f920252f43fe70b6b020" },
      actionList: { actions: 65, sha256: "403843357e1e3dd4212fba5f4d253db92096531d224595a47eadc07cb6b1f8da" },
    })
    expect(TF2_SELECTED_OPTIONS.identity).toBe("tf2.options.build-24207079.patch-10822003")
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
