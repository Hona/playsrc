import { expect, test } from "@playwright/test"

test("plays the configured startup movie and loads jump_beef", async ({ page }, testInfo) => {
  await page.goto("/", { waitUntil: "load", timeout: 30_000 })
  const main = page.locator("main")
  const movie = page.locator(".startup-movie")
  await page.waitForFunction(() => {
    const state = document.querySelector<HTMLElement>("main")?.dataset.startupState
    return state === "AwaitingGesture" || state === "Playing" || state === "Failed"
  }, undefined, { timeout: 300_000, polling: 50 })
  expect(await main.getAttribute("data-startup-state")).not.toBe("Failed")
  if (await main.getAttribute("data-startup-state") === "AwaitingGesture") await movie.click()
  await expect(main).toHaveAttribute("data-startup-state", "Playing")
  await expect.poll(() => movie.evaluate((video: HTMLVideoElement) => ({
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
  await expect(consoleEntry).toBeVisible()
  await consoleEntry.fill("map jump_beef")
  await page.keyboard.press("Enter")
  await page.waitForFunction(() => {
    const element = document.querySelector<HTMLElement>("main")
    return (element?.dataset.phase === "Ready" && element.dataset.gameui === "in-game") || element?.dataset.phase === "Failed"
  }, undefined, { timeout: 600_000, polling: 50 })
  expect(await main.getAttribute("data-phase")).toBe("Ready")
  expect(await main.getAttribute("data-detail")).toBe("Click the field to capture the mouse")
})
