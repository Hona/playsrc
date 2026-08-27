import type { Tf2WorkerClient } from "../client"
import type { Tf2Class } from "../codec"
import type { Tf2EquipmentState } from "./types"

const STORAGE_KEY = "playsrc.tf2.local-equipment.v1"

/** Browser persistence is an opaque copy; validation and equip decisions stay in Rust. */
export class Tf2EquipmentProfile {
  #state: Tf2EquipmentState | undefined
  #tail: Promise<unknown> = Promise.resolve()
  #closed = false
  constructor(readonly client: Pick<Tf2WorkerClient, "equipment">, readonly storage?: Pick<Storage, "getItem" | "setItem">) {}

  state(): Tf2EquipmentState | undefined { return this.#state }
  async initialize(): Promise<Tf2EquipmentState> {
    const saved = this.storage?.getItem(STORAGE_KEY)
    let mutation: Uint8Array | undefined
    if (saved !== null && saved !== undefined) {
      if (saved.length !== 924) throw new Error("Invalid local equipment storage length")
      const binary = atob(saved)
      if (binary.length !== 692) throw new Error("Invalid local equipment storage")
      mutation = new Uint8Array(693)
      for (let index = 0; index < binary.length; index++) mutation[index + 1] = binary.charCodeAt(index)
    }
    const state = await this.client.equipment(0, mutation?.buffer as ArrayBuffer | undefined)
    if (this.#closed) throw new Error("Equipment profile was replaced")
    this.#state = state
    return state
  }

  equip(playerClass: Tf2Class, slot: number, definitionIndex: number | null): Promise<Tf2EquipmentState> {
    const task = this.#tail.then(async () => {
      if (this.#closed || !this.#state) throw new Error("Equipment profile is unavailable")
      const mutation = new Uint8Array(7)
      mutation.set([1, playerClass, slot])
      new DataView(mutation.buffer).setUint32(3, definitionIndex ?? 0xffff_ffff, true)
      const state = await this.client.equipment(0, mutation.buffer)
      if (this.#closed) throw new Error("Equipment profile was replaced")
      this.#state = state
      this.storage?.setItem(STORAGE_KEY, btoa(String.fromCharCode(...state.persistence)))
      return state
    })
    this.#tail = task.catch(() => {})
    return task
  }
  close(): void { this.#closed = true; this.#state = undefined }
}
