const SOURCE_MOUSE_SENSITIVITY = 3
const SOURCE_MOUSE_YAW = 0.022
const SOURCE_MOUSE_PITCH = 0.022

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
