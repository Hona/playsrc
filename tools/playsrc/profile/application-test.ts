import { test as base, expect } from "@playwright/test"
import { ProfilePhases } from "./profile-phases"

const headedBrowser = process.env.PLAYSRC_PROFILE_CDP_ENDPOINT
  ? base.extend<{}, { browser: import("@playwright/test").Browser }>({
      browser: [async ({ playwright }, use) => {
        const browser = await playwright.chromium.connectOverCDP(process.env.PLAYSRC_PROFILE_CDP_ENDPOINT!, { timeout: 20_000 })
        try { await use(browser) }
        finally { await browser.close() }
      }, { scope: "worker" }],
    })
  : base

type ApplicationState = Readonly<{
  phase: string
  detail: string
  gameUi: string
  startupState: string
  consoleOutput: string
  blockers: string
}>

export const test = headedBrowser.extend<{
  applicationDiagnostics: void
  allowRecoverableApplicationFailure: boolean
  preserveStartupMovie: boolean
  profilePhases: ProfilePhases
}>({
  profilePhases: [async ({}, use, testInfo) => {
    const phases = new ProfilePhases()
    try { await use(phases) }
    finally { await testInfo.attach("profile-operation-phases", { body: JSON.stringify(phases.finish(testInfo.status === "passed")), contentType: "application/json" }) }
  }, { auto: true }],
  allowRecoverableApplicationFailure: [false, { option: true }],
  preserveStartupMovie: [false, { option: true }],
  applicationDiagnostics: [async ({ page, allowRecoverableApplicationFailure, preserveStartupMovie }, use, testInfo) => {
    const started = Date.now()
    const transitions: Array<{ milliseconds: number; phase: string; detail: string; startupState: string }> = []
    let rejectFailure: (error: Error) => void = () => {}
    let stallTimer: ReturnType<typeof setTimeout> | undefined
    let lastState: ApplicationState | undefined
    let startupSkipRequested = false
    let finished = false
    const failure = new Promise<never>((_, reject) => { rejectFailure = reject })
    const fail = (message: string) => {
      if (finished) return
      finished = true
      rejectFailure(new Error(message))
    }
    await page.exposeBinding("__playsrcApplicationState", async (_source, state: ApplicationState) => {
      const previous = transitions.at(-1)
      if (!previous || previous.phase !== state.phase || previous.detail !== state.detail || previous.startupState !== state.startupState) {
        transitions.push({ milliseconds: Date.now() - started, phase: state.phase, detail: state.detail, startupState: state.startupState })
      }
      lastState = state
      if (stallTimer) clearTimeout(stallTimer)
      if (state.startupState === "Preparing") startupSkipRequested = false
      if (!preserveStartupMovie && !startupSkipRequested && state.phase === "Startup"
        && (state.startupState === "Playing" || state.startupState === "AwaitingGesture")) {
        startupSkipRequested = true
        await page.keyboard.press("Escape")
        if (lastState !== state) return
      }
      if (state.phase === "Failed") {
        if (!allowRecoverableApplicationFailure) {
          fail(`TF2 application failed: ${state.detail}\nIn-game console:\n${state.consoleOutput || "<not mounted>"}\nBlockers: ${state.blockers}`)
        }
        return
      }
      if (["Startup", "Loading", "Replacing"].includes(state.phase)) {
        const stallSeconds = process.env.PROFILE_2FORT_MEMORY === "1" ? 150 : process.env.PROFILE_PYRO_STOCK === "1" || process.env.PROFILE_COMBAT === "1" || process.env.PROFILE_MEDIC_WEAPONS === "1" ? 180 : 65
        stallTimer = setTimeout(() => fail(`TF2 application stalled for ${stallSeconds} seconds in ${state.phase}: ${state.detail}\nIn-game console:\n${state.consoleOutput || "<not mounted>"}`), stallSeconds * 1_000)
      }
    })
    await page.addInitScript(() => {
      const report = () => {
        const main = document.querySelector<HTMLElement>("main")
        if (!main) return
        void (globalThis as typeof globalThis & { __playsrcApplicationState(state: ApplicationState): Promise<void> }).__playsrcApplicationState({
          phase: main.dataset.phase ?? "Absent",
          detail: main.dataset.detail ?? "",
          gameUi: main.dataset.gameui ?? "",
          startupState: main.dataset.startupState ?? "",
          consoleOutput: document.querySelector<HTMLElement>("[aria-label='Console output']")?.innerText ?? "",
          blockers: main.dataset.blockers ?? "[]",
        })
      }
      const install = () => {
        const main = document.querySelector<HTMLElement>("main")
        if (!main) { setTimeout(install, 0); return }
        new MutationObserver(report).observe(main, { attributes: true, attributeFilter: ["data-phase", "data-detail", "data-gameui", "data-startup-state", "data-blockers"] })
        report()
      }
      if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", install, { once: true })
      else install()
    })
    try {
      await Promise.race([use(), failure])
    } finally {
      finished = true
      if (stallTimer) clearTimeout(stallTimer)
      if (lastState) await testInfo.attach("terminal-application-state", { body: JSON.stringify(lastState, null, 2), contentType: "application/json" })
      const maps: Array<{ detail: string; startedMilliseconds: number; readyMilliseconds: number; durationMilliseconds: number }> = []
      let loading: { detail: string; milliseconds: number } | undefined
      for (const transition of transitions) {
        if (["Loading", "Replacing"].includes(transition.phase) && !loading) loading = transition
        else if (transition.phase === "Ready" && loading) {
          maps.push({ detail: transition.detail, startedMilliseconds: loading.milliseconds, readyMilliseconds: transition.milliseconds, durationMilliseconds: transition.milliseconds - loading.milliseconds })
          loading = undefined
        } else if (["Failed", "MainMenu"].includes(transition.phase)) loading = undefined
      }
      const menu = transitions.find((transition) => transition.phase === "MainMenu")
      await testInfo.attach("profile-wall-clock-phases", {
        body: JSON.stringify({
          totalMilliseconds: Date.now() - started,
          startupMilliseconds: menu?.milliseconds ?? null,
          mapLoads: maps,
          scenarioMilliseconds: maps.length ? Date.now() - started - maps.at(-1)!.readyMilliseconds : null,
          transitions,
        }),
        contentType: "application/json",
      })
    }
  }, { auto: true }],
})

export { expect }
