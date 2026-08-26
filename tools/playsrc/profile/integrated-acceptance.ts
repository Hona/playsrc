export const ACCEPTANCE_SCENARIOS = Object.freeze({
  "training-dpr1": { profile: "class-switch-high-dpi", dpr: "1" },
  "training-dpr1.25": { profile: "class-switch-high-dpi", dpr: "1.25" },
  "training-dpr1.5": { profile: "class-switch-high-dpi", dpr: "1.5" },
  "training-dpr2": { profile: "class-switch-high-dpi", dpr: "2" },
  "stock-dpr1": { profile: "class-switch-high-dpi", dpr: "1" },
  "stock-dpr1.25": { profile: "class-switch-high-dpi", dpr: "1.25" },
  "stock-dpr1.5": { profile: "class-switch-high-dpi", dpr: "1.5" },
  "stock-dpr2": { profile: "class-switch-high-dpi", dpr: "2" },
  "2fort": { profile: "2fort-startup", dpr: "2" },
  engineer: { profile: "engineer", dpr: "2" },
  lifecycle: { profile: "integrated-lifecycle", dpr: "2" },
} as const)

export function acceptanceScenario(name: string) {
  if (!Object.hasOwn(ACCEPTANCE_SCENARIOS, name)) throw new Error(`Choose one bounded scenario: ${Object.keys(ACCEPTANCE_SCENARIOS).join(" | ")}`)
  return ACCEPTANCE_SCENARIOS[name as keyof typeof ACCEPTANCE_SCENARIOS]
}

// Comparison is deliberately strict. A Mac run, a different roster, cold vs
// warm state, or a changed drawing buffer is not a Windows before/after pair.
export function compareAcceptance(before: any, after: any) {
  const keys = (report: any) => ({
    target: report.target, entry: report.entry, launch: report.launch,
    platform: report.browser.platform, userAgent: report.browser.userAgent,
    channel: report.browser.channel, viewport: report.browser.viewport,
    devices: report.gpu.chromiumDevices, adapter: report.gpu.adapter,
    settings: report.settings, cache: report.loads.map((load: any) => load.cache),
    classes: report.classSwitches.requested, activeBots: report.activeBots, roster: report.roster,
  })
  const left = keys(before), right = keys(after)
  const mismatches = Object.keys(left).filter(key => JSON.stringify(left[key as keyof typeof left]) !== JSON.stringify(right[key as keyof typeof right]))
  if (!before.browser.userAgent || !after.browser.userAgent || !before.settings || !after.settings || !before.roster || !after.roster) mismatches.push("missing-comparison-metadata")
  const metrics = (report: any) => ({
    compositorP95: report.compositor.intervals?.p95Milliseconds ?? null,
    compositorP99: report.compositor.intervals?.p99Milliseconds ?? null,
    compositorMax: report.compositor.intervals?.maximumMilliseconds ?? null,
    simulationHz: report.simulation.hertz,
    queueWriteBytes: report.gpu.queueWriteBytes,
    rssGrowthBytes: report.memory.residentBeforeBytes === null || report.memory.residentAfterBytes === null ? null : report.memory.residentAfterBytes - report.memory.residentBeforeBytes,
    firstPlayableMilliseconds: report.loads.at(-1)?.criticalPath.firstPlayableFrameMilliseconds ?? null,
  })
  return { comparable: mismatches.length === 0, mismatches, before: metrics(before), after: metrics(after),
    conclusion: mismatches.length ? "Not comparable; no improvement claim" : "Comparable observations, not a statistical non-regression proof" }
}
