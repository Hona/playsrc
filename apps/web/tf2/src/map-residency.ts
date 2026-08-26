type Owner = Readonly<{
  generation: number
  payload?: Uint8Array
  presentation?: Uint8Array
  sections?: readonly Uint8Array[]
}>

/** Logical live ranges, not an estimate of process-private pages or JS object overhead. */
export function mapResidency(active?: Owner, candidate?: Owner) {
  const sameRange = (a: Uint8Array, b: Uint8Array) => a.buffer === b.buffer
    && a.byteOffset === b.byteOffset && a.byteLength === b.byteLength
  const shared = (candidate?.sections ?? []).filter((section) => active?.sections?.some((other) => sameRange(section, other)))
  const bytes = (sections: readonly Uint8Array[] = []) => sections.reduce((total, section) => total + section.byteLength, 0)
  const owner = (value?: Owner) => value ? {
    generation: value.generation,
    canonicalBytes: value.payload?.byteLength ?? 0,
    presentationBytes: value.presentation?.byteLength ?? 0,
    sourceReferencedBytes: bytes(value.sections),
    sourceExclusiveBytes: bytes(value.sections?.filter((section) => !shared.some((other) => sameRange(section, other)))),
  } : null
  return { active: owner(active), candidate: owner(candidate), sharedSourceBytes: bytes(shared) }
}
