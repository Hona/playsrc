import type { ClassSwitchLifecycleEvent } from "./class-switch-lifecycle"

export const CLASS_INPUT_SEQUENCE = Object.freeze([
  { name: "heavyweapons", digit: 5, identity: 6 }, { name: "pyro", digit: 3, identity: 7 },
  { name: "medic", digit: 7, identity: 5 }, { name: "spy", digit: 9, identity: 8 },
  { name: "engineer", digit: 6, identity: 9 }, { name: "sniper", digit: 8, identity: 2 },
  { name: "scout", digit: 1, identity: 1 }, { name: "demoman", digit: 4, identity: 4 },
  { name: "soldier", digit: 2, identity: 3 },
].map(value => Object.freeze(value)))

/** Observe interference; never cancel, rewrite, or suppress real input. */
export function classInputViolations(events: readonly ClassSwitchLifecycleEvent[]) {
  return events.filter(event => {
    if (event.phase === "key-down") {
      return event.trusted !== true || !(event.controllerAction === "scoreboard" && event.key === "Tab"
        || event.controllerAction === "select" && (event.key === "Comma" || /^Digit[1-9]$/u.test(event.key ?? "")))
    }
    if (event.phase === "pointer-capture" || event.phase === "weapon-fire") {
      return event.trusted !== true || event.button !== 0
        || event.controllerAction !== (event.phase === "pointer-capture" ? "capture" : "attack")
    }
    return event.phase === "other-pointer-button"
  })
}

/** All setup and native waits stay inside the same bounded sample clock. */
export async function prepareClassCapture(options: {
  earliestCapture: number
  deadline: number
  now(): number
  delay(milliseconds: number): Promise<void>
  select(): Promise<boolean>
}): Promise<boolean> {
  if (options.now() >= options.deadline || options.earliestCapture >= options.deadline) return false
  // Opening/acknowledging the next class does not request pointer lock. Run its
  // real UI, rendering and Source deploy concurrently with the native limiter.
  if (!await options.select()) return false
  const wait = Math.max(0, options.earliestCapture - options.now())
  if (options.now() + wait >= options.deadline) return false
  if (wait) await options.delay(wait)
  return options.now() < options.deadline
}
