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
  image: string
  weapon: Tf2Weapon | null
  classSlots: readonly Readonly<{ class: Tf2Class; slot: number }>[]
}>

export type Tf2EquipmentState = Readonly<{
  revision: number
  inventory: readonly Tf2SupportedItem[]
  classes: readonly Readonly<{ class: Tf2Class; items: readonly Tf2EquippedItem[] }>[]
  persistence: Uint8Array
}>
