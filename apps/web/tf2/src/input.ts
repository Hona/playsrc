const LOOK_SENSITIVITY = 0.08

export function applyPointerDelta(
  yaw: number,
  pitch: number,
  movementX: number,
  movementY: number,
): Readonly<{ yaw: number; pitch: number }> {
  return Object.freeze({
    yaw: (yaw - movementX * LOOK_SENSITIVITY) % 360,
    pitch: Math.max(-89, Math.min(89, pitch + movementY * LOOK_SENSITIVITY)),
  })
}
