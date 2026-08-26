import type { SettingValue } from "../contract"

export type Tf2VideoConfiguration = Readonly<{
  rootLod: 0 | 1 | 2
  picmip: -1 | 0 | 1 | 2
  shadowRenderToTexture: 0 | 1
  flashlightDepthTexture: 0 | 1
  reduceFillRate: 0 | 1
  hdrLevel: 0 | 1 | 2
  antialias: 0 | 4
  aaQuality: 0
  trilinear: 0 | 1
  anisotropy: 1 | 2 | 4 | 8 | 16
  waterForceExpensive: 0 | 1
  waterReflectEntities: 0 | 1
  vsync: 0 | 1
  queueMode: -1 | 0
  colorCorrection: 0 | 1
  motionBlur: 0 | 1
}>

export const TF2_BALANCED_VIDEO_SETTINGS: Readonly<Record<string, SettingValue>> = Object.freeze({
  "video.model-detail": "medium",
  "video.texture-detail": "medium",
  "video.shadow-detail": "medium",
  "video.shader-detail": "low",
  "video.hdr": 0,
  "video.antialias-samples": 0,
  "video.antialias-quality": 0,
  "video.filtering": "bilinear",
  "video.water-detail": "no-reflections",
  "video.vsync": false,
  "video.multicore": true,
  "video.color-correction": false,
  "video.motion-blur": false,
})

export function tf2VideoConfiguration(values: Readonly<Record<string, SettingValue>>): Tf2VideoConfiguration {
  const selected = { ...TF2_BALANCED_VIDEO_SETTINGS, ...values }
  const model = selected["video.model-detail"]
  const texture = selected["video.texture-detail"]
  const shadow = selected["video.shadow-detail"]
  const shader = selected["video.shader-detail"]
  const filtering = selected["video.filtering"]
  const water = selected["video.water-detail"]
  const hdr = selected["video.hdr"]
  const samples = selected["video.antialias-samples"]
  const quality = selected["video.antialias-quality"]
  if (model !== "low" && model !== "medium" && model !== "high") throw new Error("model detail is invalid")
  if (texture !== "low" && texture !== "medium" && texture !== "high" && texture !== "ultra") throw new Error("texture detail is invalid")
  if (shadow !== "low" && shadow !== "medium" && shadow !== "high") throw new Error("shadow detail is invalid")
  if (shader !== "low" && shader !== "high") throw new Error("shader detail is invalid")
  if (water !== "no-reflections" && water !== "reflect-world" && water !== "reflect-all") throw new Error("water detail is invalid")
  if (hdr !== 0 && hdr !== 1 && hdr !== 2) throw new Error("HDR level is invalid")
  if (samples !== 0 && samples !== 4) throw new Error("WebGPU antialias sample count is unsupported")
  if (quality !== 0) throw new Error("WebGPU antialias quality is unsupported")
  const anisotropy = filtering === "bilinear" || filtering === "trilinear" ? 1
    : filtering === "anisotropic-2" ? 2 : filtering === "anisotropic-4" ? 4
      : filtering === "anisotropic-8" ? 8 : filtering === "anisotropic-16" ? 16 : null
  if (anisotropy === null) throw new Error("texture filtering is invalid")
  const flag = (name: string): 0 | 1 => {
    const value = selected[name]
    if (typeof value !== "boolean") throw new Error(`${name} is invalid`)
    return value ? 1 : 0
  }
  return Object.freeze({
    rootLod: model === "low" ? 2 : model === "medium" ? 1 : 0,
    picmip: texture === "low" ? 2 : texture === "medium" ? 1 : texture === "high" ? 0 : -1,
    shadowRenderToTexture: shadow === "low" ? 0 : 1,
    flashlightDepthTexture: shadow === "high" ? 1 : 0,
    reduceFillRate: shader === "low" ? 1 : 0,
    hdrLevel: hdr,
    antialias: samples,
    aaQuality: 0,
    trilinear: filtering === "trilinear" ? 1 : 0,
    anisotropy,
    waterForceExpensive: water === "no-reflections" ? 0 : 1,
    waterReflectEntities: water === "reflect-all" ? 1 : 0,
    vsync: flag("video.vsync"),
    queueMode: flag("video.multicore") ? -1 : 0,
    colorCorrection: flag("video.color-correction"),
    motionBlur: flag("video.motion-blur"),
  })
}

