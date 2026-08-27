import type { Snapshot } from "../codec"
import type { Tf2SupportedItem } from "../equipment/types"
import { equipmentCatalogIndex } from "../equipment/lookup"

export type WeaponCountMeter = Readonly<{ kind: "kills" | "crits" | "heads"; count: number }>

/** The configured weapon class selects its meter. GetCount overrides the bar;
 * only the Manmelter specialization requires that weapon to be active. */
export function weaponCountMeter(
  snapshot: Pick<Snapshot, "lifecycle" | "conditions" | "class" | "weapon" | "equippedItems" | "decapitations" | "revengeCrits">,
  inventory: readonly Tf2SupportedItem[],
): WeaponCountMeter | null {
  if (snapshot.lifecycle !== 1 || (snapshot.conditions[2]! & (1 << (77 - 64))) !== 0) return null
  const definitions = equipmentCatalogIndex(inventory)
  for (const item of snapshot.equippedItems) {
    const slot = definitions.get(item.definitionIndex)?.classSlots.find(slot => slot.class === snapshot.class && slot.slot === item.slot)
    const meter = slot?.hud?.countMeter
    if (!meter || meter === "revenge-active" && slot!.weapon !== snapshot.weapon) continue
    return Object.freeze({ kind: meter === "kills" ? "kills" : meter === "heads" ? "heads" : "crits",
      count: meter === "kills" || meter === "heads" ? snapshot.decapitations : snapshot.revengeCrits })
  }
  return null
}
