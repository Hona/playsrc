import { build } from "vite"
import path from "node:path"
import { mkdir, writeFile } from "node:fs/promises"
import type { Page } from "@playwright/test"
import { expect, test } from "./native-edge-test"
import { settleTf2Gameplay } from "./team-selection-evidence"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"
import { ARCHIVED_GENERATIONS, archivedGeneration } from "./release-generations"
import { generationFixtureServer, type GenerationFixture } from "./generation-fixture-server"
import { changingGameplayEvidence, immutableInventory } from "./generation-presentation-evidence"
import { loadLocalConfig, repositoryRoot } from "../src/config"
import type { BrowserConfiguration } from "../../../apps/web/tf2/src/config"
import { summarizeDistribution } from "./gameui-profile"

test.use({ allowRecoverableApplicationFailure: true })
const developmentOrigin = process.env.PLAYSRC_PROFILE_ORIGIN ?? `http://127.0.0.1:${process.env.PLAYSRC_DEV_PORT ?? "4173"}`

test("native same-window tabs expose real visibility and independent session storage", async ({ page }) => {
  await page.goto(`${developmentOrigin}/`, { waitUntil: "domcontentloaded" })
  const opened = page.waitForEvent("popup")
  await page.evaluate(() => { sessionStorage.setItem("generation-tab-test", "original"); window.open("about:blank", "_blank") })
  const tab = await opened
  try {
    const first = await page.context().newCDPSession(page)
    const second = await page.context().newCDPSession(tab)
    expect((await first.send("Browser.getWindowForTarget")).windowId).toBe((await second.send("Browser.getWindowForTarget")).windowId)
    await tab.bringToFront()
    await tab.evaluate(() => sessionStorage.setItem("generation-tab-test", "other"))
    await expect.poll(() => page.evaluate(() => document.visibilityState), { timeout: 3_000 }).toBe("hidden")
    expect(await page.evaluate(() => sessionStorage.getItem("generation-tab-test"))).toBe("original")
    await page.bringToFront()
    await expect.poll(() => tab.evaluate(() => document.visibilityState), { timeout: 3_000 }).toBe("hidden")
    expect(await tab.evaluate(() => sessionStorage.getItem("generation-tab-test"))).toBe("other")
  } finally { await tab.close() }
})

