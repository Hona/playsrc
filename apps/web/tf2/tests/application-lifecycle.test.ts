import { describe, expect, test } from "bun:test"
import { TF2_MAIN_MENU_STATE, transitionTf2GameUi } from "@playsrc/game-tf2-browser/gameui"
import {
  ApplicationFrameClock,
  ApplicationOperationLedger,
  PredictedEyeInterpolation,
  composeViewmodelTransform,
  currentPresentationGeneration,
  routeApplicationEscape,
  selectAuthoredSky,
} from "../src/application-lifecycle"

describe("TF2 application lifecycle ownership", () => {
  test("admits one nondecreasing frame clock across stale animation callbacks and owner creation", () => {
    const clock = new ApplicationFrameClock()
    expect(clock.admit(4)).toBe(4)
    expect(clock.admit(4.125)).toBe(4.125)
    expect(clock.admit(4.1)).toBe(4.125)
    expect(clock.admit(4.25)).toBe(4.25)
    for (const value of [Number.NaN, Number.POSITIVE_INFINITY, -1]) {
      expect(() => clock.admit(value)).toThrow("Application frame timestamp is invalid")
    }
    expect(clock.current).toBe(4.25)
  })

  test("invalidates only superseded map-operation generations and aborts stale work", () => {
    const ledger = new ApplicationOperationLedger()
    const initial = ledger.begin()
    expect(initial.generation).toBe(1)
    expect(ledger.current(initial)).toBe(true)
    const replacement = ledger.begin()
    expect(initial.signal.aborted).toBe(true)
    expect(ledger.current(initial)).toBe(false)
    expect(replacement.generation).toBe(2)
    expect(ledger.current(replacement)).toBe(true)
    expect(ledger.complete(initial)).toBe(false)
    expect(ledger.current(replacement)).toBe(true)
    expect(ledger.complete(replacement)).toBe(true)
    expect(ledger.current(replacement)).toBe(false)
    expect(replacement.signal.aborted).toBe(false)
    const closed = ledger.begin()
    ledger.cancel()
    expect(closed.signal.aborted).toBe(true)
    expect(ledger.current(closed)).toBe(false)
  })

  test("rejects stale mapper and encoder identities before resumed presentation can mutate them", () => {
    const originalMapper = { mapped: 0 }
    const originalEncoder = { encoded: 0 }
    const owned = Object.freeze({ generation: 4, mapper: originalMapper, encoder: originalEncoder })
    expect(currentPresentationGeneration(owned, 4, originalMapper, originalEncoder)).toBe(true)
    expect(currentPresentationGeneration(owned, 5, originalMapper, originalEncoder)).toBe(false)
    expect(currentPresentationGeneration(owned, 4, { mapped: 0 }, originalEncoder)).toBe(false)
    expect(currentPresentationGeneration(owned, 4, originalMapper, { encoded: 0 })).toBe(false)
    expect(currentPresentationGeneration(owned, 4, undefined, originalEncoder)).toBe(false)
  })

  test("draws an authored 3D sky only when the classified view has one real controller", () => {
    expect(selectAuthoredSky(0, false)).toBe(false)
    expect(selectAuthoredSky(1, false)).toBe(false)
    expect(selectAuthoredSky(2, false)).toBe(false)
    expect(selectAuthoredSky(0, true)).toBe(false)
    expect(selectAuthoredSky(1, true)).toBe(false)
    expect(selectAuthoredSky(2, true)).toBe(true)
  })

  test("routes exactly one physical Escape edge between Options, gameplay, and pause", () => {
    expect(routeApplicationEscape({ code: "Escape", repeat: false, phase: "Ready", gameUi: "in-game", optionsVisible: false })).toBe("activate")
    expect(routeApplicationEscape({ code: "Escape", repeat: false, phase: "Ready", gameUi: "pause", optionsVisible: false })).toBe("resume")
    expect(routeApplicationEscape({ code: "Escape", repeat: false, phase: "Ready", gameUi: "pause", optionsVisible: true })).toBe("options")
    expect(routeApplicationEscape({ code: "Escape", repeat: false, phase: "MainMenu", gameUi: "main-menu", optionsVisible: true })).toBe("options")
    expect(routeApplicationEscape({ code: "Escape", repeat: true, phase: "Ready", gameUi: "in-game", optionsVisible: false })).toBe("ignore")
    expect(routeApplicationEscape({ code: "KeyW", repeat: false, phase: "Ready", gameUi: "in-game", optionsVisible: false })).toBe("ignore")
    expect(routeApplicationEscape({ code: "Escape", repeat: false, phase: "Loading", gameUi: "loading", optionsVisible: false })).toBe("ignore")
  })

  test("keeps the game-owned second Escape pending until the application acknowledges hiding", () => {
    const loading = transitionTf2GameUi(TF2_MAIN_MENU_STATE, { kind: "loading-started", mapIdentity: "jump_beef" })
    const playing = transitionTf2GameUi(loading.state, { kind: "loading-succeeded" })
    const opened = transitionTf2GameUi(playing.state, { kind: "escape" })
    expect(opened.state.kind).toBe("pause")
    expect(opened.request).toBeNull()
    const resume = transitionTf2GameUi(opened.state, { kind: "escape" })
    expect(resume.state.kind).toBe("pause")
    expect(resume.request).toEqual({ kind: "resume-game" })
    const duplicate = transitionTf2GameUi(resume.state, { kind: "escape" })
    expect(duplicate.disposition).toBe("ignored")
    expect(duplicate.request).toBeNull()
    const acknowledged = transitionTf2GameUi(resume.state, { kind: "gameui-hidden" })
    expect(acknowledged.state.kind).toBe("in-game")
  })
})

