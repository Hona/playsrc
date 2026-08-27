import { TF2_CONTENT_BUILD } from "../content-build"
import { configuredTf2AuthoredCrosshairInput } from "./crosshair.generated"

const SHA256 = /^[0-9a-f]{64}$/u
const STYLE = /^[a-z0-9_-]+$/u
const MAX_STYLES = 128
const MAX_FRAMES = 64
const MAX_DIMENSION = 512

export type Tf2AuthoredCrosshairSource = Readonly<{
  logicalPath: string
  byteLength: number
  sha256: string
}>

export type Tf2AuthoredCrosshairCrop = Readonly<{
  x: number
  y: number
  width: number
  height: number
}>

export type Tf2AuthoredCrosshairFrame = Readonly<{
  index: number
  pngSha256: string
  pngDataUrl: string
}>

export type Tf2AuthoredCrosshair = Readonly<{
  file: string
  material: Tf2AuthoredCrosshairSource
  texture: Tf2AuthoredCrosshairSource
  textureWidth: number
  textureHeight: number
  crop: Tf2AuthoredCrosshairCrop | null
  frames: readonly Tf2AuthoredCrosshairFrame[]
}>

export type Tf2AuthoredWeaponCrosshair = Readonly<{
  source: Tf2AuthoredCrosshairSource
  crosshair: Tf2AuthoredCrosshair
  autoaim: Tf2AuthoredCrosshair | null
}>

export type Tf2AuthoredCrosshairDescriptor = Readonly<{
  schema: "playsrc-tf2-authored-crosshairs-v2"
  contentBuild: string
  iconSource: Tf2AuthoredCrosshairSource
  stock: Tf2AuthoredCrosshair
  weapons: readonly Tf2AuthoredWeaponCrosshair[]
  styles: readonly Tf2AuthoredCrosshair[]
}>

function record(value: unknown, subject: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`TF2 authored crosshair ${subject} is malformed`)
  }
  return value as Record<string, unknown>
}

function source(input: unknown, subject: string): Tf2AuthoredCrosshairSource {
  const value = record(input, subject)
  if (typeof value.logicalPath !== "string"
    || !/^[a-z0-9_./-]+$/u.test(value.logicalPath)
    || value.logicalPath.includes("..")
    || !Number.isSafeInteger(value.byteLength)
    || (value.byteLength as number) <= 0
    || typeof value.sha256 !== "string"
    || !SHA256.test(value.sha256)) {
    throw new Error(`TF2 authored crosshair ${subject} identity is malformed`)
  }
  return Object.freeze({
    logicalPath: value.logicalPath,
    byteLength: value.byteLength as number,
    sha256: value.sha256,
  })
}

function asset(input: unknown, subject: string): Tf2AuthoredCrosshair {
  const value = record(input, subject)
  const material = source(value.material, `${subject} material`)
  const texture = source(value.texture, `${subject} texture`)
  if (typeof value.file !== "string"
    || (value.file !== "" && !STYLE.test(value.file))
    || !Number.isSafeInteger(value.textureWidth)
    || !Number.isSafeInteger(value.textureHeight)
    || (value.textureWidth as number) <= 0
    || (value.textureWidth as number) > MAX_DIMENSION
    || (value.textureHeight as number) <= 0
    || (value.textureHeight as number) > MAX_DIMENSION
    || !Array.isArray(value.frames)
    || value.frames.length === 0
    || value.frames.length > MAX_FRAMES) {
    throw new Error(`TF2 authored crosshair ${subject} dimensions or frames are malformed`)
  }
  let crop: Tf2AuthoredCrosshairCrop | null = null
  if (value.crop !== null) {
    const region = record(value.crop, `${subject} source crop`)
    if (![region.x, region.y, region.width, region.height].every(Number.isSafeInteger)
      || (region.x as number) < 0
      || (region.y as number) < 0
      || (region.width as number) <= 0
      || (region.height as number) <= 0
      || (region.x as number) + (region.width as number) > (value.textureWidth as number)
      || (region.y as number) + (region.height as number) > (value.textureHeight as number)) {
      throw new Error(`TF2 authored crosshair ${subject} source crop exceeds its texture`)
    }
    crop = Object.freeze({
      x: region.x as number,
      y: region.y as number,
      width: region.width as number,
      height: region.height as number,
    })
  }
  const frames = value.frames.map((input, index) => {
    const frame = record(input, `${subject} frame ${index}`)
    if (frame.index !== index
      || typeof frame.pngSha256 !== "string"
      || !SHA256.test(frame.pngSha256)
      || typeof frame.pngDataUrl !== "string"
      || !/^data:image\/png;base64,[A-Za-z0-9+/]+={0,2}$/u.test(frame.pngDataUrl)) {
      throw new Error(`TF2 authored crosshair ${subject} frame ${index} is malformed`)
    }
    return Object.freeze({ index, pngSha256: frame.pngSha256, pngDataUrl: frame.pngDataUrl })
  })
  return Object.freeze({
    file: value.file,
    material,
    texture,
    textureWidth: value.textureWidth as number,
    textureHeight: value.textureHeight as number,
    crop,
    frames: Object.freeze(frames),
  })
}

