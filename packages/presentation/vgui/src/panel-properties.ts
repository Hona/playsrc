import { asciiFold } from "./resource-properties"

/** Installed panel values retain their authored spelling/order and exact-key
 * getters. Layout's first ASCII-insensitive lookup is intentionally separate.
 * Paint and DOM publication share the serialization until these values change. */
export class PanelProperties extends Map<string, string> {
  private serialized: string | undefined

  first(name: string): string | null {
    const folded = asciiFold(name)
    for (const key of this.keys()) if (asciiFold(key) === folded) return this.get(key)!
    return null
  }

  signature(): string {
    return this.serialized ??= JSON.stringify([...this])
  }

  override set(name: string, value: string): this {
    if (this.get(name) !== value) this.serialized = undefined
    return super.set(name, value)
  }

  override delete(name: string): boolean {
    const deleted = super.delete(name)
    if (deleted) this.serialized = undefined
    return deleted
  }

  override clear(): void {
    if (this.size) this.serialized = undefined
    super.clear()
  }
}
