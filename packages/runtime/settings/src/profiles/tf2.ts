import {
  SOURCE_CONVAR_FLAGS as F,
  SOURCE_KEY_MODIFIERS as M,
  type BindingValue,
  type EnumValue,
  type RestartDisposition,
  type SettingOwner,
  type SettingSchema,
  type SourceConCommandContract,
  type SourceConVarContract,
} from "../contract"
import { defineSettingsCatalog } from "../schema"

export const TF2_OPTIONS_AUTHORITY = Object.freeze({
  sdkRevision: "88fa198fba3fb85d46d4c95018254693fdc3af0a",
  publicBuild: 24_245_096,
  patch: 10_828_683,
  optionsScript: Object.freeze({
    logicalPath: "cfg/user_default.scr",
    bytes: 13_979,
    sha256: "d8f4cf14cebc84082b555fe4277f05aa5959cbd2b2e9f920252f43fe70b6b020",
    settings: 88,
  }),
  actionList: Object.freeze({
    logicalPath: "scripts/kb_act.lst",
    bytes: 3_305,
    sha256: "403843357e1e3dd4212fba5f4d253db92096531d224595a47eadc07cb6b1f8da",
    actions: 65,
  }),
  multiplayerResource: Object.freeze({
    logicalPath: "resource/optionssubmultiplayer.res",
    bytes: 21_251,
    sha256: "40cacfeaff5f0304edbbdfe03a970dddb786a6c4ad8c9492edafae556a00c19e",
  }),
} as const)

const convars: SourceConVarContract[] = []
const commands: SourceConCommandContract[] = []
const settings: SettingSchema[] = []
const convarNames = new Map<string, SourceConVarContract>()

function consoleVisibility(flags: number): SourceConVarContract["visibility"] {
  if ((flags & F.HIDDEN) !== 0) return "hidden"
  if ((flags & F.DEVELOPMENT_ONLY) !== 0) return "development"
  return "visible"
}

function convar(
  name: string,
  defaultValue: string,
  flags: number,
  help = "",
  minimum?: number,
  maximum?: number,
): void {
  const existing = convarNames.get(name.toLowerCase())
  if (existing) {
    if (existing.defaultValue !== defaultValue || existing.flags !== flags) {
      throw new Error(`conflicting TF2 selected convar ${name}`)
    }
    return
  }
  const value = Object.freeze({
    name,
    defaultValue,
    help,
    flags,
    visibility: consoleVisibility(flags),
    ...(minimum !== undefined ? { minimum } : {}),
    ...(maximum !== undefined ? { maximum } : {}),
  })
  convarNames.set(name.toLowerCase(), value)
  convars.push(value)
}

function common(
  id: string,
  page: string,
  owner: SettingOwner,
  flags: number,
  consoleNames: readonly string[],
  restart: RestartDisposition = "live",
) {
  return {
    id,
    page,
    owner,
    flags,
    visibility: "visible" as const,
    restart,
    persistence: "persistent" as const,
    consoleNames,
  }
}

function booleanSetting(
  name: string,
  defaultValue: boolean,
  flags: number,
  page = "advanced",
  owner: SettingOwner = "game",
  rawDefault = defaultValue ? "1" : "0",
): void {
  convar(name, rawDefault, flags)
  settings.push(Object.freeze({ kind: "boolean", ...common(name, page, owner, flags, [name]), defaultValue }))
}

function floatSetting(
  name: string,
  defaultValue: number,
  minimum: number,
  maximum: number,
  flags: number,
  page = "advanced",
  owner: SettingOwner = "game",
  rawDefault = String(defaultValue),
): void {
  convar(name, rawDefault, flags, "", minimum, maximum)
  settings.push(Object.freeze({ kind: "float", ...common(name, page, owner, flags, [name]), defaultValue, minimum, maximum }))
}

function integerSetting(
  name: string,
  defaultValue: number,
  minimum: number,
  maximum: number,
  flags: number,
  page = "advanced",
  owner: SettingOwner = "game",
  rawDefault = String(defaultValue),
): void {
  convar(name, rawDefault, flags, "", minimum, maximum)
  settings.push(Object.freeze({ kind: "integer", ...common(name, page, owner, flags, [name]), defaultValue, minimum, maximum }))
}

function enumSetting(
  name: string,
  defaultValue: EnumValue,
  values: readonly EnumValue[],
  flags: number,
  page = "advanced",
  owner: SettingOwner = "game",
  rawDefault = String(defaultValue),
): void {
  convar(name, rawDefault, flags)
  settings.push(Object.freeze({
    kind: "enum",
    ...common(name, page, owner, flags, [name]),
    defaultValue,
    options: Object.freeze(values.map((value) => Object.freeze({ value, label: String(value) }))),
  }))
}