describe("Source predicted local-eye presentation", () => {
  test("uses only the Simulation-owned fraction between consecutive authoritative ticks", () => {
    const interpolation = new PredictedEyeInterpolation()
    const first = Object.freeze([10, 20, 30] as const)
    const second = Object.freeze([13, 14, 45] as const)
    interpolation.reset(12n, first)
    expect(interpolation.sample(0.75)).toBe(first)
    interpolation.admit(13n, second)
    expect(interpolation.sample(0)).toBe(first)
    expect(interpolation.sample(0.5)).toEqual([11.5, 17, 37.5])
    expect(interpolation.sample(1)).toBe(second)
    expect(first).toEqual([10, 20, 30])
    expect(second).toEqual([13, 14, 45])
  })

  test("retains the last two tick samples from one coalesced host publication", () => {
    const interpolation = new PredictedEyeInterpolation()
    interpolation.reset(7n, Object.freeze([0, 0, 0] as const))
    interpolation.admit(8n, Object.freeze([1, 2, 3] as const))
    interpolation.admit(9n, Object.freeze([2, 4, 6] as const))
    expect(interpolation.sample(0)).toEqual([1, 2, 3])
    expect(interpolation.sample(0.5)).toEqual([1.5, 3, 4.5])
    expect(interpolation.sample(1)).toEqual([2, 4, 6])
  })

  test("snaps teleports, resets replacement generations, and freezes suspension", () => {
    const interpolation = new PredictedEyeInterpolation()
    interpolation.reset(30n, Object.freeze([1, 1, 1] as const))
    interpolation.admit(31n, Object.freeze([2, 2, 2] as const))
    interpolation.admit(32n, Object.freeze([900, -300, 70] as const), true)
    expect(interpolation.sample(0)).toEqual([900, -300, 70])
    expect(interpolation.sample(0.5)).toEqual([900, -300, 70])
    interpolation.admit(33n, Object.freeze([901, -299, 71] as const))
    interpolation.suspend()
    expect(interpolation.sample(0)).toEqual([901, -299, 71])
    const replacement = Object.freeze([-7, 8, 9] as const)
    interpolation.reset(1n, replacement)
    expect(interpolation.sample(0.25)).toBe(replacement)
    interpolation.clear()
    expect(interpolation.sample(0)).toBeUndefined()
  })

  test("rejects malformed samples, phases, and reversed authoritative ticks", () => {
    const interpolation = new PredictedEyeInterpolation()
    expect(() => interpolation.reset(0n, Object.freeze([0, Number.NaN, 0] as const))).toThrow()
    interpolation.reset(3n, Object.freeze([0, 0, 0] as const))
    expect(() => interpolation.admit(3n, Object.freeze([1, 0, 0] as const))).toThrow("Predicted eye tick reversed")
    for (const phase of [-1, 1.0001, Number.NaN, Number.POSITIVE_INFINITY]) {
      expect(() => interpolation.sample(phase)).toThrow("Predicted eye interpolation phase is invalid")
    }
  })
})

describe("default TF2 rigid first-person viewmodel alignment", () => {
  test("composes authored eye-local bob with the current Source forward/right/up basis", () => {
    const eye = Object.freeze({
      position: Object.freeze([100, 200, 300] as const),
      yawDegrees: 90,
      pitchDegrees: 0,
    })
    const local = Object.freeze({
      position: Object.freeze([2, 3, 4] as const),
      angles: Object.freeze([1, 5, 7] as const),
    })
    const world = composeViewmodelTransform(local, eye)
    expect(world.position[0]).toBeCloseTo(103, 12)
    expect(world.position[1]).toBeCloseTo(202, 12)
    expect(world.position[2]).toBeCloseTo(304, 12)
    expect(world.angles).toEqual([1, 95, 7])
    expect(local).toEqual({ position: [2, 3, 4], angles: [1, 5, 7] })
  })

  test("carries exact pitch/yaw changes through camera parenting without changing local pose", () => {
    const local = Object.freeze({ position: Object.freeze([1, 0, 0] as const), angles: Object.freeze([3, 4, 9] as const) })
    const before = composeViewmodelTransform(local, { position: [10, 20, 30], yawDegrees: 179, pitchDegrees: -5 })
    const after = composeViewmodelTransform(local, { position: [100, 200, 300], yawDegrees: -179, pitchDegrees: 15 })
    expect(before.angles).toEqual([-2, 183, 9])
    expect(after.angles).toEqual([18, -175, 9])
    expect(Math.hypot(after.position[0] - 100, after.position[1] - 200, after.position[2] - 300)).toBeCloseTo(1, 12)
    expect(local).toEqual({ position: [1, 0, 0], angles: [3, 4, 9] })
  })

  test("preserves a zero local transform at the exact eye and rejects malformed samples", () => {
    const eye = { position: Object.freeze([3, 4, 5] as const), yawDegrees: 45, pitchDegrees: -10 }
    const local = { position: Object.freeze([0, 0, 0] as const), angles: Object.freeze([0, 0, 0] as const) }
    expect(composeViewmodelTransform(local, eye)).toEqual({ position: [3, 4, 5], angles: [-10, 45, 0] })
    expect(() => composeViewmodelTransform(local, { ...eye, yawDegrees: Number.NaN })).toThrow("Viewmodel display transform is invalid")
  })
})
