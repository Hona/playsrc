import type { FrameResult } from "@playsrc/rendering"

type FrameDiagnosticTarget = Readonly<{ dataset: DOMStringMap }>
type StaticPropScreen = FrameResult["runtimeStaticPropScreen"][number]
type SkyPass = FrameResult["sky3dPass"]

function sameNumbers(left: readonly number[] | undefined, right: readonly number[]): boolean {
  return left !== undefined && left.length === right.length
    && left.every((value, index) => Object.is(value, right[index]))
}

function sameScreens(left: readonly StaticPropScreen[] | undefined, right: readonly StaticPropScreen[]): boolean {
  return left !== undefined && left.length === right.length && left.every((screen, index) => {
    const next = right[index]!
    return screen.source === next.source
      && Object.is(screen.x, next.x)
      && Object.is(screen.y, next.y)
      && Object.is(screen.width, next.width)
      && Object.is(screen.height, next.height)
  })
}

function sameSky(left: SkyPass, right: SkyPass): boolean {
  if (!left || !right) return left === right
  return left.skySurfaces === right.skySurfaces
    && left.skyProps === right.skyProps
    && left.mainProps === right.mainProps
    && left.stateRestored === right.stateRestored
    && sameNumbers(left.visibleSkyPropSources, right.visibleSkyPropSources)
    && sameNumbers(left.fog.primary, right.fog.primary)
    && Object.is(left.fog.start, right.fog.start)
    && Object.is(left.fog.end, right.fog.end)
    && left.phases.length === right.phases.length
    && left.phases.every((phase, index) => phase === right.phases[index])
}

export class CanvasFrameDiagnostics {
  #mainSources?: readonly number[]
  #skyPass?: SkyPass
  #screens?: readonly StaticPropScreen[]
  #published = false

  publish(target: FrameDiagnosticTarget, frame: Pick<FrameResult, "visibleMainStaticPropSources" | "sky3dPass" | "runtimeStaticPropScreen">): void {
    if (!sameNumbers(this.#mainSources, frame.visibleMainStaticPropSources)) {
      target.dataset.visibleMainStaticProps = JSON.stringify(frame.visibleMainStaticPropSources)
      this.#mainSources = frame.visibleMainStaticPropSources
    }
    if (!this.#published || !sameSky(this.#skyPass, frame.sky3dPass)) {
      target.dataset.sky3dPass = frame.sky3dPass ? JSON.stringify(frame.sky3dPass) : ""
      this.#skyPass = frame.sky3dPass
    }
    if (!sameScreens(this.#screens, frame.runtimeStaticPropScreen)) {
      target.dataset.runtimeStaticPropScreen = JSON.stringify(frame.runtimeStaticPropScreen)
      this.#screens = frame.runtimeStaticPropScreen
    }
    this.#published = true
  }

  clear(): void {
    this.#mainSources = undefined
    this.#skyPass = undefined
    this.#screens = undefined
    this.#published = false
  }
}
