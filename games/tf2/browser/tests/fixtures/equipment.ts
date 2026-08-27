import bytes from "./equipment-state.json" with { type: "json" }
import { decodeEquipmentState } from "../../src/equipment/codec"
import type { Tf2Class } from "../../src/codec"

/** Regenerate with cargo run -p playsrc-tf2 --example equipment-fixture. */
export const nativeEquipment = decodeEquipmentState(new Uint8Array(bytes))
export const stockItems = (playerClass: Tf2Class) => nativeEquipment.classes.find(value => value.class === playerClass)!.baseItems
