export type ClassSwitchLifecycleEvent = Readonly<{
  at: number
  phase: string
  playerClass?: number
  key?: string
  visible?: boolean
  button?: number
  trusted?: boolean
  controllerAction?: string | null
}>

// fireAt records a physical +attack press, not a claimed shot. The shared gate
// separately requires the actual Rust tick admission; Source deployment and
// weapon cooldowns may legitimately defer a PrimaryAttack activity.
export function summarizeClassSwitchLifecycle(events: readonly ClassSwitchLifecycleEvent[]) {
  const admitted = new Set<number>()
  let openedAt: number | null = null
  let panelAt: number | null = null
  let previous = Number.NEGATIVE_INFINITY
  const switches: Array<{
    playerClass: number
    admission: "first" | "retained"
    openedAt: number | null
    selectedAt: number
    selectionMilliseconds: number | null
    panelMilliseconds: number | null
    fireAt: number | null
    fireMilliseconds: number | null
  }> = []
  for (const event of events) {
    if (!Number.isFinite(event.at) || event.at < previous) throw new Error("Class-switch lifecycle timestamps must be ordered")
    previous = event.at
    if (event.phase === "key-down" && event.key === "Comma") {
      if (panelAt === null || event.at - panelAt > 1) panelAt = null
      openedAt = event.at
    }
    else if (event.phase === "class-panel" && event.visible) panelAt = event.at
    else if (event.phase === "selected") {
      if (!Number.isSafeInteger(event.playerClass) || event.playerClass! < 1 || event.playerClass! > 9) throw new Error("Class-switch lifecycle class identity is invalid")
      const playerClass = event.playerClass!
      switches.push({
        playerClass,
        admission: admitted.has(playerClass) ? "retained" : "first",
        openedAt,
        selectedAt: event.at,
        selectionMilliseconds: openedAt === null ? null : Number((event.at - openedAt).toFixed(3)),
        panelMilliseconds: openedAt === null || panelAt === null ? null : Number(Math.max(0, panelAt - openedAt).toFixed(3)),
        fireAt: null,
        fireMilliseconds: null,
      })
      admitted.add(playerClass)
      openedAt = null
      panelAt = null
    } else if (event.phase === "weapon-fire") {
      const selected = switches.at(-1)
      if (selected && selected.fireAt === null) {
        selected.fireAt = event.at
        selected.fireMilliseconds = Number((event.at - selected.selectedAt).toFixed(3))
      }
    }
  }
  return switches
}
