import { expect, test } from "./application-test"
import { decodeScreenshot } from "./screenshot-pixels"
import { settleTf2Gameplay } from "./team-selection-evidence"

test("headed stale application recovery preserves immutable objects and publishes one visible Ready generation", async ({ page }, testInfo) => {
  const started = Date.now()
  let configurations = 0
  let prematureObjects = 0
  let immutableRequests = 0
  const failures: string[] = []
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = {} })
  page.on("console", (message) => {
    if (/GPUValidationError|context lost|Destroyed texture|PMTX identity/u.test(message.text())) failures.push(message.text())
  })
  page.on("request", (request) => {
    if (!/\/objects\/sha256\/[0-9a-f]{64}$/u.test(request.url())) return
    immutableRequests += 1
    if (configurations < 2) prematureObjects += 1
  })
  await page.route("**/playsrc-config.json", async (route) => {
    const response = await route.fetch()
    const configuration = await response.json()
    configurations += 1
    if (configurations === 1) {
      configuration.applicationBuild = configuration.applicationBuild === "a".repeat(64) ? "b".repeat(64) : "a".repeat(64)
    }
    await route.fulfill({ response, json: configuration, headers: { ...response.headers(), "cache-control": "no-store" } })
  })

  await page.goto("/", { waitUntil: "domcontentloaded", timeout: 30_000 }).catch((error) => {
    if (!String(error).includes("interrupted by another navigation")) throw error
  })
  await page.bringToFront()
  const root = page.locator("main")
  await expect(root).toHaveAttribute("data-phase", "MainMenu", { timeout: 100_000 })
  expect(configurations).toBe(2)
  expect(prematureObjects).toBe(0)

  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  await entry.fill("map jump_beef")
  await page.keyboard.press("Enter")
  await settleTf2Gameplay(page)
  await expect(root).toHaveAttribute("data-phase", "Ready")
  const frames = await page.evaluate(() => new Promise<number>((resolve) => {
    const started = performance.now()
    let count = 0
    const frame = () => {
      count += 1
      if (performance.now() - started >= 5_000) resolve(count)
      else requestAnimationFrame(frame)
    }
    requestAnimationFrame(frame)
  }))
  const profile = await page.evaluate(() => ({
    generation: (globalThis as any).__playsrcProfile.applicationGeneration,
    immutableCache: (globalThis as any).__playsrcProfile.immutableCache,
    phase: document.querySelector("main")?.dataset.phase,
    recovery: sessionStorage.getItem("playsrc.tf2.application-generation.v1"),
  }))
  expect(profile.generation.bundle).toBe(profile.generation.configuration)
  expect(profile.generation.worker).toBe(profile.generation.bundle)
  expect(profile.generation.staleMessages).toBe(0)
  expect(profile.generation.mapGeneration).toBe(1)
  expect(profile.generation.readyMilliseconds).toBeGreaterThan(0)
  expect(profile.generation.presentationSchema).toBe(16)
  expect(profile.phase).toBe("Ready")
  expect(JSON.parse(profile.recovery!).length).toBe(1)
  expect(frames).toBeGreaterThan(20)
  expect(failures).toEqual([])
  const screenshot = await page.screenshot()
  const pixels = decodeScreenshot(screenshot)
  let visible = 0
  for (let offset = 0; offset < pixels.pixels.length; offset += pixels.channels) {
    if (pixels.pixels[offset]! > 8 || pixels.pixels[offset + 1]! > 8 || pixels.pixels[offset + 2]! > 8) visible += 1
  }
  expect(visible).toBeGreaterThan(20_000)
  await testInfo.attach("recovered-visible-ready-generation", { body: screenshot, contentType: "image/png" })
  console.log(`TF2_APPLICATION_UPGRADE ${JSON.stringify({ configurations, prematureObjects, immutableRequests, frames, compositorFps: frames / 5, visiblePixels: visible, failures, wallMilliseconds: Date.now() - started, ...profile })}`)
})
