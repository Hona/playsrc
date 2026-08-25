import { test as base, expect } from "@playwright/test"

type ApplicationState = Readonly<{
  phase: string
  detail: string
  gameUi: string
  startupState: string
  consoleOutput: string
  blockers: string
}>

export const test = base.extend<{
  applicationDiagnostics: void
  allowRecoverableApplicationFailure: boolean
  preserveStartupMovie: boolean
}>({
  allowRecoverableApplicationFailure: [false, { option: true }],
  preserveStartupMovie: [false, { option: true }],
  applicationDiagnostics: [async ({ page, allowRecoverableApplicationFailure, preserveStartupMovie }, use, testInfo) => {
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
        const stallSeconds = process.env.PROFILE_PYRO_STOCK === "1" || process.env.PROFILE_COMBAT === "1" ? 180 : 65
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
    }
  }, { auto: true }],
})

export { expect }
