import type { Tf2Class, Tf2Weapon } from "../codec"
import type { Tf2EquipmentState, Tf2EquippedItem, Tf2SupportedItem } from "./types"

export function decodeEquippedItems(view: DataView, start: number): Readonly<{ items: readonly Tf2EquippedItem[]; end: number }> {
  let at = start
  const count = view.getUint32(at, true); at += 4
  if (count > 19) throw new Error("invalid equipped item count")
  const items: Tf2EquippedItem[] = []
  const slots = new Set<number>()
  for (let index = 0; index < count; index++) {
    const itemId = view.getUint32(at, true), definitionIndex = view.getUint32(at + 4, true)
    const quality = view.getUint8(at + 8), style = view.getUint8(at + 9), slot = view.getUint8(at + 10), attributeCount = view.getUint8(at + 11)
    at += 12
    if (itemId !== definitionIndex + 1 || quality > 15 || slot >= 19 || slots.has(slot) || attributeCount > 16) throw new Error("invalid equipped item")
    slots.add(slot)
    const attributes = Array.from({ length: attributeCount }, () => {
      const definition = view.getUint32(at, true), value = view.getFloat32(at + 4, true); at += 8
      if (!Number.isFinite(value)) throw new Error("invalid item attribute")
      return Object.freeze({ definition, value })
    })
    items.push(Object.freeze({ itemId, definitionIndex, quality, style, slot, attributes: Object.freeze(attributes) }))
  }
  return Object.freeze({ items: Object.freeze(items), end: at })
}

export function decodeEquipmentState(bytes: Uint8Array): Tf2EquipmentState {
  // Browser TextDecoder rejects views over the threaded WASM shared memory.
  // This UI-only projection takes one owned copy, never one copy per string.
  if (typeof SharedArrayBuffer !== "undefined" && bytes.buffer instanceof SharedArrayBuffer) bytes = new Uint8Array(bytes)
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  if (bytes.byteLength > 1024 * 1024 || view.getUint32(0, true) !== 0x49454654 || view.getUint32(4, true) !== 4) throw new Error("invalid equipment state")
  const revision = view.getUint32(8, true), count = view.getUint32(12, true)
  if (count > 256) throw new Error("invalid supported item count")
  let at = 16
  const text = () => {
    const length = view.getUint32(at, true); at += 4
    if (length > 2048 || at + length > bytes.byteLength) throw new Error("invalid item presentation")
    const value = new TextDecoder("utf-8", { fatal: true }).decode(bytes.subarray(at, at + length)); at += length
    return value
  }
  const inventory: Tf2SupportedItem[] = []
  for (let index = 0; index < count; index++) {
    const decoded = decodeEquippedItems(view, at); at = decoded.end
    if (decoded.items.length !== 1) throw new Error("invalid owned item count")
    const weapon = view.getUint8(at++), classes = view.getUint8(at++)
    if (classes < 1 || classes > 27) throw new Error("invalid class eligibility")
    const classSlots = Array.from({ length: classes }, () => {
      const identity = view.getUint8(at++), slot = view.getUint8(at++), runtime = view.getUint8(at++), selection = view.getUint8(at++)
      if (identity < 1 || identity > 9 || slot > 18) throw new Error("invalid class slot")
      if (selection !== 255 && selection > 5) throw new Error("invalid weapon selection slot")
      return Object.freeze({ class: identity as Tf2Class, slot, weapon: runtime === 0 ? null : runtime as Tf2Weapon, selectionSlot: selection === 255 ? null : selection })
    })
    const name = text(), displayName = text(), image = text()
    const lines = view.getUint32(at, true); at += 4
    if (lines > 128) throw new Error("invalid item description count")
    const description = Object.freeze(Array.from({ length: lines }, () => Object.freeze({ text: text(), color: text() })))
    const animationSlot = text() || null
    const sounds = view.getUint32(at, true); at += 4
    if (sounds > 32) throw new Error("invalid class sound count")
    const extraSounds = Object.freeze(Array.from({ length: sounds }, text))
    const attach = view.getUint8(at++), deathNoticeIcon = text() || null, modelPlayer = text()
    if (attach > 1) throw new Error("invalid item attachment")
    const pairs = () => {
      const count = view.getUint32(at, true); at += 4
      if (count > 128) throw new Error("invalid item replacements")
      return Object.freeze(Array.from({ length: count }, () => Object.freeze([text(), text()] as const)))
    }
    inventory.push(Object.freeze({ item: decoded.items[0]!, weapon: weapon === 0 ? null : weapon as Tf2Weapon, classSlots: Object.freeze(classSlots), name, displayName, description, animationSlot, extraSounds, image, modelPlayer,
      attachToHands: attach === 1, deathNoticeIcon, animationReplacements: pairs(), soundOverrides: pairs() }))
  }
  const classes = Array.from({ length: 9 }, (_, index) => {
    const decoded = decodeEquippedItems(view, at); at = decoded.end
    const base = decodeEquippedItems(view, at); at = base.end
    return Object.freeze({ class: (index + 1) as Tf2Class, items: decoded.items, baseItems: base.items })
  })
  const length = view.getUint32(at, true); at += 4
  if (length !== 692 || at + length !== bytes.byteLength) throw new Error("invalid equipment persistence")
  return Object.freeze({ revision, inventory: Object.freeze(inventory), classes: Object.freeze(classes), persistence: bytes.slice(at) })
}
