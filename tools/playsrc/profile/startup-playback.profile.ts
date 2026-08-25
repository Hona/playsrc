import { expect, test } from "./application-test"

test.use({ preserveStartupMovie: true })

test("plays the configured startup movie and loads jump_beef", async ({ page }, testInfo) => {
  await page.route("**/objects/sha256/**", async (route) => {
    await new Promise((resolve) => setTimeout(resolve, 150))
    await route.continue()
  })
  await page.goto("/", { waitUntil: "load", timeout: 30_000 })
  const main = page.locator("main")
  const movie = page.locator(".startup-movie")
  const plaque = page.locator(".startup-loading-plaque")
  await expect(plaque).toBeVisible()
  await expect(plaque).toContainText(/^Loading \d+%\.\.\.$/)
  expect(await plaque.evaluate((element) => {
    const bounds = element.getBoundingClientRect()
    return [bounds.right, bounds.bottom, bounds.width, bounds.height]
  })).toEqual([1280, 720, 128, 64])
  await testInfo.attach("startup-loading-plaque", { body: await page.screenshot(), contentType: "image/png" })
  await page.setViewportSize({ width: 390, height: 844 })
  expect(await plaque.evaluate((element) => {
    const bounds = element.getBoundingClientRect()
    return [bounds.right, bounds.bottom, bounds.width, bounds.height]
  })).toEqual([390, 844, 128, 64])
  await page.setViewportSize({ width: 1280, height: 720 })
  await page.evaluate(() => {
    const main = document.querySelector<HTMLElement>("main")!
    const samples = [Number(main.dataset.bootstrapProgress)]
    ;(window as typeof window & { __playsrcBootstrapProgress: number[] }).__playsrcBootstrapProgress = samples
    new MutationObserver(() => samples.push(Number(main.dataset.bootstrapProgress))).observe(main, { attributes: true, attributeFilter: ["data-bootstrap-progress"] })
  })
  await page.waitForFunction(() => {
    const main = document.querySelector<HTMLElement>("main")
    const video = document.querySelector<HTMLVideoElement>(".startup-movie")
    return main?.dataset.startupState === "Failed" || (main?.dataset.startupState === "Playing" && video !== null && video.readyState >= HTMLMediaElement.HAVE_METADATA && !video.paused)
  }, undefined, { timeout: 300_000, polling: 50 })
  expect(await main.getAttribute("data-startup-state")).not.toBe("Failed")
  await expect(main).toHaveAttribute("data-startup-state", "Playing")
  await expect(plaque).toBeHidden()
  const progress = await page.evaluate(() => (window as typeof window & { __playsrcBootstrapProgress: number[] }).__playsrcBootstrapProgress)
  expect(progress.at(-1)).toBe(100)
  expect(progress.every((value, index) => index === 0 || value >= progress[index - 1]!)).toBe(true)
  expect(await movie.evaluate((video: HTMLVideoElement) => ({
    paused: video.paused,
    width: video.videoWidth,
    height: video.videoHeight,
    duration: video.duration,
  }))).toEqual({ paused: false, width: 1440, height: 1080, duration: 10.051 })
  await page.waitForFunction(() => document.querySelector<HTMLVideoElement>(".startup-movie")!.currentTime >= 4.9, undefined, { timeout: 30_000 })
  await testInfo.attach("startup-middle", { body: await page.screenshot(), contentType: "image/png" })
  await page.waitForFunction(() => document.querySelector<HTMLVideoElement>(".startup-movie")!.currentTime >= 9.75, undefined, { timeout: 30_000 })
  await testInfo.attach("startup-final", { body: await page.screenshot(), contentType: "image/png" })
  await expect(main).toHaveAttribute("data-startup-state", "Completed")
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 300_000 })

  await page.keyboard.press("Backquote")
  const consoleEntry = page.locator("[aria-label='Console command']")
  const consoleOutput = page.locator("[aria-label='Console output']")
  await expect(consoleEntry).toBeVisible()
  await expect(consoleOutput).toContainText("STATUS: Startup:")
  await consoleEntry.fill("map jump_beef")
  await page.keyboard.press("Enter")
  await page.waitForFunction(() => {
    const element = document.querySelector<HTMLElement>("main")
    return (element?.dataset.phase === "Ready" && element.dataset.gameui === "in-game") || element?.dataset.phase === "Failed"
  }, undefined, { timeout: 600_000, polling: 50 })
  expect(await main.getAttribute("data-phase")).toBe("Ready")
  expect(await main.getAttribute("data-detail")).toBe("Click the field to capture the mouse")
  await expect(consoleOutput).toContainText("STATUS: Loading:")
  await expect(consoleOutput).not.toContainText("ERROR:")
  await expect(consoleOutput).not.toContainText("FATAL:")
})
