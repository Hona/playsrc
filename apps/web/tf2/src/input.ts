const SOURCE_MOUSE_SENSITIVITY = 3
const SOURCE_MOUSE_YAW = 0.022
const SOURCE_MOUSE_PITCH = 0.022

export type PhysicalBinding = Readonly<{
  action: string
  code: string
  modifiers: number
}>

type PhysicalBindingResolution = Readonly<{ action: string; match: "exact" | "unmodified" }>

type IndexedPhysicalBinding = Readonly<{
  displayCode: string
  exact: Map<number, PhysicalBindingResolution>
  unmodified?: PhysicalBindingResolution
}>

export class PhysicalBindingIndex {
  readonly #codes = new Map<string, IndexedPhysicalBinding>()

  replace(bindings: readonly PhysicalBinding[]): void {
    this.#codes.clear()
    for (const binding of bindings) {
      const code = binding.code.toLowerCase()
      const existing = this.#codes.get(code)
      const exact = existing?.exact ?? new Map<number, PhysicalBindingResolution>()
      exact.set(binding.modifiers, Object.freeze({ action: binding.action, match: "exact" }))
      const unmodified = binding.modifiers === 0
        ? Object.freeze({ action: binding.action, match: "unmodified" as const })
        : existing?.unmodified
      this.#codes.set(code, { displayCode: binding.code, exact, ...(unmodified ? { unmodified } : {}) })
    }
  }

  resolve(code: string, modifiers: number): PhysicalBindingResolution | null {
    const binding = this.#codes.get(code.toLowerCase())
    return binding?.exact.get(modifiers) ?? binding?.unmodified ?? null
  }

  lookupBinding(action: string): string | null {
    for (const binding of this.#codes.values()) {
      for (const [modifiers, resolved] of binding.exact) {
        if (resolved.action !== action) continue
        return [modifiers & 1 ? "Shift" : "", modifiers & 2 ? "Ctrl" : "", modifiers & 4 ? "Alt" : "", binding.displayCode].filter(Boolean).join("+")
      }
    }
    return null
  }

  clear(): void {
    this.#codes.clear()
  }
}

export class PhysicalButtonState {
  readonly #physical = new Map<string, string>()
  readonly #actions = new Map<string, number>()

  press(identity: string, action: string): boolean {
    if (this.#physical.has(identity)) return false
    this.#physical.set(identity, action)
    const sources = this.#actions.get(action) ?? 0
    this.#actions.set(action, sources + 1)
    return sources === 0
  }

  release(identity: string): boolean {
    const action = this.#physical.get(identity)
    if (action === undefined) return false
    this.#physical.delete(identity)
    const sources = this.#actions.get(action)
    if (sources === undefined || sources <= 1) {
      this.#actions.delete(action)
      return true
    }
    this.#actions.set(action, sources - 1)
    return false
  }

  held(action: string): boolean {
    return this.#actions.has(action)
  }

  clear(): void {
    this.#physical.clear()
    this.#actions.clear()
  }
}

export function sourceMouseButtonCode(button: number): string | null {
  const sourceButton = ([1, 3, 2, 4, 5] as const)[button]
  return sourceButton === undefined ? null : `MOUSE${sourceButton}`
}

export function pointerLockRequestRequired(owner: Element | null, target: Element): boolean {
  return owner !== target
}

export function applyPointerDelta(
  yaw: number,
  pitch: number,
  movementX: number,
  movementY: number,
): Readonly<{ yaw: number; pitch: number }> {
  if (![yaw, pitch, movementX, movementY].every(Number.isFinite)) throw new TypeError("mouse view input is invalid")
  return Object.freeze({
    yaw: (yaw - movementX * SOURCE_MOUSE_SENSITIVITY * SOURCE_MOUSE_YAW) % 360,
    pitch: Math.max(-89, Math.min(89, pitch + movementY * SOURCE_MOUSE_SENSITIVITY * SOURCE_MOUSE_PITCH)),
  })
}

export function rebasePointerYaw(
  authoritativeYaw: number,
  sampledMovementX: number,
  currentMovementX: number,
): number {
  if (![authoritativeYaw, sampledMovementX, currentMovementX].every(Number.isFinite)) {
    throw new TypeError("mouse yaw rebase input is invalid")
  }
  return (authoritativeYaw - (currentMovementX - sampledMovementX) * SOURCE_MOUSE_SENSITIVITY * SOURCE_MOUSE_YAW) % 360
}

export function rawPointerMovementUnsupported(error: unknown): boolean {
  return error !== null && typeof error === "object" && "name" in error && error.name === "NotSupportedError"
}
