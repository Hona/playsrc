const SOURCE_MOUSE_SENSITIVITY = 3
const SOURCE_MOUSE_YAW = 0.022
const SOURCE_MOUSE_PITCH = 0.022

export type PhysicalBinding = Readonly<{
  action: string
  code: string
  modifiers: number
}>

export function resolvePhysicalBinding<Candidate>(
  code: string,
  modifiers: number,
  candidates: readonly Candidate[],
  read: (candidate: Candidate) => PhysicalBinding | null,
): Readonly<{ action: string; match: "exact" | "unmodified" }> | null {
  let unmodified: string | null = null
  for (const candidate of candidates) {
    const binding = read(candidate)
    if (binding === null) continue
    if (binding.code.toLowerCase() !== code.toLowerCase()) continue
    if (binding.modifiers === modifiers) return Object.freeze({ action: binding.action, match: "exact" })
    if (binding.modifiers === 0) unmodified = binding.action
  }
  return unmodified === null ? null : Object.freeze({ action: unmodified, match: "unmodified" })
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