export function tf2VideoConvars(configuration: Tf2VideoConfiguration): Readonly<Record<string, number>> {
  return Object.freeze({
    r_rootlod: configuration.rootLod,
    mat_picmip: configuration.picmip,
    r_shadowrendertotexture: configuration.shadowRenderToTexture,
    r_flashlightdepthtexture: configuration.flashlightDepthTexture,
    mat_reducefillrate: configuration.reduceFillRate,
    mat_hdr_level: configuration.hdrLevel,
    mat_antialias: configuration.antialias,
    mat_aaquality: configuration.aaQuality,
    mat_trilinear: configuration.trilinear,
    mat_forceaniso: configuration.anisotropy,
    r_waterforceexpensive: configuration.waterForceExpensive,
    r_waterforcereflectentities: configuration.waterReflectEntities,
    mat_vsync: configuration.vsync,
    mat_queue_mode: configuration.queueMode,
    mat_colorcorrection: configuration.colorCorrection,
    mat_motion_blur_enabled: configuration.motionBlur,
  })
}

export function tf2VideoSettingsFromConvars(
  configuration: Tf2VideoConfiguration,
  changes: Readonly<Record<string, number>>,
): Readonly<Record<string, SettingValue>> {
  const values = { ...tf2VideoConvars(configuration), ...changes }
  const root = values.r_rootlod
  const mip = values.mat_picmip
  const shadow = values.r_shadowrendertotexture
  const depth = values.r_flashlightdepthtexture
  const trilinear = values.mat_trilinear
  const anisotropy = values.mat_forceaniso
  const expensive = values.r_waterforceexpensive
  const entities = values.r_waterforcereflectentities
  const filtering = anisotropy === 1 ? trilinear === 1 ? "trilinear" : trilinear === 0 ? "bilinear" : null
    : trilinear === 0 && [2, 4, 8, 16].includes(anisotropy!) ? `anisotropic-${anisotropy}` : null
  const selected: Record<string, SettingValue> = {
    "video.model-detail": root === 2 ? "low" : root === 1 ? "medium" : root === 0 ? "high" : null,
    "video.texture-detail": mip === 2 ? "low" : mip === 1 ? "medium" : mip === 0 ? "high" : mip === -1 ? "ultra" : null,
    "video.shadow-detail": shadow === 0 && depth === 0 ? "low" : shadow === 1 && depth === 0 ? "medium" : shadow === 1 && depth === 1 ? "high" : null,
    "video.shader-detail": values.mat_reducefillrate === 1 ? "low" : values.mat_reducefillrate === 0 ? "high" : null,
    "video.hdr": values.mat_hdr_level!,
    "video.antialias-samples": values.mat_antialias!,
    "video.antialias-quality": values.mat_aaquality!,
    "video.filtering": filtering,
    "video.water-detail": expensive === 0 && entities === 0 ? "no-reflections" : expensive === 1 && entities === 0 ? "reflect-world" : expensive === 1 && entities === 1 ? "reflect-all" : null,
    "video.vsync": values.mat_vsync === 0 ? false : values.mat_vsync === 1 ? true : null,
    "video.multicore": values.mat_queue_mode === -1 ? true : values.mat_queue_mode === 0 ? false : null,
    "video.color-correction": values.mat_colorcorrection === 0 ? false : values.mat_colorcorrection === 1 ? true : null,
    "video.motion-blur": values.mat_motion_blur_enabled === 0 ? false : values.mat_motion_blur_enabled === 1 ? true : null,
  }
  tf2VideoConfiguration(selected)
  return Object.freeze(selected)
}