test("exact archived generation transitions, isolated tabs, warm CAS and actual compositor recovery", async ({ page, context }, testInfo) => {
  const wall = Date.now()
  const local = await loadLocalConfig()
  const configured = await (await page.request.get(`${developmentOrigin}/playsrc-config.json`)).json() as BrowserConfiguration
  const target = configured.defaultTarget
  const roster = Number(process.env.PROFILE_UPGRADE_ROSTER ?? 0)
  const switchOnly = process.env.PROFILE_UPGRADE_SWITCH_ONLY === "1"
  const archived = await Promise.all(ARCHIVED_GENERATIONS.map((fixture) => archivedGeneration(fixture)))
  const currentConfiguration = { ...configured, renderLevel: 2 as const,
    ...(!roster ? { targets: archived[1]!.configuration.targets, catalog: archived[1]!.configuration.catalog } : {}),
  }
  for (const target of configured.targets) {
    expect(currentConfiguration.targets.find((candidate) => candidate.target === target.target)?.objects.resources).toEqual(target.objects.resources)
  }
  const prior = { build: process.env.PLAYSRC_APPLICATION_BUILD, config: process.env.PLAYSRC_BROWSER_CONFIG }
  const app = path.join(repositoryRoot, "apps", "web", "tf2")
  try {
    process.env.PLAYSRC_APPLICATION_BUILD = configured.applicationBuild
    process.env.PLAYSRC_BROWSER_CONFIG = JSON.stringify(currentConfiguration)
    await build({ root: app, configFile: path.join(app, "vite.config.ts"), logLevel: "error" })
  } finally {
    for (const [key, value] of [["PLAYSRC_APPLICATION_BUILD", prior.build], ["PLAYSRC_BROWSER_CONFIG", prior.config]]) {
      if (value === undefined) delete process.env[key!]
      else process.env[key!] = value
    }
  }
  const fixtures: GenerationFixture[] = archived.map((fixture) => ({ name: fixture.tag, output: fixture.output, configuration: { ...fixture.configuration, defaultTarget: target } }))
  const current: GenerationFixture = { name: "current", output: path.join(app, "dist", "cloudflare", "tf2"), configuration: currentConfiguration }
  fixtures.push(current)
  const server = await generationFixtureServer(fixtures, local.assetDir)
  const errors: Array<{ generation: string; message: string }> = []
  const transitions: any[] = []
  const recoveries: number[] = []
  const baseline: any[] = []
  const services: any[] = []
  const switches: any[] = []
  let browserCacheHits = 0
  const network = await context.newCDPSession(page)
  await network.send("Network.enable")
  network.on("Network.requestServedFromCache", () => { browserCacheHits += 1 })
  const install = async (tab: Page) => {
    await tab.addInitScript(installBrowserFrameProfiler)
    await tab.addInitScript(() => { (globalThis as any).__playsrcProfile = {} })
    tab.on("pageerror", (error) => errors.push({ generation: server.state.html.name, message: error.message }))
    tab.on("console", (message) => {
      if (/GPUValidationError|context lost|Destroyed texture|PMTX identity/u.test(message.text())) errors.push({ generation: server.state.html.name, message: message.text() })
    })
  }
  const menu = async (tab: Page) => {
    await tab.waitForFunction(() => {
      const main = document.querySelector<HTMLElement>("main")
      return ["Failed", "MainMenu"].includes(main?.dataset.phase ?? "") || ["Playing", "AwaitingGesture"].includes(main?.dataset.startupState ?? "")
    }, undefined, { timeout: 30_000 })
    if (await tab.locator("main").getAttribute("data-phase") === "Startup") await tab.keyboard.press("Escape")
    await expect(tab.locator("main")).toHaveAttribute("data-phase", "MainMenu", { timeout: 30_000 })
  }
  const navigate = async (tab: Page) => {
    await tab.bringToFront()
    await tab.goto(`${server.origin}/tf2/`, { waitUntil: "domcontentloaded" }).catch((error) => {
      if (!String(error).includes("interrupted by another navigation")) throw error
    })
    await menu(tab)
  }
  const command = async (tab: Page, text: string) => {
    if (await tab.locator("main").getAttribute("data-console-visible") !== "true") await tab.keyboard.press("Backquote")
    const entry = tab.locator("[aria-label='Console command']")
    await entry.fill(text)
    await entry.press("Enter")
  }
  const ready = async (tab: Page) => {
    await command(tab, `map ${target}`)
    await settleTf2Gameplay(tab)
    await expect(tab.locator("main")).toHaveAttribute("data-phase", "Ready", { timeout: 30_000 })
    if (roster) {
      await command(tab, `tf_bot_quota ${roster}`)
      await expect(tab.locator("main")).toHaveAttribute("data-bot-count", String(roster), { timeout: 15_000 })
      await tab.keyboard.press("Backquote")
    }
  }
  const serviceState = async (tab: Page) => tab.evaluate(async () => ({
    controller: navigator.serviceWorker.controller?.scriptURL ?? null,
    registrations: (await navigator.serviceWorker.getRegistrations()).map((registration) => ({ scope: registration.scope, active: registration.active?.scriptURL ?? null, waiting: registration.waiting?.scriptURL ?? null })),
    cacheStorage: await caches.keys(),
  }))
  let oldTab: Page | undefined
  try {
    await install(page)
    // Actual deployed page, read only. No Service Worker is invented by this test.
    const production = await context.newPage()
    await production.bringToFront()
    await production.goto("https://playsrc.online/tf2", { waitUntil: "domcontentloaded", timeout: 20_000 })
    services.push({ origin: "production", ...await serviceState(production) })
    await production.close()
    for (const fixture of switchOnly ? [current] : fixtures) {
      console.log(`GENERATION_STAGE exact ${fixture.name} ${target}`)
      server.select(fixture)
      server.state.workerDelayMilliseconds = fixture === current ? 250 : 0
      const started = Date.now()
      await navigate(page)
      await ready(page)
      transitions.push({ generation: fixture.name, applicationBuild: fixture.configuration.applicationBuild, wasm: fixture.configuration.wasm.sha256, resourceRoot: fixture.configuration.targets.find((candidate) => candidate.target === target)!.objects.resources.sha256, readyMilliseconds: Date.now() - started, bots: Number(await page.locator("main").getAttribute("data-bot-count")) })
    }
    services.push({ origin: "fixture", ...await serviceState(page) })
    const retained = await immutableInventory(page)
    expect(retained.length).toBeGreaterThan(100)
    // Exercise three real stale-configuration responses. Ready must reset only the
    // completed episode, not forgive a repeatedly mismatching startup.
    const staleConfigurations = switchOnly ? [] : [fixtures[0]!, fixtures[1]!, fixtures[0]!]
    if (!roster && !switchOnly) staleConfigurations.push({ ...current, name: "same-build-stale-wasm", configuration: { ...current.configuration, wasm: fixtures[0]!.configuration.wasm } })
    for (const stale of staleConfigurations) {
      console.log(`GENERATION_STAGE recover ${stale.name}`)
      server.state.configurations.push(stale)
      const startRequests = server.state.requests.length
      const started = Date.now()
      await navigate(page)
      await ready(page)
      recoveries.push(Date.now() - started)
      expect(server.state.requests.slice(startRequests).filter((request) => request.pathname.startsWith("/objects/"))).toEqual([])
      expect(await immutableInventory(page)).toEqual(retained)
    }
    // An independent old tab keeps its authentic module/WASM generation while a
    // newer tab opens and reloads against the same origin and IDB database.
    if (!switchOnly) {
      server.select(fixtures[1]!)
      console.log("GENERATION_STAGE retained-tab")
      const opened = page.waitForEvent("popup")
      await page.evaluate(() => { window.open("about:blank", "_blank") })
      oldTab = await opened
      await install(oldTab)
      await navigate(oldTab)
      server.select(current)
      await navigate(page)
      await ready(page)
      expect(await immutableInventory(page)).toEqual(retained)
      await oldTab.bringToFront()
      expect(await oldTab.locator("main").getAttribute("data-phase")).toBe("MainMenu")
      services.push({ origin: "retained-old-tab", ...await serviceState(oldTab) })
      if (!roster) {
        server.state.configurations.push(fixtures[0]!)
        let deliver!: () => void
        server.state.configurationGate = new Promise<void>((resolve) => { deliver = resolve })
        const requests = server.state.requests.length
        await page.goto(`${server.origin}/tf2/`, { waitUntil: "domcontentloaded" })
        await oldTab.bringToFront()
        await expect.poll(() => page.evaluate(() => document.visibilityState)).toBe("hidden")
        deliver()
        server.state.configurationGate = undefined
        await oldTab.waitForTimeout(250)
        expect(server.state.requests.slice(requests).filter((request) => request.pathname.startsWith("/objects/"))).toEqual([])
        expect(server.state.requests.slice(requests).filter((request) => request.pathname.endsWith("playsrc-config.json")).length).toBeLessThanOrEqual(1)
        await page.bringToFront()
        await menu(page)
        await ready(page)
        expect(await immutableInventory(page)).toEqual(retained)
      }
      await oldTab.close()
      oldTab = undefined
    }
    await page.bringToFront()
    await expect(page.locator("main")).toHaveAttribute("data-phase", "Ready")
    if (!roster) {
      for (const map of ["ctf_2fort", target]) {
        console.log(`GENERATION_STAGE map-switch ${map}`)
        const previous = Number(await page.locator("main").getAttribute("data-generation"))
        const started = Date.now()
        await command(page, `map ${map}`)
        await expect(page.locator("main")).toHaveAttribute("data-generation", String(previous + 1), { timeout: 30_000 })
        if (await page.locator("main").getAttribute("data-phase") === "Failed") throw new Error((await page.locator("main").getAttribute("data-detail")) ?? "Map replacement failed")
        await settleTf2Gameplay(page)
        switches.push({ map, generation: previous + 1, readyMilliseconds: Date.now() - started })
      }
      await command(page, "joinclass heavyweapons")
      await expect.poll(async () => (await page.locator("main").getAttribute("data-hud-probe"))?.split(":")[1], { timeout: 5_000 }).toBe("6")
    }
    await command(page, "joinclass soldier")
    await expect.poll(async () => (await page.locator("main").getAttribute("data-hud-probe"))?.split(":")[1]).toBe("3")
    await page.keyboard.press("Backquote")
    if (switchOnly) { console.log(`TF2_GENERATION_SWITCH ${JSON.stringify(switches)}`); return }
    const visual = await changingGameplayEvidence(page)
    expect(visual.phase).toBe("Ready")
    expect(visual.bots).toBe(roster)
    expect(visual.roster).toHaveLength(roster)
    expect(new Set(visual.roster.map((bot: any) => bot.identity)).size).toBe(roster)
    expect(visual.roster.every((bot: any) => Number.isInteger(bot.class) && bot.class >= 1 && bot.class <= 9
      && [2, 3].includes(bot.team) && [1, 2].includes(bot.lifecycle) && bot.position.every(Number.isFinite))).toBe(true)
    expect(visual.losses).toEqual([])
    expect(visual.validationErrors).toBe(0)
    expect(visual.ticks / visual.elapsedMilliseconds * 1_000).toBeGreaterThan(60)
    await testInfo.attach("changing-recovered-gameplay", { body: visual.afterPixels, contentType: "image/png" })
    const finalErrors = errors.slice()
    expect(finalErrors).toEqual([])
    // Before the handshake, the exact archived browser accepts a current config
    // and only discovers the mixed format during decoding. Observe actual errors.
    for (let trial = 0; trial < 3; trial++) {
      console.log(`GENERATION_STAGE before ${trial}`)
      server.state.html = fixtures[1]!
      server.state.configuration = current
      const started = Date.now()
      await page.goto(`${server.origin}/tf2/`, { waitUntil: "domcontentloaded" })
      await page.waitForFunction(() => ["Failed", "MainMenu"].includes(document.querySelector<HTMLElement>("main")?.dataset.phase ?? "")
        || ["Playing", "AwaitingGesture"].includes(document.querySelector<HTMLElement>("main")?.dataset.startupState ?? ""), undefined, { timeout: 30_000 })
      if (await page.locator("main").getAttribute("data-phase") === "Startup") await page.keyboard.press("Escape")
      if (await page.locator("main").getAttribute("data-phase") !== "Failed") {
        await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu")
        await command(page, `map ${target}`)
      }
      await expect(page.locator("main")).toHaveAttribute("data-phase", "Failed", { timeout: 30_000 })
      baseline.push({ milliseconds: Date.now() - started, detail: await page.locator("main").getAttribute("data-detail") })
    }
    const { afterPixels, ...presentation } = visual
    expect(browserCacheHits).toBeGreaterThan(0)
    expect(server.state.activationDelays.some((delay) => delay >= 250)).toBe(true)
    const report = { target, roster, browser: await context.browser()!.version(), transitions, switches, services, browserCacheHits, workerActivationDelaysMilliseconds: server.state.activationDelays, warmCas: { records: retained.length, bytes: retained.reduce((total, record) => total + record.byteLength, 0), unchanged: true }, before: { failures: baseline.length, samples: baseline, failureLatency: summarizeDistribution(baseline.map((sample) => sample.milliseconds)) }, after: { failures: finalErrors.length, readySamplesMilliseconds: recoveries, readyLatency: summarizeDistribution(recoveries) }, presentation, wallMilliseconds: Date.now() - wall }
    const directory = path.join(local.sourceCacheDir, "profiles", "application-upgrade")
    await mkdir(directory, { recursive: true })
    await writeFile(path.join(directory, `${target}-evidence.json`), JSON.stringify(report, null, 2))
    await writeFile(path.join(directory, `${target}-ready.png`), afterPixels)
    console.log(`TF2_APPLICATION_UPGRADE ${JSON.stringify(report)}`)
  } catch (error) {
    const terminal = await page.evaluate(() => ({ detail: document.querySelector<HTMLElement>("main")?.dataset.detail, phase: document.querySelector<HTMLElement>("main")?.dataset.phase, output: document.querySelector("[aria-label='Console output']")?.textContent })).catch(() => null)
    console.log(`GENERATION_FAILURE ${JSON.stringify({ terminal, error: String(error) })}`)
    throw error
  } finally {
    await oldTab?.close()
    await page.goto("about:blank").catch(() => {})
    await network.detach().catch(() => {})
    await server.close()
  }
})