function stringSetting(
  name: string,
  defaultValue: string,
  maximumUtf8Bytes: number,
  flags: number,
  page = "advanced",
  owner: SettingOwner = "application",
): void {
  convar(name, defaultValue, flags)
  settings.push(Object.freeze({
    kind: "string",
    ...common(name, page, owner, flags, [name]),
    defaultValue,
    minimumUtf8Bytes: 0,
    maximumUtf8Bytes,
  }))
}

const A = F.ARCHIVE
const AX = A | F.ARCHIVE_XBOX
const AU = A | F.USER_INFO
const AUX = A | F.USER_INFO | F.ARCHIVE_XBOX
const AC = A | F.CLIENT_DLL
const ACU = A | F.CLIENT_DLL | F.USER_INFO
const ACD = A | F.CLIENT_DLL | F.DO_NOT_RECORD
const D = F.DO_NOT_RECORD

// Configured TF2 Advanced page: 55 BOOL records.
booleanSetting("voice_modenable", true, A | F.CLIENT_COMMAND_CAN_EXECUTE)
booleanSetting("cl_enable_text_chat", true, A)
booleanSetting("cl_autoreload", true, AU)
booleanSetting("hud_fastswitch", false, AX)
booleanSetting("tf_dingalingaling", false, A)
booleanSetting("tf_dingalingaling_lasthit", false, A)
booleanSetting("hud_combattext", true, AUX)
booleanSetting("hud_combattext_batching", false, AUX)
booleanSetting("hud_combattext_doesnt_block_overhead_text", true, AU)
booleanSetting("tf_remember_activeweapon", false, ACU)
booleanSetting("tf_remember_lastswitched", false, ACU)
booleanSetting("tf_sniper_fullcharge_bell", false, A)
booleanSetting("tf_simple_disguise_menu", false, A, "advanced", "game", "")
booleanSetting("cl_autorezoom", true, AU)
booleanSetting("tf_hud_no_crosshair_on_scope_zoom", false, AC)
booleanSetting("tf_medigun_autoheal", false, ACU)
booleanSetting("hud_medichealtargetmarker", false, AX)
booleanSetting("hud_medicautocallers", false, AX)
booleanSetting("cl_hud_minmode", false, A)
booleanSetting("tf_colorblindassist", false, AC)
booleanSetting("cl_use_tournament_specgui", false, A)
booleanSetting("cl_spec_carrieditems", true, A)
booleanSetting("glow_outline_effect_enable", true, A, "advanced", "renderer")
booleanSetting("tf_enable_glows_after_respawn", true, A)
booleanSetting("cl_hud_playerclass_use_playermodel", true, A)
booleanSetting("hud_freezecamhide", false, AC)
booleanSetting("tf_spectate_pyrovision", false, A)
booleanSetting("tf_romevision_opt_in", false, A)
booleanSetting("tf_hud_target_id_disable_floating_health", false, A)
booleanSetting("tf_scoreboard_mouse_mode", true, A, "advanced", "game", "2")
booleanSetting("tf_scoreboard_ping_as_text", false, A)
booleanSetting("tf_scoreboard_alt_class_icons", false, A)
booleanSetting("tf_use_match_hud", true, A)
booleanSetting("replay_enableeventbasedscreenshots", false, D | A, "advanced", "application")
booleanSetting("replay_screenshotresolution", false, D, "advanced", "application")
booleanSetting("tf_replay_pyrovision", false, A)
booleanSetting("tf_particles_disable_weather", false, A, "advanced", "renderer")
booleanSetting("cl_disablehtmlmotd", false, A, "advanced", "application")
booleanSetting("hud_takesshots", false, AC, "advanced", "application")
booleanSetting("hud_classautokill", true, ACU)
booleanSetting("tf_respawn_on_loadoutchanges", true, A)
booleanSetting("cl_flipviewmodels", false, AU | F.NOT_CONNECTED)
booleanSetting("tf_use_min_viewmodels", false, A)
booleanSetting("cl_spraydisable", true, AC)
booleanSetting("tf_hide_custom_decals", false, A)
booleanSetting("tf_delete_temp_files", true, AC, "advanced", "application")
booleanSetting("sb_close_browser_on_connect", true, A, "advanced", "application")
booleanSetting("cl_cloud_settings", true, F.HIDDEN, "advanced", "application")
booleanSetting("cl_steamscreenshots", true, A, "advanced", "application")
booleanSetting("cl_notifications_show_ingame", true, A, "advanced", "application")
booleanSetting("cl_promotional_codes_button_show", true, A, "advanced", "application")
booleanSetting("ds_sound", true, ACD, "advanced", "application")
booleanSetting("ds_log", true, ACD, "advanced", "application")
booleanSetting("ds_screens", true, ACD, "advanced", "application")
booleanSetting("ds_autodelete", false, ACD, "advanced", "application")

