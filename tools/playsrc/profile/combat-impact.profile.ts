import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, test } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"
import { chooseTf2Team } from "./team-selection-evidence"

test("headed 2Fort combat presents authored tracer, blood, local indicators, hit audio and depth-tested pixels", async ({ page }, testInfo) => {
  await page.addInitScript(() => { ;(globalThis as any).__playsrcProfile = {} })
  await page.goto("/")
  await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu", { timeout: 120_000 })
  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  const command = async (value: string) => { await entry.fill(value); await entry.press("Enter") }
  await command("map ctf_2fort")
  await page.waitForFunction(() => document.querySelector<HTMLElement>("main")?.dataset.teamSelectionVisible === "true", undefined, { timeout: 600_000 })
  await page.keyboard.press("Backquote")
  await chooseTf2Team(page, "red")
  await expect(page.locator("main")).toHaveAttribute("data-phase", "Ready", { timeout: 180_000 })
  if(await page.locator("main").getAttribute("data-class-selection-visible")==="true"){
    await page.keyboard.press("Digit1")
    await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible","false")
  }
  if(await page.locator("main").getAttribute("data-console-visible")!=="true")await page.keyboard.press("Backquote")
  await command("joinclass scout")
  await command("tf_dingalingaling 1")
  await command("tf_bot_add blue scout easy")
  await expect(page.locator("main")).toHaveAttribute("data-bot-count", "1")
  const bot = await page.evaluate(() => (globalThis as any).__playsrcProfile.bots[0])
  await command(`setpos ${bot.position[0] - 72} ${bot.position[1]} ${bot.position[2]}`)
  await page.keyboard.press("Backquote")
  await page.bringToFront()
  await page.locator("canvas.world-canvas").click({ position: { x: 640, y: 360 } })
  await expect(page.locator("main")).toHaveAttribute("data-pointer-locked", "true")
  const before = await page.locator("canvas.world-canvas").screenshot()
  await page.evaluate(() => {
    ;(globalThis as any).__combatImpactAim = setInterval(() => {
      const profile = (globalThis as any).__playsrcProfile
      const target = profile.bots?.find((candidate: any) => candidate.lifecycle === 1)
      const camera = profile.displacementCamera
      if (!target || !camera) return
      const delta = [target.position[0] - camera.position[0], target.position[1] - camera.position[1], target.position[2] + 53 - camera.position[2]]
      const yaw = Math.atan2(delta[1], delta[0]) * 180 / Math.PI
      const pitch = -Math.atan2(delta[2], Math.hypot(delta[0], delta[1])) * 180 / Math.PI
      const event = new MouseEvent("mousemove", { bubbles: true })
      Object.defineProperty(event, "movementX", { value: -(((yaw - camera.yawDegrees + 540) % 360) - 180) * 0.35 / 0.066 })
      Object.defineProperty(event, "movementY", { value: (pitch - camera.pitchDegrees) * 0.35 / 0.066 })
      dispatchEvent(event)
      if (!(globalThis as any).__combatImpactFiring) {
        ;(globalThis as any).__combatImpactFiring = true
        dispatchEvent(new MouseEvent("mousedown", { button: 0, bubbles: true }))
      }
    }, 30)
  })
  await expect.poll(async () => page.evaluate(() => document.querySelector<HTMLElement>("main")?.dataset.audioStarts ?? ""),
    { timeout: 30_000 }).toContain("Player.HitSoundDefaultDing")
  const decalCount=await page.evaluate(()=>Number(document.querySelector<HTMLElement>("main")?.dataset.combatDecals??0))
  const after = await page.locator("canvas.world-canvas").screenshot()
  await page.evaluate(() => {
    dispatchEvent(new MouseEvent("mouseup", { button: 0, bubbles: true }))
    clearInterval((globalThis as any).__combatImpactAim)
  })
  const first = decodeScreenshot(before), second = decodeScreenshot(after)
  let changed = 0
  let red = 0
  for (let index = 0; index < first.pixels.length; index += first.channels) {
    const difference = Math.abs(first.pixels[index]! - second.pixels[index]!)
      + Math.abs(first.pixels[index + 1]! - second.pixels[index + 1]!)
      + Math.abs(first.pixels[index + 2]! - second.pixels[index + 2]!)
    if (difference > 36) changed += 1
    if (difference > 36 && second.pixels[index]! > second.pixels[index + 1]! * 1.2
      && second.pixels[index]! > second.pixels[index + 2]! * 1.2) red += 1
  }
  expect(changed).toBeGreaterThan(32)
  expect(red).toBeGreaterThan(8)
  await testInfo.attach("headed-2fort-combat-before", { body: before, contentType: "image/png" })
  await testInfo.attach("headed-2fort-combat-after", { body: after, contentType: "image/png" })
  const seconds = profileSampleSeconds()
  const measurement = await page.evaluate(async duration => {
    const root = document.querySelector<HTMLElement>("main")!, started = performance.now(), tick = Number(root.dataset.snapshotTick)
    let previous = started
    const frames: number[] = []
    await new Promise<void>(resolve => requestAnimationFrame(function frame(now) {
      frames.push(now - previous); previous = now
      now - started >= duration * 1000 ? resolve() : requestAnimationFrame(frame)
    }))
    return { seconds: (performance.now() - started) / 1000, ticks: Number(root.dataset.snapshotTick) - tick, frames }
  }, seconds)
  expect(measurement.ticks).toBeGreaterThan(seconds * 60)
  const report = { schema: "playsrc-tf2-headed-combat-impacts-v1", headed: true, map: "ctf_2fort", decals:decalCount,
    pixels: { changed, red }, ticksPerSecond: measurement.ticks / measurement.seconds,
    frames: summarizeFrameTimes(measurement.frames) }
  const config = await loadLocalConfig()
  const directory = path.join(config.sourceCacheDir, "evidence", "tf2-combat-impacts")
  await mkdir(directory, { recursive: true })
  await writeFile(path.join(directory, "ctf_2fort-combat.json"), `${JSON.stringify(report, null, 2)}\n`)
  console.log(`[combat-impacts] ${JSON.stringify(report)}`)
})
