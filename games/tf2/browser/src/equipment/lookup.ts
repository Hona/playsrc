import type { Tf2SupportedItem } from "./types"

const indexes = new WeakMap<readonly Tf2SupportedItem[], ReadonlyMap<number, Tf2SupportedItem>>()

export function equipmentCatalogIndex(inventory: readonly Tf2SupportedItem[]): ReadonlyMap<number, Tf2SupportedItem> {
  let index = indexes.get(inventory)
  if (!index) { index = new Map(inventory.map(item => [item.item.definitionIndex, item])); indexes.set(inventory, index) }
  return index
}