// Configured TF2 Advanced page: 13 SLIDER records.
floatSetting("tf_dingaling_volume", 0.75, 0, 1, A)
floatSetting("tf_dingaling_pitchmindmg", 100, 1, 255, A)
floatSetting("tf_dingaling_pitchmaxdmg", 100, 1, 255, A)
floatSetting("tf_dingaling_lasthit_volume", 0.75, 0, 1, A)
floatSetting("tf_dingaling_lasthit_pitchmindmg", 100, 1, 255, A)
floatSetting("tf_dingaling_lasthit_pitchmaxdmg", 100, 1, 255, A)
floatSetting("hud_combattext_red", 255, 1, 255, AUX)
floatSetting("hud_combattext_green", 0, 1, 255, AUX)
floatSetting("hud_combattext_blue", 0, 1, 255, AUX)
floatSetting("hud_medicautocallersthreshold", 75, 10, 99, AX)
floatSetting("viewmodel_fov", 54, 54, 70, A)
floatSetting("tf_hud_target_id_alpha", 100, 0, 255, A)
floatSetting("replay_postdeathrecordtime", 5, 0, 10, D, "advanced", "application")

// Configured TF2 Advanced page: 12 LIST records.
enumSetting("tf_dingalingaling_effect", 0, [0, 1, 2, 3, 4, 5, 6, 7, 8], A)
enumSetting("tf_dingalingaling_last_effect", 0, [0, 1, 2, 3, 4, 5, 6, 7, 8], A)
enumSetting("tf_spectator_target_location", 0, [0, 1, 2, 3], A)
enumSetting("pyro_vignette", 2, [0, 1, 2], A, "advanced", "renderer")
enumSetting("pyro_vignette_distortion", 1, [0, 1], A, "advanced", "renderer")
enumSetting("pyro_dof", 1, [0, 1], A, "advanced", "renderer")
enumSetting("tf_contract_progress_show", 1, [0, 1, 2], ACD)
enumSetting("tf_contract_competitive_show", 2, [0, 1, 2], ACD)
enumSetting("cl_trading_show_requests_from", 3, [1, 2, 3, 4], A, "advanced", "application")
enumSetting("cl_show_market_data_on_items", 1, [0, 1, 2], A, "advanced", "application")
enumSetting("ds_enable", 0, [0, 1, 2, 3], ACD, "advanced", "application")
enumSetting("ds_notify", 0, [0, 1, 2], ACD, "advanced", "application")

// Configured TF2 Advanced page: five NUMBER and three STRING records.
integerSetting("replay_maxscreenshotsperreplay", 8, 1, 1_000, D, "advanced", "application")
integerSetting("replay_mintimebetweenscreenshots", 5, 1, 1_000, D, "advanced", "application")
integerSetting("mp_decals", 200, 0, 4_096, A, "advanced", "renderer")
integerSetting("ds_min_streak", 4, 2, 1_000, ACD, "advanced", "application")
integerSetting("ds_kill_delay", 15, 5, 1_000, ACD, "advanced", "application")
stringSetting("youtube_http_proxy", "", 127, A)
stringSetting("ds_dir", "demos", 24, ACD)
stringSetting("ds_prefix", "", 24, ACD)

