import { expect, test } from "./application-test"
import { chooseTf2Team } from "./team-selection-evidence"
import { decodeScreenshot } from "./screenshot-pixels"

test("headed Badlands control point world, capture HUD and moving local bot roster", async ({ page }, testInfo) => {
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = {} })
  const errors: string[] = []
  page.on("pageerror", error => errors.push(error.message))
  const main = page.locator("main")
  await page.goto("/")
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 60_000 })
  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  await entry.fill("map cp_badlands"); await entry.press("Enter")
  await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 60_000 })
  if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  await chooseTf2Team(page, "red")
  await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 30_000 })
  const hud = page.locator(".hud-layer [data-vgui-name='HudControlPointIcons']")
  await expect(hud).toBeVisible()
  await expect(hud.locator("[data-vgui-name='BaseImage']")).toHaveCount(5)
  if (await main.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
  await entry.fill("tf_bot_quota 15"); await entry.press("Enter")
  await page.keyboard.press("Backquote")
  await expect(main).toHaveAttribute("data-bot-count", "15")
  await testInfo.attach("badlands-initial-world-and-hud", { body: await page.screenshot(), contentType: "image/png" })
  const before = await page.evaluate(() => (globalThis as any).__playsrcProfile.bots.map((bot: any) => ({ identity: bot.identity, position: bot.position })))
  await page.waitForTimeout(5000)
  const after = await page.evaluate(() => (globalThis as any).__playsrcProfile.bots.map((bot: any) => ({ identity: bot.identity, position: bot.position, objective: bot.objective })))
  await testInfo.attach("badlands-motion-facts", { body: JSON.stringify({ before, after, errors }), contentType: "application/json" })
  expect(after.length).toBeGreaterThan(0)
  expect(after.some((bot: any) => before.some((prior: any) => bot.identity === prior.identity && Math.hypot(...bot.position.map((value: number, i: number) => value - prior.position[i])) > 32))).toBe(true)
  const screenshot = await page.screenshot()
  const image = decodeScreenshot(screenshot)
  let visible = 0
  for (let i = 0; i < image.pixels.length; i += image.channels) if (image.pixels[i]! + image.pixels[i + 1]! + image.pixels[i + 2]! > 72) visible++
  expect(visible).toBeGreaterThan(20_000)
  await testInfo.attach("badlands-world-and-five-point-hud", { body: screenshot, contentType: "image/png" })
  await testInfo.attach("badlands-bot-motion", { body: JSON.stringify({ before, after, errors }), contentType: "application/json" })
  expect(errors).toEqual([])
})
