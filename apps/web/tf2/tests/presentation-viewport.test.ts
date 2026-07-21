import { describe, expect, test } from "bun:test"
import {
  initializePresentationViewportOwner,
  type ApplicationPresentationViewport,
  type PresentationViewportPlatform,
} from "../src/presentation-viewport"

class Target {
  readonly listeners = new Map<string, Set<() => void>>()
  addEventListener(type: string, listener: () => void): void {
    const listeners = this.listeners.get(type) ?? new Set()
    listeners.add(listener)
    this.listeners.set(type, listeners)
  }
  removeEventListener(type: string, listener: () => void): void { this.listeners.get(type)?.delete(listener) }
  dispatch(type: string): void { for (const listener of [...(this.listeners.get(type) ?? [])]) listener() }
  count(): number { return [...this.listeners.values()].reduce((total, listeners) => total + listeners.size, 0) }
}

function fixture(initial = { width: 1280, height: 720, devicePixelRatio: 1 }) {
  let measurement = initial
  let nextFrame = 1
  const frames = new Map<number, () => void>()
  const visualViewport = new Target()
  const orientation = new Target()
  const document = new Target()
  const resolutionQueries: Target[] = []
  let resizeCallback: (() => void) | null = null
  let observers = 0
  const platform: PresentationViewportPlatform = {
    measure: () => measurement,
    requestFrame(callback) { const handle = nextFrame++; frames.set(handle, callback); return handle },
    cancelFrame(handle) { frames.delete(handle) },
    observeResize(callback) { resizeCallback = callback; observers += 1; return { disconnect() { resizeCallback = null; observers -= 1 } } },
    visualViewport,
    orientation,
    document,
    resolutionQuery() { const target = new Target(); resolutionQueries.push(target); return target },
  }
  const publications: ApplicationPresentationViewport[] = []
  let suspensions = 0
  const owner = initializePresentationViewportOwner({ platform, onViewport: (viewport) => publications.push(viewport), onSuspended: () => { suspensions += 1 } })
  const flush = () => { const pending = [...frames.values()]; frames.clear(); for (const callback of pending) callback() }
  return {
    owner,
    publications,
    visualViewport,
    orientation,
    document,
    resolutionQueries,
    flush,
    resize: () => resizeCallback?.(),
    set: (next: typeof measurement) => { measurement = next },
    resources: () => ({ frames: frames.size, observers, listeners: visualViewport.count() + orientation.count() + document.count() + resolutionQueries.reduce((sum, query) => sum + query.count(), 0) }),
  }
}

describe("TF2 application presentation viewport owner", () => {
  test("coalesces every browser notification into one immutable positive viewport", async () => {
    const value = fixture()
    value.resize()
    value.visualViewport.dispatch("resize")
    value.orientation.dispatch("change")
    value.document.dispatch("fullscreenchange")
    value.document.dispatch("pointerlockchange")
    expect(value.owner.snapshot().pendingFrames).toBe(1)
    value.flush()
    expect(await value.owner.first()).toEqual({ width: 1280, height: 720, devicePixelRatio: 1, revision: 1 })
    expect(value.publications).toHaveLength(1)
    expect(Object.isFrozen(value.publications[0])).toBeTrue()
    expect(value.owner.snapshot()).toMatchObject({ measurements: 1, publications: 1, suspensions: 0 })
  })

  test("makes duplicates inert, suspends zero size, rearms DPR and restores exactly", () => {
    const value = fixture()
    value.flush()
    const initial = value.owner.snapshot()
    value.resize()
    value.flush()
    expect(value.owner.snapshot()).toMatchObject({ viewport: initial.viewport, publications: 1 })

    value.set({ width: 0, height: 0, devicePixelRatio: 1 })
    value.resize()
    value.flush()
    expect(value.owner.snapshot()).toMatchObject({ viewport: null, publications: 1, suspensions: 1 })

    value.set({ width: 1024, height: 768, devicePixelRatio: 2 })
    value.visualViewport.dispatch("resize")
    value.flush()
    expect(value.publications.at(-1)).toEqual({ width: 1024, height: 768, devicePixelRatio: 2, revision: 2 })
    expect(value.resolutionQueries).toHaveLength(2)
    expect(value.resolutionQueries[0]!.count()).toBe(0)
    expect(value.resolutionQueries[1]!.count()).toBe(1)
  })

  test("destroys the observer, listeners, DPR query and pending frame exactly once", () => {
    const value = fixture()
    value.flush()
    value.resize()
    expect(value.resources().frames).toBe(1)
    value.owner.destroy()
    value.owner.destroy()
    expect(value.owner.snapshot()).toMatchObject({ lifecycle: "destroyed", pendingFrames: 0, listeners: 0, observers: 0 })
    expect(value.resources()).toEqual({ frames: 0, observers: 0, listeners: 0 })
  })

  test("reports initial zero-size suspension and waits for the first positive box", async () => {
    const value = fixture({ width: 0, height: 0, devicePixelRatio: 1 })
    value.flush()
    expect(value.owner.snapshot()).toMatchObject({ viewport: null, publications: 0, suspensions: 1 })
    value.resize()
    value.flush()
    expect(value.owner.snapshot().suspensions).toBe(1)
    value.set({ width: 390, height: 844, devicePixelRatio: 1 })
    value.resize()
    value.flush()
    expect(await value.owner.first()).toEqual({ width: 390, height: 844, devicePixelRatio: 1, revision: 1 })
  })
})