// Generic Audio page plus the target adapter's explicit mute state.
convar("volume", "1.0", AX, "Sound volume", 0, 1)
convar("snd_musicvolume", "1.0", AX, "Music volume", 0, 1)
convar("closecaption", "0", AUX, "Enable close captioning")
convar("cc_subtitles", "0", AX, "Restrict captions to voice")
convar("snd_surround_speakers", "-1", F.INTERNAL_USE, "Speaker layout")
convar("snd_pitchquality", "1", A, "High quality pitch shifting")
convar("dsp_slow_cpu", "0", A | F.DEMO, "Reduced DSP processing")
convar("dsp_enhance_stereo", "0", A, "Enhanced headphone stereo")
convar("snd_mute_losefocus", "1", A, "Mute sound while unfocused")
settings.push(
  Object.freeze({ kind: "float", ...common("audio.effect-volume", "audio", "audio", AX, ["volume"]), defaultValue: 1, minimum: 0, maximum: 1 }),
  Object.freeze({ kind: "float", ...common("audio.music-volume", "audio", "audio", AX, ["snd_musicvolume"]), defaultValue: 1, minimum: 0, maximum: 1 }),
  Object.freeze({ kind: "boolean", ...common("audio.master-muted", "audio", "audio", 0, []), defaultValue: false }),
  Object.freeze({
    kind: "enum",
    ...common("audio.caption-mode", "audio", "audio", AUX | AX, ["closecaption", "cc_subtitles"]),
    defaultValue: "none",
    options: Object.freeze(["none", "captions", "subtitles"].map((value) => Object.freeze({ value, label: value }))),
  }),
  Object.freeze({
    kind: "enum",
    ...common("audio.quality", "audio", "audio", A | F.DEMO, ["snd_pitchquality", "dsp_slow_cpu", "dsp_enhance_stereo"]),
    defaultValue: "high",
    options: Object.freeze(["low", "medium", "high"].map((value) => Object.freeze({ value, label: value }))),
  }),
  Object.freeze({
    kind: "enum",
    ...common("audio.speaker-layout", "audio", "audio", F.INTERNAL_USE, ["snd_surround_speakers", "dsp_enhance_stereo"]),
    defaultValue: 2,
    options: Object.freeze([0, 2, 4, 5, 7].map((value) => Object.freeze({ value, label: String(value) }))),
  }),
  Object.freeze({ kind: "boolean", ...common("audio.mute-while-unfocused", "audio", "audio", A, ["snd_mute_losefocus"]), defaultValue: true }),
  Object.freeze({
    kind: "string",
    ...common("audio.spoken-language", "audio", "application", 0, [], "application-restart"),
    defaultValue: "english",
    minimumUtf8Bytes: 1,
    maximumUtf8Bytes: 49,
  }),
)

// Generic Mouse page. Composite booleans preserve the exact sign/value mappings for owner adapters.
convar("m_pitch", "0.022", A, "Mouse pitch factor")
convar("m_filter", "0", A, "Average mouse input over two frames")
convar("m_rawinput", "1", A, "Use raw mouse input")
convar("m_customaccel", "0", A, "Custom mouse acceleration")
convar("m_customaccel_exponent", "1", A, "Mouse acceleration exponent", 1)
convar("joystick", "0", A, "Enable joystick input")
convar("joy_movement_stick", "0", AX, "Select movement stick")
convar("joy_inverty", "0", AX, "Invert joystick Y axis")
convar("sensitivity", "3", A, "Mouse sensitivity", 0.0001, 10_000_000)
convar("joy_yawsensitivity", "-1", AX, "Joystick yaw sensitivity")
convar("joy_pitchsensitivity", "1", AX, "Joystick pitch sensitivity")
convar("hud_quickinfo", "1", A, "Display quick information")
settings.push(
  Object.freeze({ kind: "boolean", ...common("mouse.reverse", "mouse", "input", A, ["m_pitch"]), defaultValue: false }),
  Object.freeze({ kind: "boolean", ...common("mouse.filter", "mouse", "input", A, ["m_filter"]), defaultValue: false }),
  Object.freeze({ kind: "boolean", ...common("mouse.raw-input", "mouse", "input", A, ["m_rawinput"]), defaultValue: true }),
  Object.freeze({ kind: "boolean", ...common("mouse.custom-acceleration", "mouse", "input", A, ["m_customaccel"]), defaultValue: false }),
  Object.freeze({ kind: "boolean", ...common("mouse.joystick", "mouse", "input", A, ["joystick"]), defaultValue: false }),
  Object.freeze({ kind: "boolean", ...common("mouse.joystick-southpaw", "mouse", "input", AX, ["joy_movement_stick"]), defaultValue: false }),
  Object.freeze({ kind: "boolean", ...common("mouse.reverse-joystick", "mouse", "input", AX, ["joy_inverty"]), defaultValue: false }),
  Object.freeze({ kind: "boolean", ...common("mouse.hud-quick-info", "mouse", "game", A, ["hud_quickinfo"]), defaultValue: true }),
  Object.freeze({ kind: "float", ...common("mouse.sensitivity", "mouse", "input", A, ["sensitivity"]), defaultValue: 3, minimum: 0.1, maximum: 6 }),
  Object.freeze({ kind: "float", ...common("mouse.acceleration-exponent", "mouse", "input", A, ["m_customaccel_exponent"]), defaultValue: 1, minimum: 1, maximum: 1.4 }),
  Object.freeze({ kind: "float", ...common("mouse.joystick-yaw-sensitivity", "mouse", "input", AX, ["joy_yawsensitivity"]), defaultValue: -1, minimum: -7, maximum: -0.5 }),
  Object.freeze({ kind: "float", ...common("mouse.joystick-pitch-sensitivity", "mouse", "input", AX, ["joy_pitchsensitivity"]), defaultValue: 1, minimum: 0.5, maximum: 7 }),
)

