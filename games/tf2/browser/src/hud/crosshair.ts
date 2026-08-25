import type { SettingValue } from "@playsrc/settings"
import { tf2AuthoredCrosshairs, type Tf2AuthoredCrosshair } from "../ui-resources/crosshair"
import type { Tf2HudBinding, Tf2HudCrosshair } from "./contract"

export const TF2_CROSSHAIR_SETTINGS = Object.freeze([
  Object.freeze({ name: "cl_crosshair_red", settingId: "multiplayer.crosshair-red", defaultValue: "200" }),
  Object.freeze({ name: "cl_crosshair_green", settingId: "multiplayer.crosshair-green", defaultValue: "200" }),
  Object.freeze({ name: "cl_crosshair_blue", settingId: "multiplayer.crosshair-blue", defaultValue: "200" }),
  Object.freeze({ name: "cl_crosshair_scale", settingId: "multiplayer.crosshair-scale", defaultValue: "32.0" }),
  Object.freeze({ name: "cl_crosshair_file", settingId: "multiplayer.crosshair-file", defaultValue: "" }),
] as const)

export type Tf2CrosshairSettings = Readonly<{
  red: number
  green: number
  blue: number
  scale: number
  file: string
}>

export type Tf2CrosshairGeometry = Readonly<{
  kind: "stock" | "custom"
  asset: Tf2AuthoredCrosshair
  left: number
  top: number
  width: number
  height: number
  color: readonly [number, number, number, number]
}>

function colorChannel(value: SettingValue | undefined, channel: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error(`TF2 crosshair ${channel} setting is not finite`)
  }
  return ((Math.trunc(value) % 256) + 256) % 256
}

export function tf2CrosshairSettings(values: Readonly<Record<string, SettingValue>>): Tf2CrosshairSettings {
  const scale = values["multiplayer.crosshair-scale"]
  const file = values["multiplayer.crosshair-file"]
  if (typeof scale !== "number" || !Number.isFinite(scale)) throw new Error("TF2 crosshair scale setting is not finite")
  if (typeof file !== "string") throw new Error("TF2 crosshair file setting is not a string")
  return Object.freeze({
    red: colorChannel(values["multiplayer.crosshair-red"], "red"),
    green: colorChannel(values["multiplayer.crosshair-green"], "green"),
    blue: colorChannel(values["multiplayer.crosshair-blue"], "blue"),
    scale,
    file,
  })
}

export function tf2CrosshairHudValues(settings: Tf2CrosshairSettings): Pick<Tf2HudCrosshair, "texture" | "color" | "scale"> {
  return Object.freeze({
    texture: settings.file === "" ? "crosshair_default" : `vgui/crosshairs/${settings.file}`,
    color: Object.freeze([settings.red, settings.green, settings.blue, 255] as const),
    scale: settings.scale,
  })
}

export function tf2CustomCrosshairFile(texture: string): string | null {
  const file = texture.match(/^vgui\/crosshairs\/([a-z0-9_-]+)$/iu)?.[1]
  return file === undefined ? null : file.toLowerCase()
}

export function resolveTf2CrosshairGeometry(
  binding: Tf2HudBinding,
  viewport: Readonly<{ width: number; height: number }>,
): Tf2CrosshairGeometry | null {
  const player = binding.facts.player.kind === "available" ? binding.facts.player.value : null
  const crosshair = player?.crosshair.kind === "available" ? player.crosshair.value : null
  const visible = binding.values.find((value) => value.kind === "visible" && value.panel === "HudCrosshair")
  if (!player || !crosshair || visible?.kind !== "visible" || !visible.value) return null
  const file = tf2CustomCrosshairFile(crosshair.texture)
  const kind = file === null ? "stock" : "custom"
  let asset: Tf2AuthoredCrosshair | undefined
  if (file !== null) {
    asset = tf2AuthoredCrosshairs.styles.find((style) => style.file.toLowerCase() === file)
  } else if (crosshair.texture === "crosshair_default") {
    const active = player.activeWeapon.kind === "available" ? player.activeWeapon.value : null
    asset = active === null
      ? tf2AuthoredCrosshairs.stock
      : tf2AuthoredCrosshairs.weapons.find((weapon) => weapon.weaponIdentities.includes(active))?.crosshair
        ?? tf2AuthoredCrosshairs.stock
  }
  if (!asset) return null
  const nominalWidth = kind === "custom" ? 32 : asset.crop!.width
  const nominalHeight = kind === "custom" ? 32 : asset.crop!.height
  const width = Math.trunc(crosshair.weaponScale * crosshair.scale / 32 * nominalWidth + 0.5)
  const height = Math.trunc(crosshair.weaponScale * crosshair.scale / 32 * nominalHeight + 0.5)
  const centerX = Math.trunc(viewport.width / 2 + 0.5)
  const centerY = Math.trunc(viewport.height / 2 + 0.5)
  return Object.freeze({
    kind,
    asset,
    left: centerX - (kind === "custom" ? width : Math.trunc(width / 2)),
    top: centerY - (kind === "custom" ? height : Math.trunc(height / 2)),
    width: kind === "custom" ? width * 2 : width,
    height: kind === "custom" ? height * 2 : height,
    color: Object.freeze([crosshair.color[0], crosshair.color[1], crosshair.color[2], 255] as const),
  })
}
