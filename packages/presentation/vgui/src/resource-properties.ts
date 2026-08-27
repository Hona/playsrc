/** KeyValues scalar lookup is ASCII-insensitive and selects the first entry.
 * Keep the authored list for ordered application; this short-lived index only
 * removes repeated scans while validating/applying that same list. */
const ASCII_FOLDS = new Map<string, string>()

export function asciiFold(value: string): string {
  const prior = ASCII_FOLDS.get(value)
  if (prior !== undefined) return prior
  const folded = value.replace(/[A-Z]/gu, character => character.toLowerCase())
  if (ASCII_FOLDS.size < 16_384) ASCII_FOLDS.set(value, folded)
  return folded
}

export function resourcePropertyReader(properties: readonly Readonly<{ name: string; value: string }>[]): (name: string) => string | null {
  const values = new Map<string, string>()
  for (const property of properties) {
    const name = asciiFold(property.name)
    if (!values.has(name)) values.set(name, property.value)
  }
  return name => values.get(asciiFold(name)) ?? null
}