// Keyboard advanced toggles.
convar("con_enable", "0", A, "Allow developer console activation")
settings.push(Object.freeze({ kind: "boolean", ...common("keyboard.console-enabled", "keyboard", "application", A, ["con_enable"]), defaultValue: false }))

// TF2 Multiplayer page selected by gameinfo.txt and the configured resource.
convar("cl_crosshair_red", "200", A, "Crosshair red channel")
convar("cl_crosshair_green", "200", A, "Crosshair green channel")
convar("cl_crosshair_blue", "200", A, "Crosshair blue channel")
convar("cl_crosshair_scale", "32.0", A, "Crosshair scale")
convar("cl_crosshair_file", "", A, "Crosshair style")
convar("cl_logofile", "materials/decals/spraylogo.vtf", A, "Spray logo")
convar("cl_downloadfilter", "all", A, "Downloaded file filter")
settings.push(
  Object.freeze({ kind: "float", ...common("multiplayer.crosshair-red", "multiplayer", "game", A, ["cl_crosshair_red"]), defaultValue: 200, minimum: 0, maximum: 255 }),
  Object.freeze({ kind: "float", ...common("multiplayer.crosshair-green", "multiplayer", "game", A, ["cl_crosshair_green"]), defaultValue: 200, minimum: 0, maximum: 255 }),
  Object.freeze({ kind: "float", ...common("multiplayer.crosshair-blue", "multiplayer", "game", A, ["cl_crosshair_blue"]), defaultValue: 200, minimum: 0, maximum: 255 }),
  Object.freeze({ kind: "float", ...common("multiplayer.crosshair-scale", "multiplayer", "game", A, ["cl_crosshair_scale"]), defaultValue: 32, minimum: 16, maximum: 48 }),
  Object.freeze({ kind: "string", ...common("multiplayer.crosshair-file", "multiplayer", "game", A, ["cl_crosshair_file"]), defaultValue: "", minimumUtf8Bytes: 0, maximumUtf8Bytes: 255 }),
  Object.freeze({ kind: "string", ...common("multiplayer.spray-file", "multiplayer", "application", A, ["cl_logofile"]), defaultValue: "materials/decals/spraylogo.vtf", minimumUtf8Bytes: 0, maximumUtf8Bytes: 511 }),
  Object.freeze({
    kind: "enum",
    ...common("multiplayer.download-filter", "multiplayer", "application", A, ["cl_downloadfilter"]),
    defaultValue: "all",
    options: Object.freeze(["all", "nosounds", "mapsonly", "none"].map((value) => Object.freeze({ value, label: value }))),
  }),
)

