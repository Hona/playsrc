import { expect, test } from "bun:test"
import { CLASS_CORRECTNESS_PLAN, installClassCorrectnessObserver } from "../profile/class-input-correctness"
import { installBrowserFrameProfiler } from "../profile/browser-frame-profiler"

function fixture() {
  const events = new Map<string, (event: any) => void>()
  const gpu = { prototype: { requestAdapter: () => Promise.resolve({}) } }
  const host: any = { GPU: gpu, performance: { now: () => 1 },
    document: { addEventListener: (name: string, callback: any) => events.set(name, callback), pointerLockElement: null } }
  const original = gpu.prototype.requestAdapter
  const profile = installBrowserFrameProfiler(host, "lifecycle")
  expect(gpu.prototype.requestAdapter).toBe(original)
  return { host, profile, observer: installClassCorrectnessObserver(host), events }
}

test("correctness-only coverage has three bounded windows, all nine classes twice, and no CPU/performance contract", () => {
  expect(CLASS_CORRECTNESS_PLAN.windowSizes).toEqual([6, 6, 6])
  expect(CLASS_CORRECTNESS_PLAN.maximumWindowMilliseconds).toBe(10_000)
  expect(CLASS_CORRECTNESS_PLAN.classes.map(value => value.identity)).toEqual([6, 7, 5, 8, 9, 2, 1, 4, 3, 6, 7, 5, 8, 9, 2, 1, 4, 3])
  expect(CLASS_CORRECTNESS_PLAN).toMatchObject({ cpuSampling: false, nativeTrace: false, heapSampling: false, performanceSample: false })
})

test("primary acknowledgement requires a newer Rust tick inside a matching active player's real publication", () => {
  const { profile, observer } = fixture()
  const before = observer.before()
  const record = { requestId: 9, at: 25, replayAttack: { hostTick: "42", playerClass: 6, weapon: 9, lifecycle: 1 },
    publications: [{ selectedTicks: 1, firstHostTick: "42", lastHostTick: "42", player: { tick: "41", playerClass: 6 }, weapons: [{ weapon: 9 }], activities: [] }] }
  profile.simulation.push(record)
  expect(observer.acknowledgement(before, 6)).toMatchObject({ hostTick: "42", playerTick: "41", requestId: 9, weapon: 9 })
  expect(observer.acknowledgement(observer.before(), 6)).toBeNull()
  expect(observer.acknowledgement(before, 7)).toBeNull()
  for (const mutation of [
    (value: typeof record) => { value.replayAttack.lifecycle = 2 },
    (value: typeof record) => { value.publications[0]!.selectedTicks = 0 },
    (value: typeof record) => { value.publications[0]!.player.playerClass = 7 },
    (value: typeof record) => { value.publications[0]!.firstHostTick = "43" },
    (value: typeof record) => { value.publications[0]!.lastHostTick = "41" },
    (value: typeof record) => { value.publications[0]!.weapons = [] },
  ]) {
    const invalid = structuredClone(record); mutation(invalid); profile.simulation = [invalid]
    expect(observer.acknowledgement(before, 6)).toBeNull()
  }
})

test("prefire proof is a real Heavy activity and observation never cancels native input", () => {
  const { host, profile, observer, events } = fixture()
  profile.simulation.push({ publications: [{ player: { playerClass: 6 }, activities: [{ tick: "1209", weapon: 9, activity: 12 }] }] })
  expect(observer.prefire({ index: 0 })).toMatchObject({ tick: "1209", weapon: 9, activity: 12 })
  profile.simulation[0].publications[0].player.playerClass = 7
  expect(observer.prefire({ index: 0 })).toBeNull()
  const target = { matches: () => true }
  observer.active = true; observer.action = "attack"; host.document.pointerLockElement = target
  events.get("pointerdown")!({ target, button: 0, isTrusted: true, preventDefault() { throw new Error("must not intercept") } })
  expect(observer.events).toEqual([{ at: 1, phase: "weapon-fire", controllerAction: "attack", button: 0, trusted: true }])
})
