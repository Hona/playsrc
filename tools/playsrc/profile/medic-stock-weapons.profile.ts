import { expect, test } from "./application-test"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"

test("authored Medic stock weapons preserve visible models, ammo, bot targets, and simulation cadence", async ({ page }, testInfo) => {
  await page.addInitScript(() => {
    ;(globalThis as typeof globalThis & { __playsrcProfile?: object }).__playsrcProfile = {}
    let locked: Element | null = null
    Object.defineProperty(document, "pointerLockElement", { configurable: true, get: () => locked })
    Object.defineProperty(Element.prototype, "requestPointerLock", { configurable: true, value(this: Element) {
      locked = this
      queueMicrotask(() => document.dispatchEvent(new Event("pointerlockchange")))
      return Promise.resolve()
    } })
    Object.defineProperty(document, "exitPointerLock", { configurable: true, value() {
      locked = null
      queueMicrotask(() => document.dispatchEvent(new Event("pointerlockchange")))
      return Promise.resolve()
    } })
  })
  await page.goto("/")
  await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu", { timeout: 120_000 })
  await page.keyboard.press("Backquote")
  const command = page.locator("[aria-label='Console command']")
  await command.fill("map pl_upward")
  await command.press("Enter")
  await expect(page.locator("main")).toHaveAttribute("data-phase", "Ready", { timeout: 600_000 })
  await command.fill("joinclass medic")
  await command.press("Enter")
  await expect.poll(async () => page.locator("main").getAttribute("data-weapon-trace"))
    .toContain("10:40/150")
  await command.fill("tf_bot_add red heavy")
  await command.press("Enter")
  await expect(page.locator("main")).toHaveAttribute("data-bot-count", "1")
  await command.fill("tf_bot_add blue soldier")
  await command.press("Enter")
  await expect(page.locator("main")).toHaveAttribute("data-bot-count", "2")
  await page.keyboard.press("Backquote")
  await expect(page.locator("main")).toHaveAttribute("data-console-visible", "false")

  await page.keyboard.press("Digit2")
  await expect.poll(async () => page.locator("main").getAttribute("data-weapon-trace"))
    .toContain("11:0/0")
  await expect.poll(async () => page.locator("main").getAttribute("data-viewmodel-activity"))
    .toContain("SECONDARY")
  await expect(page.locator("[data-vgui-name='HudMedicCharge']")).toBeVisible()
  const medigun = await page.locator(".world-canvas").screenshot()
  await testInfo.attach("headed-authored-medigun", { body: medigun, contentType: "image/png" })

  await page.keyboard.press("Digit3")
  await expect.poll(async () => page.locator("main").getAttribute("data-viewmodel-activity"))
    .toContain("MELEE")
  await page.evaluate(async () => {
    const canvas = document.querySelector(".world-canvas")
    if (!canvas) throw new Error("Medic weapon evidence canvas is unavailable")
    if (document.pointerLockElement !== canvas) await canvas.requestPointerLock()
    dispatchEvent(new MouseEvent("mousedown", { button: 0, bubbles: true }))
  })
  await expect.poll(async () => page.locator("main").getAttribute("data-audio-starts"))
    .toContain("Weapon_BoneSaw.Miss")
  await page.evaluate(() => dispatchEvent(new MouseEvent("mouseup", { button: 0, bubbles: true })))

  await page.keyboard.press("Digit1")
  await expect.poll(async () => page.locator("main").getAttribute("data-viewmodel-activity"))
    .toContain("PRIMARY")
  await page.evaluate(() => dispatchEvent(new MouseEvent("mousedown", { button: 0, bubbles: true })))
  await expect.poll(async () => page.locator("main").getAttribute("data-fire-events"))
    .not.toBe("0")
  await page.evaluate(() => dispatchEvent(new MouseEvent("mouseup", { button: 0, bubbles: true })))
  await expect.poll(async () => page.locator("main").getAttribute("data-audio-starts"))
    .toContain("Weapon_SyringeGun.Single")
  const syringe = await page.locator(".world-canvas").screenshot()
  await testInfo.attach("headed-authored-syringe", { body: syringe, contentType: "image/png" })

  const seconds = profileSampleSeconds()
  const measurement = await page.evaluate(async (duration) => {
    const root = document.querySelector<HTMLElement>("main")!
    const first = Number(root.dataset.snapshotTick)
    const started = performance.now()
    const frames: number[] = []
    let prior = started
    await new Promise<void>((resolve) => {
      const frame = (now: number) => {
        frames.push(now - prior)
        prior = now
        if (now - started >= duration * 1_000) resolve()
        else requestAnimationFrame(frame)
      }
      requestAnimationFrame(frame)
    })
    return { seconds: (performance.now() - started) / 1_000, ticks: Number(root.dataset.snapshotTick) - first, frames }
  }, seconds)
  expect(measurement.ticks).toBeGreaterThan(seconds * 55)
  console.log(`[medic-stock-weapons] ${JSON.stringify({ headed: true, target: "pl_upward", bots: 2, seconds: measurement.seconds, ticksPerSecond: measurement.ticks / measurement.seconds, frames: summarizeFrameTimes(measurement.frames) })}`)
})