// Static Video/Advanced controls. Display modes, supported AA modes, adapters, and audio languages remain owner-provided live choices.
convar("mat_monitorgamma", "2.2", A, "Monitor gamma", 1.6, 2.6)
convar("mat_dxlevel", "0", 0, "Current DirectX support level")
convar("fov_desired", "75", AU, "Base field of view", 20, 90)
convar("r_rootlod", "1", A | F.MATERIAL_SYSTEM_THREAD, "Root model LOD", 0, 2)
convar("mat_picmip", "1", A, "Texture detail", -1, 4)
convar("mat_trilinear", "0", F.ALLOWED_IN_COMPETITIVE, "Trilinear filtering")
convar("mat_forceaniso", "1", A, "Anisotropic filtering")
convar("mat_antialias", "0", A, "Antialias sample count")
convar("mat_aaquality", "0", A, "Antialias quality")
convar("r_shadowrendertotexture", "1", A, "Render-to-texture shadows")
convar("r_flashlightdepthtexture", "0", F.ALLOWED_IN_COMPETITIVE, "Depth texture shadows")
convar("mat_reducefillrate", "1", F.ALLOWED_IN_COMPETITIVE, "Reduced shader detail")
convar("mat_hdr_level", "0", A, "HDR level")
convar("r_waterforceexpensive", "0", A, "Realtime water reflection")
convar("r_waterforcereflectentities", "0", F.ALLOWED_IN_COMPETITIVE, "Reflect entities in water")
convar("mat_vsync", "0", F.ALLOWED_IN_COMPETITIVE, "Vertical synchronization", 0, 1)
convar("mat_queue_mode", "-1", A, "Material queue mode")
convar("mat_colorcorrection", "0", A, "Color correction")
convar("mat_motion_blur_enabled", "0", A, "Motion blur")
settings.push(
  Object.freeze({ kind: "integer", ...common("video.display-width", "video", "renderer", 0, ["mat_setvideomode"]), defaultValue: 640, minimum: 0, maximum: 2_147_483_647 }),
  Object.freeze({ kind: "integer", ...common("video.display-height", "video", "renderer", 0, ["mat_setvideomode"]), defaultValue: 480, minimum: 0, maximum: 2_147_483_647 }),
  Object.freeze({ kind: "integer", ...common("video.refresh-rate", "video", "renderer", 0, []), defaultValue: 60, minimum: 0, maximum: 2_147_483_647 }),
  Object.freeze({ kind: "boolean", ...common("video.windowed", "video", "renderer", 0, ["mat_setvideomode"]), defaultValue: false }),
  Object.freeze({ kind: "boolean", ...common("video.vr-enabled", "video", "renderer", 0, ["mat_enable_vrmode"], "application-restart"), defaultValue: false }),
  Object.freeze({ kind: "boolean", ...common("video.hd-content", "video", "application", 0, [], "application-restart"), defaultValue: false }),
  Object.freeze({ kind: "integer", ...common("video.dx-level", "video", "renderer", 0, ["mat_dxlevel"], "owner-restart"), defaultValue: 0, minimum: 0, maximum: 2_147_483_647 }),
  Object.freeze({ kind: "float", ...common("video.gamma", "video", "renderer", A, ["mat_monitorgamma"]), defaultValue: 2.2, minimum: 1.6, maximum: 2.6 }),
  Object.freeze({ kind: "float", ...common("video.field-of-view", "video", "game", AU, ["fov_desired"]), defaultValue: 75, minimum: 75, maximum: 90 }),
  Object.freeze({ kind: "enum", ...common("video.model-detail", "video", "renderer", A | F.MATERIAL_SYSTEM_THREAD, ["r_rootlod"]), defaultValue: "medium", options: Object.freeze(["low", "medium", "high"].map((value) => Object.freeze({ value, label: value }))) }),
  Object.freeze({ kind: "enum", ...common("video.texture-detail", "video", "renderer", A, ["mat_picmip"]), defaultValue: "medium", options: Object.freeze(["low", "medium", "high", "ultra"].map((value) => Object.freeze({ value, label: value }))) }),
  Object.freeze({ kind: "enum", ...common("video.filtering", "video", "renderer", A, ["mat_trilinear", "mat_forceaniso"]), defaultValue: "bilinear", options: Object.freeze(["bilinear", "trilinear", "anisotropic-2", "anisotropic-4", "anisotropic-8", "anisotropic-16"].map((value) => Object.freeze({ value, label: value }))) }),
  Object.freeze({ kind: "integer", ...common("video.antialias-samples", "video", "renderer", A, ["mat_antialias"]), defaultValue: 0, minimum: 0, maximum: 8 }),
  Object.freeze({ kind: "integer", ...common("video.antialias-quality", "video", "renderer", A, ["mat_aaquality"]), defaultValue: 0, minimum: 0, maximum: 4 }),
  Object.freeze({ kind: "enum", ...common("video.shadow-detail", "video", "renderer", A, ["r_shadowrendertotexture", "r_flashlightdepthtexture"]), defaultValue: "medium", options: Object.freeze(["low", "medium", "high"].map((value) => Object.freeze({ value, label: value }))) }),
  Object.freeze({ kind: "enum", ...common("video.shader-detail", "video", "renderer", F.ALLOWED_IN_COMPETITIVE, ["mat_reducefillrate"]), defaultValue: "low", options: Object.freeze(["low", "high"].map((value) => Object.freeze({ value, label: value }))) }),
  Object.freeze({ kind: "enum", ...common("video.hdr", "video", "renderer", A, ["mat_hdr_level"], "owner-restart"), defaultValue: 0, options: Object.freeze([0, 1, 2].map((value) => Object.freeze({ value, label: String(value) }))) }),
  Object.freeze({ kind: "enum", ...common("video.water-detail", "video", "renderer", A, ["r_waterforceexpensive", "r_waterforcereflectentities"]), defaultValue: "no-reflections", options: Object.freeze(["no-reflections", "reflect-world", "reflect-all"].map((value) => Object.freeze({ value, label: value }))) }),
  Object.freeze({ kind: "boolean", ...common("video.vsync", "video", "renderer", F.ALLOWED_IN_COMPETITIVE, ["mat_vsync"]), defaultValue: false }),
  Object.freeze({ kind: "boolean", ...common("video.multicore", "video", "renderer", A, ["mat_queue_mode"]), defaultValue: true }),
  Object.freeze({ kind: "boolean", ...common("video.color-correction", "video", "renderer", A, ["mat_colorcorrection"]), defaultValue: false }),
  Object.freeze({ kind: "boolean", ...common("video.motion-blur", "video", "renderer", A, ["mat_motion_blur_enabled"]), defaultValue: false }),
)

