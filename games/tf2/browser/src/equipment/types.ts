import type { Tf2Class, Tf2Weapon } from "../codec"

/** Read-only projections of the Rust local equipment owner. */
export type Tf2ItemAttribute = Readonly<{ definition: number; value: number }>
export type Tf2EquippedItem = Readonly<{
  itemId: number
  definitionIndex: number
  quality: number
  style: number
  slot: number
  attributes: readonly Tf2ItemAttribute[]
}>

export type Tf2SupportedItem = Readonly<{
  item: Tf2EquippedItem
  name: string
  displayName: string
  description: readonly Readonly<{ text: string; color: string }>[]
  animationSlot: string | null
  extraSounds: readonly string[]
  image: string
  modelPlayer: string
  attachToHands: boolean
  animationReplacements: readonly (readonly [string, string])[]
  soundOverrides: readonly (readonly [string, string])[]
  deathNoticeIcon: string | null
  weapon: Tf2Weapon | null
  classSlots: readonly Readonly<{ class: Tf2Class; slot: number; weapon: Tf2Weapon | null; selectionSlot: number | null; hud: Tf2WeaponHud | null }>[]
}>

export type Tf2WeaponHud = Readonly<{
  countMeter: "kills" | "revenge-active" | "heads" | "revenge" | null
  script: string
  ammoDisplay: "hidden" | "total" | "clip-and-reserve"
  bucket: number
  position: number
  drawsCrosshair: boolean
  suppressCrosshair: boolean
}>

export type Tf2EquipmentState = Readonly<{
  revision: number
  inventory: readonly Tf2SupportedItem[]
  classes: readonly Readonly<{ class: Tf2Class; items: readonly Tf2EquippedItem[]; baseItems: readonly Tf2EquippedItem[] }>[]
  persistence: Uint8Array
}>
