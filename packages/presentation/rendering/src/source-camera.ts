// Adapted from Valve Source SDK 2013 view.cpp; the Source 1 SDK License applies.
const SOURCE_BASE_ASPECT = 4 / 3

export function sourceHorizontal4By3FovToVertical(horizontalFov4By3: number): number {
  if (!Number.isFinite(horizontalFov4By3) || horizontalFov4By3 <= 0 || horizontalFov4By3 >= 180) {
    throw new RangeError("Source horizontal-4:3 FOV is invalid")
  }
  return 2 * Math.atan(Math.tan((horizontalFov4By3 * Math.PI) / 360) / SOURCE_BASE_ASPECT) * 180 / Math.PI
}

export function sourceViewportDepthRange(input: readonly [number, number]): readonly [number, number] {
  if (
    input.length !== 2 ||
    !input.every(Number.isFinite) ||
    input[0] < 0 ||
    input[0] >= input[1] ||
    input[1] > 1
  ) {
    throw new RangeError("Source viewport depth range is invalid")
  }
  return Object.freeze([input[0], input[1]])
}
