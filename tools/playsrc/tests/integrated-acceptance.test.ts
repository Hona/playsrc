import { expect, test } from "bun:test"
import { acceptanceScenario, compareAcceptance } from "../profile/integrated-acceptance"
import { profileLockWaitBudget } from "../src/profile-runner"

test("acceptance reserves real scenario time rather than starting a doomed profile after a long lock wait", () => {
  expect(profileLockWaitBudget(1000, 120_000)).toBe(54_000)
  expect(profileLockWaitBudget(1000)).toBe(174_000)
  expect(() => profileLockWaitBudget(0, 175_000)).toThrow("reservation")
})

test("acceptance selects one fixed-roster bounded headed scenario", () => {
  expect(acceptanceScenario("training-dpr1.25")).toEqual({ profile: "class-switch-high-dpi", dpr: "1.25" })
  expect(() => acceptanceScenario("all")).toThrow("one bounded scenario")
})

test("comparison refuses cross-machine, quality, cache, roster and viewport claims", () => {
  const before = { target: "pl_upward", entry: "training", launch: { players: 16 }, browser: { platform: "darwin", userAgent: "Edge", viewport: { width: 1280, height: 720, devicePixelRatio: 2 } }, gpu: { chromiumDevices: ["GPU"], queueWriteBytes: 120 }, settings: { hdr: 2 }, loads: [{ cache: "cold", criticalPath: {} }], classSwitches: { requested: [1, 2] }, activeBots: 15, compositor: { intervals: { maximumMilliseconds: 505 } }, simulation: { hertz: 66.66 }, memory: { residentBeforeBytes: null, residentAfterBytes: null } }
  Object.assign(before, { roster: [{ identity: 2, class: 1, team: 2, difficulty: 1 }] })
  expect(compareAcceptance(before, structuredClone(before))).toMatchObject({ comparable: true, before: { compositorMax: 505, rssGrowthBytes: null } })
  for (const change of [ { browser: { ...before.browser, platform: "win32" } }, { settings: { hdr: 0 } }, { activeBots: 23 }, { loads: [{ cache: "warm", criticalPath: {} }] }, { browser: { ...before.browser, viewport: { width: 1280, height: 720, devicePixelRatio: 1 } } } ]) {
    expect(compareAcceptance(before, { ...before, ...change }).comparable).toBe(false)
  }
})
