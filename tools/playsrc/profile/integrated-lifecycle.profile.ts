import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { TF2_BROWSER_SETTINGS_STORAGE_KEY } from "@playsrc/game-tf2-browser/settings-integration"
import { loadLocalConfig } from "../src/config"
import { expect, test } from "./application-test"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"
import { activeGameplayTraceWindow, summarizeCompositorTruth, type ChromiumTraceEvent } from "./compositor-truth"
import { chooseTf2Team } from "./team-selection-evidence"

test("integrated persisted quality, two live map replacements and overhead water", async ({ page, context }, testInfo) => {
  const directory = path.join((await loadLocalConfig()).sourceCacheDir, "profiles", "integrated-acceptance", `lifecycle-${Date.now()}`)
  await mkdir(directory, { recursive: true })
  await page.addInitScript(installBrowserFrameProfiler)
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = {} })
  const errors: string[] = []
  page.on("pageerror", error => errors.push(error.message))
  page.on("console", async message => {
    if (message.type() !== "error") return
    const detail = await Promise.all(message.args().map(argument => argument.jsonValue().catch(() => null)))
    await writeFile(path.join(directory, `console-error-${Date.now()}.json`), JSON.stringify(detail))
    console.log(`ACCEPTANCE_ERROR ${JSON.stringify(detail)}`)
  })
  const main = page.locator("main"), canvas = page.locator("canvas.world-canvas")
  const command = async (value: string) => {
    if (await main.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    const entry = page.locator("[aria-label='Console command']")
    await entry.fill(value)
    await entry.press("Enter")
  }
  await page.goto("/")
  await page.bringToFront()
  await expect(main).toHaveAttribute("data-phase", "MainMenu")
  // Raise HDR/filtering through the real console and preserve them across reload
  // and replacement. Never lower quality to manufacture a faster sample.
  await command("mat_hdr_level 2")
  await command("mat_forceaniso 4")
  await expect(main).toHaveAttribute("data-settings-persistence", "stored")
  await expect(main).toHaveAttribute("data-phase", "MainMenu")
  const persisted = await page.evaluate(key => localStorage.getItem(key), TF2_BROWSER_SETTINGS_STORAGE_KEY)
  expect(persisted).not.toBeNull()
  await page.reload()
  await expect(main).toHaveAttribute("data-phase", "MainMenu")
  await expect(main).toHaveAttribute("data-settings-persistence", "loaded")
  expect(await page.evaluate(key => localStorage.getItem(key), TF2_BROWSER_SETTINGS_STORAGE_KEY)).toBe(persisted)
  const maps: any[] = []
  let quality: any
  for (const [index, target] of ["pl_upward", "ctf_2fort", "jump_beef"].entries()) {
    const started = Date.now()
    await command(`map ${target}`)
    await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 60_000 })
    if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await chooseTf2Team(page, index === 1 ? "blue" : "red")
    await expect(main).toHaveAttribute("data-phase", "Ready")
    const current = await page.evaluate(() => ({ quality: (globalThis as any).__playsrcProfile.videoQuality, hud: document.querySelector<HTMLElement>("main")!.dataset.hudPresentationProbe, load: document.querySelector<HTMLElement>("main")!.dataset.loadPerformance }))
    const { waterPasses: _passes, ...stableQuality } = current.quality
    if (quality) expect(stableQuality).toEqual(quality)
    else quality = stableQuality
    expect(stableQuality.hdrLevel).toBe(2)
    expect(stableQuality.lightingProfile).toBe("hdr")
    expect(await page.evaluate(key => localStorage.getItem(key), TF2_BROWSER_SETTINGS_STORAGE_KEY)).toBe(persisted)
    await page.bringToFront()
    await canvas.click()
    try { await expect(main).toHaveAttribute("data-pointer-locked", "true", { timeout: 5000 }) }
    catch { throw new Error(`Native map input failed for ${target}: ${JSON.stringify(await main.evaluate(element => ({ detail: element.dataset.detail, focused: document.hasFocus(), visible: document.visibilityState, classSelection: element.dataset.classSelectionVisible, owner: document.pointerLockElement?.className })))}`) }
    const before = await main.getAttribute("data-camera-position")
    for (const key of ["w", "a", "s", "d"]) {
      await page.keyboard.down(key)
      await page.waitForTimeout(100)
      await page.keyboard.up(key)
    }
    await page.mouse.move(680, 380)
    await page.mouse.click(680, 380)
    expect(await main.getAttribute("data-camera-position")).not.toBe(before)
    maps.push({ target, milliseconds: Date.now() - started, ...current, workerMemory: await Promise.all(page.workers().filter(worker => worker.url().includes("gameplay-worker")).map(worker => worker.evaluate(() => (globalThis as any).__playsrcWorkerMemory))) })
    await writeFile(path.join(directory, "maps.json"), JSON.stringify(maps, null, 2))
    await page.screenshot({ path: path.join(directory, `${target}.png`) })
  }
  const water = await page.evaluate(() => (globalThis as any).__playsrcProfile.materialAnimation?.volumes?.[0])
  expect(water).toBeTruthy()
  // Use the game's real console for the authored-water viewpoint; input below
  // still travels through trusted mouse events and genuine pointer lock.
  await command("noclip")
  const x = (water.bounds[0][0] + water.bounds[1][0]) / 2
  const y = (water.bounds[0][1] + water.bounds[1][1]) / 2
  await command(`setpos ${x} ${y} ${water.surfaceZ + 192}`)
  await page.keyboard.press("Backquote")
  await canvas.click({ position: { x: 640, y: 360 } })
  await expect(main).toHaveAttribute("data-pointer-locked", "true")
  const pitch = Number(await main.getAttribute("data-camera-pitch"))
  // Default m_pitch .022 * sensitivity 3; positive mouse Y looks down.
  await page.mouse.move(640, 360 + (80 - pitch) / 0.066)
  await expect.poll(async () => Number(await main.getAttribute("data-camera-pitch"))).toBeGreaterThan(65)
  const cdp = await context.newCDPSession(page)
  const traceEvents: ChromiumTraceEvent[] = []
  cdp.on("Tracing.dataCollected", ({ value }) => traceEvents.push(...value))
  await cdp.send("Tracing.start", { categories: "benchmark,viz,gpu,devtools.timeline,blink.user_timing", options: "record-as-much-as-possible" })
  const initial = await page.evaluate(() => {
    (globalThis as any).__playsrcFrameProfiler.active = true
    performance.mark("playsrc-active-gameplay-start")
    return { tick: Number(document.querySelector<HTMLElement>("main")!.dataset.snapshotTick), started: performance.now() }
  })
  await page.waitForTimeout(5000)
  const measurement = await page.evaluate(() => {
    performance.mark("playsrc-active-gameplay-end")
    const profiler = (globalThis as any).__playsrcFrameProfiler
    profiler.active = false
    const main = document.querySelector<HTMLElement>("main")!
    return { tick: Number(main.dataset.snapshotTick), ended: performance.now(), visible: document.visibilityState, focused: document.hasFocus(), plan: main.dataset.waterPlan, passes: main.dataset.waterPasses, restored: main.dataset.waterRestored, gpu: profiler.counters, losses: profiler.losses }
  })
  const finished = new Promise<void>(resolve => cdp.once("Tracing.tracingComplete", () => resolve()))
  await cdp.send("Tracing.end")
  await finished
  await writeFile(path.join(directory, "water.trace.json"), JSON.stringify({ traceEvents }))
  const compositor = summarizeCompositorTruth(traceEvents, measurement.ended - initial.started, activeGameplayTraceWindow(traceEvents))
  await page.screenshot({ path: path.join(directory, "overhead-water.png") })
  const report = { maps, quality, measurement, compositor, errors, secondsPerFrameIncidentResolved: false }
  await writeFile(path.join(directory, "report.json"), JSON.stringify(report, null, 2))
  await testInfo.attach("integrated-lifecycle", { body: JSON.stringify(report), contentType: "application/json" })
  expect(measurement.visible).toBe("visible")
  expect(measurement.focused).toBe(true)
  expect(measurement.tick - initial.tick).toBeGreaterThan(300)
  expect(measurement.plan).toMatch(/^above:/)
  expect(measurement.passes).toContain("main")
  expect(measurement.restored).toBe("true")
  expect(measurement.losses).toEqual([])
  expect(errors).toEqual([])
  expect(compositor.evidence).toBe("chromium-compositor-presentation-trace")
  expect(compositor.maximumUnpresentedMilliseconds).toBeLessThan(250)
})