commands.push(
  Object.freeze({ name: "bind", help: "Bind one physical input to one command", flags: F.DO_NOT_RECORD, visibility: "visible", completion: "none" }),
  Object.freeze({ name: "unbind", help: "Remove one physical input binding", flags: F.DO_NOT_RECORD, visibility: "visible", completion: "none" }),
  Object.freeze({ name: "unbindall", help: "Remove non-reserved physical input bindings", flags: F.DO_NOT_RECORD, visibility: "visible", completion: "none" }),
  Object.freeze({ name: "joyadvancedupdate", help: "Recompute joystick mapping", flags: 0, visibility: "visible", completion: "none" }),
  Object.freeze({ name: "mat_setvideomode", help: "Apply renderer width, height, and window state", flags: 0, visibility: "visible", completion: "none" }),
  Object.freeze({ name: "mat_enable_vrmode", help: "Change renderer VR mode for the next launch", flags: 0, visibility: "visible", completion: "none" }),
  Object.freeze({ name: "mat_savechanges", help: "Persist renderer configuration", flags: 0, visibility: "visible", completion: "none" }),
)

const desktopCodes = Object.freeze([
  ...Array.from({ length: 10 }, (_, index) => String(index)),
  ...Array.from({ length: 26 }, (_, index) => String.fromCharCode(97 + index)),
  "KP_INS", "KP_END", "KP_DOWNARROW", "KP_PGDN", "KP_LEFTARROW", "KP_5", "KP_RIGHTARROW", "KP_HOME", "KP_UPARROW", "KP_PGUP",
  "KP_SLASH", "KP_MULTIPLY", "KP_MINUS", "KP_PLUS", "KP_ENTER", "KP_DEL",
  "[", "]", "SEMICOLON", "'", "`", ",", ".", "/", "\\", "-", "=",
  "ENTER", "SPACE", "BACKSPACE", "TAB", "CAPSLOCK", "NUMLOCK", "ESCAPE", "SCROLLLOCK", "INS", "DEL", "HOME", "END", "PGUP", "PGDN", "PAUSE",
  "SHIFT", "RSHIFT", "ALT", "RALT", "CTRL", "RCTRL", "LWIN", "RWIN", "APP", "UPARROW", "LEFTARROW", "DOWNARROW", "RIGHTARROW",
  ...Array.from({ length: 12 }, (_, index) => `F${index + 1}`),
  "MOUSE1", "MOUSE2", "MOUSE3", "MOUSE4", "MOUSE5", "MWHEELUP", "MWHEELDOWN",
])