export function createTf2AuthoredCrosshairDescriptor(input: unknown): Tf2AuthoredCrosshairDescriptor {
  const value = record(input, "descriptor")
  if (value.schema !== "playsrc-tf2-authored-crosshairs-v2"
    || value.contentBuild !== TF2_CONTENT_BUILD.contentBuild
    || !Array.isArray(value.styles)
    || value.styles.length === 0
    || value.styles.length > MAX_STYLES
    || !Array.isArray(value.weapons)
    || value.weapons.length > MAX_STYLES) {
    throw new Error("TF2 authored crosshair descriptor identity or bounds are invalid")
  }
  const stock = asset(value.stock, "stock icon")
  if (stock.file !== "" || stock.crop === null) throw new Error("TF2 authored stock crosshair is not an atlas icon")
  const occupiedWeapons = new Set<string>()
  const weapons = value.weapons.map((input, index) => {
    const item = record(input, `weapon ${index}`)
    const script = source(item.source, `weapon ${index} script`)
    if (!/^scripts\/tf_weapon_[a-z0-9_]+\.ctx$/u.test(script.logicalPath) || occupiedWeapons.has(script.logicalPath)) {
      throw new Error(`TF2 authored weapon crosshair ${index} script is invalid or duplicated`)
    }
    occupiedWeapons.add(script.logicalPath)
    const crosshair = asset(item.crosshair, `weapon ${index} icon`)
    if (crosshair.crop === null) throw new Error(`TF2 authored weapon crosshair ${index} has no atlas crop`)
    const autoaim = item.autoaim === null ? null : asset(item.autoaim, `weapon ${index} autoaim`)
    return Object.freeze({
      source: script,
      crosshair,
      autoaim,
    })
  })
  const rawStyles = value.styles
  const styles = rawStyles.map((input, index) => {
    const style = asset(input, `style ${index}`)
    if (style.file === "" || style.crop !== null
      || style.material.logicalPath !== `materials/vgui/crosshairs/${style.file}.vmt`
      || style.texture.logicalPath !== `materials/vgui/crosshairs/${style.file}.vtf`) {
      throw new Error(`TF2 authored crosshair style ${index} does not have one exact material/texture pair`)
    }
    if (index > 0 && rawStyles[index - 1].file >= style.file) {
      throw new Error("TF2 authored crosshair styles are not distinct and canonically ordered")
    }
    return style
  })
  return Object.freeze({
    schema: "playsrc-tf2-authored-crosshairs-v2",
    contentBuild: value.contentBuild,
    iconSource: source(value.iconSource, "icon definition"),
    stock,
    weapons: Object.freeze(weapons),
    styles: Object.freeze(styles),
  })
}

export const tf2AuthoredCrosshairs = createTf2AuthoredCrosshairDescriptor(configuredTf2AuthoredCrosshairInput)
