import { test, expect } from "./application-test"
import { chooseTf2Team } from "./team-selection-evidence"
import { writeFile } from "node:fs/promises"

test("configured Granary soundscape selections reach the browser without processing claims", async ({ page }, testInfo) => {
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = {} })
  const main = page.locator("main")
  const errors: string[] = []
  page.on("pageerror", error => errors.push(error.message))
  const command = async (value: string) => {
    if (await main.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    const entry = page.locator("[aria-label='Console command']")
    await entry.fill(value); await entry.press("Enter")
    await page.keyboard.press("Backquote")
  }
  const selection = () => page.evaluate(() => (globalThis as any).__playsrcProfile.soundscape)
  await page.goto("/")
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 60_000 })
  await command("map cp_granary")
  await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 60_000 })
  await chooseTf2Team(page, "red")
  await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 30_000 })
  // Authored soundscape origins from configured cp_granary BSP
  // 6080d8b69cd526aa3e76252d61138ccd4d371953ea0ef2787e5a56bd3399f08d.
  await command("setpos -997 -5496 -466")
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.soundscape?.entity === 49)
  const outside = await selection()
  await page.screenshot({ path: testInfo.outputPath("outside-selection.png") })
  await command("setpos -1476 -6447.29 -308")
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.soundscape?.entity === 47)
  const inside = await selection()
  expect(inside.soundscape).not.toBe(outside.soundscape)
  expect(inside.positions).toHaveLength(8)
  await page.screenshot({ path: testInfo.outputPath("inside-selection.png") })
  const sample = await page.evaluate(async () => {
    const root = document.querySelector<HTMLElement>("main")!, start = performance.now(), tick = Number(root.dataset.snapshotTick)
    const frames: number[] = []; let prior = start
    await new Promise<void>(resolve => {
      const frame = (now: number) => { frames.push(now - prior); prior = now; if (now - start >= 5000) resolve(); else requestAnimationFrame(frame) }
      requestAnimationFrame(frame)
    })
    return { seconds: (performance.now() - start) / 1000, ticks: Number(root.dataset.snapshotTick) - tick, frames }
  })
  expect(sample.ticks / sample.seconds).toBeGreaterThan(63)
  const filename = testInfo.outputPath("soundscape-selection.json")
  await writeFile(filename, JSON.stringify({ outside, inside, sample, errors, audioProcessingVerified: false }))
  await testInfo.attach("soundscape-selection", { path: filename, contentType: "application/json" })
  expect(errors).toEqual([])
})