const actionDefaults: Readonly<Record<string, BindingValue>> = Object.freeze({
  "+forward": Object.freeze({ code: "w", modifiers: 0 }),
  "+back": Object.freeze({ code: "s", modifiers: 0 }),
  "+moveleft": Object.freeze({ code: "a", modifiers: 0 }),
  "+moveright": Object.freeze({ code: "d", modifiers: 0 }),
  "+jump": Object.freeze({ code: "SPACE", modifiers: 0 }),
  "+duck": Object.freeze({ code: "CTRL", modifiers: 0 }),
  "+moveup": Object.freeze({ code: "'", modifiers: 0 }),
  "+movedown": Object.freeze({ code: "/", modifiers: 0 }),
  "+lookup": Object.freeze({ code: "PGUP", modifiers: 0 }),
  "+lookdown": Object.freeze({ code: "PGDN", modifiers: 0 }),
  "+voicerecord": Object.freeze({ code: "v", modifiers: 0 }),
  say: Object.freeze({ code: "y", modifiers: 0 }),
  say_team: Object.freeze({ code: "u", modifiers: 0 }),
  voice_menu_1: Object.freeze({ code: "z", modifiers: 0 }),
  voice_menu_2: Object.freeze({ code: "x", modifiers: 0 }),
  voice_menu_3: Object.freeze({ code: "c", modifiers: 0 }),
  changeclass: Object.freeze({ code: ",", modifiers: 0 }),
  changeteam: Object.freeze({ code: ".", modifiers: 0 }),
  open_charinfo_direct: Object.freeze({ code: "m", modifiers: 0 }),
  open_charinfo_backpack: Object.freeze({ code: "n", modifiers: 0 }),
  show_quest_log: Object.freeze({ code: "F2", modifiers: 0 }),
  dropitem: Object.freeze({ code: "l", modifiers: 0 }),
  "+taunt": Object.freeze({ code: "g", modifiers: 0 }),
  "+use_action_slot_item": Object.freeze({ code: "h", modifiers: 0 }),
  showmapinfo: Object.freeze({ code: "i", modifiers: 0 }),
  "+inspect": Object.freeze({ code: "f", modifiers: 0 }),
  "+attack2": Object.freeze({ code: "MOUSE2", modifiers: 0 }),
  lastdisguise: Object.freeze({ code: "b", modifiers: 0 }),
  disguiseteam: Object.freeze({ code: "-", modifiers: 0 }),
  "+attack": Object.freeze({ code: "MOUSE1", modifiers: 0 }),
  "+attack3": Object.freeze({ code: "MOUSE3", modifiers: 0 }),
  "+reload": Object.freeze({ code: "r", modifiers: 0 }),
  invprev: Object.freeze({ code: "MWHEELUP", modifiers: 0 }),
  invnext: Object.freeze({ code: "MWHEELDOWN", modifiers: 0 }),
  lastinv: Object.freeze({ code: "q", modifiers: 0 }),
  slot1: Object.freeze({ code: "1", modifiers: 0 }),
  slot2: Object.freeze({ code: "2", modifiers: 0 }),
  slot3: Object.freeze({ code: "3", modifiers: 0 }),
  slot4: Object.freeze({ code: "4", modifiers: 0 }),
  slot5: Object.freeze({ code: "5", modifiers: 0 }),
  slot6: Object.freeze({ code: "6", modifiers: 0 }),
  slot7: Object.freeze({ code: "7", modifiers: 0 }),
  slot8: Object.freeze({ code: "8", modifiers: 0 }),
  slot9: Object.freeze({ code: "9", modifiers: 0 }),
  slot10: Object.freeze({ code: "0", modifiers: 0 }),
  "impulse 201": Object.freeze({ code: "t", modifiers: 0 }),
  "+showscores": Object.freeze({ code: "TAB", modifiers: 0 }),
  screenshot: Object.freeze({ code: "F5", modifiers: 0 }),
  save_replay: Object.freeze({ code: "F6", modifiers: 0 }),
  abuse_report_queue: Object.freeze({ code: "F7", modifiers: 0 }),
  toggleconsole: Object.freeze({ code: "`", modifiers: 0 }),
  cl_trigger_first_notification: Object.freeze({ code: "j", modifiers: 0 }),
  cl_decline_first_notification: Object.freeze({ code: "k", modifiers: 0 }),
})

export const TF2_KEYBOARD_ACTIONS = Object.freeze([
  "+forward", "+back", "+moveleft", "+moveright", "+jump", "+duck", "+moveup", "+movedown", "+lookup", "+lookdown",
  "+voicerecord", "say", "say_team", "voice_menu_1", "voice_menu_2", "voice_menu_3", "+helpme", "changeclass", "changeteam",
  "open_charinfo_direct", "open_charinfo_backpack", "show_quest_log", "show_matchmaking", "+quickswitch",
  "load_itempreset 0", "load_itempreset 1", "load_itempreset 2", "load_itempreset 3", "dropitem", "+taunt", "+use_action_slot_item",
  "+context_action", "showmapinfo", "+inspect", "callvote", "player_ready_toggle", "+attack2", "lastdisguise", "disguiseteam",
  "+attack", "+attack3", "+reload", "invprev", "invnext", "lastinv", "slot1", "slot2", "slot3", "slot4", "slot5", "slot6",
  "slot7", "slot8", "slot9", "slot10", "impulse 201", "+showscores", "screenshot", "save_replay", "abuse_report_queue",
  "quit", "toggleconsole", "askconnect_accept", "cl_trigger_first_notification", "cl_decline_first_notification",
] as const)

function actionId(action: string, index: number): string {
  const slug = action
    .replace(/^\+/u, "plus-")
    .replace(/[^a-z0-9]+/gu, "-")
    .replace(/^-|-$/gu, "")
  return `keyboard.${String(index + 1).padStart(2, "0")}-${slug}`
}

for (const [index, action] of TF2_KEYBOARD_ACTIONS.entries()) {
  settings.push(Object.freeze({
    kind: "binding",
    ...common(actionId(action, index), "keyboard", "input", F.DO_NOT_RECORD, ["bind", "unbind"]),
    defaultValue: actionDefaults[action] ?? null,
    action,
  }))
}

export const TF2_SELECTED_OPTIONS = defineSettingsCatalog({
  identity: "tf2.options.build-24245096.patch-10828683",
  convars,
  commands,
  bindingProfile: {
    identity: "tf2.desktop-keyboard-mouse",
    inputs: desktopCodes.map((code) => ({ code })),
    modifierMask: M.ALL,
    conflictPolicy: "replace",
    reserved: [{ code: "ESCAPE", modifiers: M.NONE }],
  },
  settings,
})
