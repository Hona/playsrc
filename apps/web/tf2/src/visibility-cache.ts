export type SkyVisibilityIdentity = Readonly<{
  generation: number
  viewportRevision: number
  tick: bigint
  position: readonly [number, number, number]
  origin: readonly [number, number, number]
  area: number
  yawDegrees: number
  pitchDegrees: number
  verticalFovDegrees: number
  near: number
  far: number
}>

function sameVector(left: readonly number[], right: readonly number[]): boolean {
  return left.length === right.length && left.every((value, index) => Object.is(value, right[index]))
}

export function sameSkyVisibilityIdentity(left: SkyVisibilityIdentity, right: SkyVisibilityIdentity): boolean {
  return left.generation === right.generation
    && left.viewportRevision === right.viewportRevision
    && left.tick === right.tick
    && left.area === right.area
    && Object.is(left.yawDegrees, right.yawDegrees)
    && Object.is(left.pitchDegrees, right.pitchDegrees)
    && Object.is(left.verticalFovDegrees, right.verticalFovDegrees)
    && Object.is(left.near, right.near)
    && Object.is(left.far, right.far)
    && sameVector(left.position, right.position)
    && sameVector(left.origin, right.origin)
}

export class ExactSkyVisibilityCache<Value> {
  #entry?: Readonly<{ identity: SkyVisibilityIdentity; value: Value }>

  read(identity: SkyVisibilityIdentity): Value | undefined {
    return this.#entry && sameSkyVisibilityIdentity(this.#entry.identity, identity)
      ? this.#entry.value
      : undefined
  }

  write(identity: SkyVisibilityIdentity, value: Value): void {
    this.#entry = { identity, value }
  }

  clear(): void {
    this.#entry = undefined
  }
}
