/** Opt-in owner spans. These are not evidence of visible presentation. */
export function selectionTransitionMark(kind: string, detail?: Record<string, unknown>): void {
  const profile = (globalThis as any).__playsrcProfile
  if (profile?.captureSelectionTransitions !== true) return
  const entries = profile.selectionOwners ??= []
  if (entries.length >= 1024) { profile.selectionOwnersDropped = (profile.selectionOwnersDropped ?? 0) + 1; return }
  const at = performance.now()
  entries.push({ kind, at, detail })
  performance.mark(`playsrc-selection-${kind}`, { detail })
}

export function selectionTransitionDraw(detail: Record<string, unknown>): void {
  const profile = (globalThis as any).__playsrcProfile
  if (profile?.captureSelectionTransitions !== true) return
  const key = JSON.stringify(detail)
  if (profile.selectionDrawKey === key) return
  profile.selectionDrawKey = key
  selectionTransitionMark("draw-complete", detail)
}
