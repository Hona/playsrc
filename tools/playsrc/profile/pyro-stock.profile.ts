import { createHash } from "node:crypto"
import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, test } from "./application-test"
import { loadLocalConfig } from "../src/config"

const SAMPLE_MILLISECONDS = 6_000

test("headed stock Pyro weapons preserve authored flame, compression blast, shotgun, and Fire Axe pixels", async ({ page }, testInfo) => {
  await page.goto("/", { waitUntil: "load", timeout: 30_000 })
  await page.waitForFunction(() => ["MainMenu", "Failed"].includes(document.querySelector<HTMLElement>("main")?.dataset.phase ?? ""), undefined, { timeout: 180_000, polling: 25 })
  expect(await page.locator("main").getAttribute("data-phase")).toBe("MainMenu")

  const submit = async (command: string): Promise<void> => {
    if (await page.locator("main").getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    const entry = page.locator("[aria-label='Console command']")
    await expect(entry).toBeVisible()
    await entry.fill(command)
    await page.keyboard.press("Enter")
  }
  await submit("map jump_beef")
  await page.waitForFunction(() => {
    const main = document.querySelector<HTMLElement>("main")
    return main?.dataset.phase === "Failed" || (main?.dataset.phase === "Ready" && main.dataset.gameui === "in-game")
  }, undefined, { timeout: 600_000, polling: 25 })
  expect(await page.locator("main").getAttribute("data-phase")).toBe("Ready")

  await submit("joinclass pyro")
  if (await page.locator("main").getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  await page.waitForFunction(() => {
    const main = document.querySelector<HTMLElement>("main")
    return main?.dataset.phase === "Failed" || Number(main?.dataset.hudProbe?.split(":")[1]) === 7
  }, undefined, { timeout: 30_000, polling: 20 })
  expect(await page.locator("main").getAttribute("data-phase")).toBe("Ready")
  const initial = await page.locator("main").evaluate((main) => ({
    hud: (main as HTMLElement).dataset.hudProbe,
    weapon: (main as HTMLElement).dataset.weaponTrace,
    activity: (main as HTMLElement).dataset.viewmodelActivity,
    tick: Number((main as HTMLElement).dataset.snapshotTick),
  }))
  expect(initial.hud?.startsWith("175:7:")).toBe(true)
  expect(initial.weapon).toContain("15:0/200")
  expect(initial.weapon).toContain("7:6/32")
  expect(initial.weapon).toContain("16:0/0")

  const canvas = page.locator("canvas.world-canvas")
  await expect(canvas).toBeVisible()
  const before = await canvas.screenshot()
  await canvas.click({ position: { x: 640, y: 360 } })
  const started = await page.evaluate(() => performance.now())
  const startTick = Number((await page.locator("main").getAttribute("data-snapshot-tick")) ?? "0")
  await page.mouse.down({ button: "left" })
  await page.waitForFunction(() => {
    const main = document.querySelector<HTMLElement>("main")
    return main?.dataset.phase === "Failed" || main?.dataset.weaponTrace?.includes("15:0/19")
  }, undefined, { timeout: 5_000, polling: 20 })
  await page.waitForTimeout(250)
  const emission=await page.locator("main").evaluate(main=>({phase:(main as HTMLElement).dataset.phase,fuel:(main as HTMLElement).dataset.weaponTrace,flames:(main as HTMLElement).dataset.flamePoints,particles:(main as HTMLElement).dataset.particleItems,activity:(main as HTMLElement).dataset.viewmodelActivity,audio:(main as HTMLElement).dataset.audioStarts}))
  if(Number(emission.particles)===0)throw new Error(`Pyro emitted no visible authored particles: ${JSON.stringify(emission)}`)
  const burning = await canvas.screenshot()
  expect(createHash("sha256").update(burning).digest("hex")).not.toBe(createHash("sha256").update(before).digest("hex"))
  const flame = await page.locator("main").evaluate((main) => ({
    particles: Number((main as HTMLElement).dataset.particleItems),
    weapon: (main as HTMLElement).dataset.weaponTrace,
    activity: (main as HTMLElement).dataset.viewmodelActivity,
    audio: (main as HTMLElement).dataset.audioStarts,
  }))
  expect(flame.particles).toBeGreaterThan(0)
  expect(flame.audio).toContain("Weapon_FlameThrower.Fire")
  await page.mouse.up({ button: "left" })

  const beforeAirblast = Number(flame.weapon?.match(/15:0\/(\d+)/)?.[1] ?? "0")
  await page.mouse.click(640, 360, { button: "right" })
  await page.waitForFunction((before) => {
    const main = document.querySelector<HTMLElement>("main")
    const fuel = Number(main?.dataset.weaponTrace?.match(/15:0\/(\d+)/)?.[1] ?? "0")
    return main?.dataset.phase === "Failed" || fuel <= before - 20
  }, beforeAirblast, { timeout: 30_000, polling: 20 })
  expect(await page.locator("main").getAttribute("data-audio-starts")).toContain("Weapon_FlameThrower.AirBurstAttack")

  await page.keyboard.press("Digit2")
  await page.waitForFunction(() => document.querySelector<HTMLElement>("main")?.dataset.viewmodelActivity?.startsWith("ACT_SECONDARY_"), undefined, { timeout: 30_000, polling: 20 })
  await page.waitForTimeout(550)
  await page.mouse.click(640, 360, { button: "left" })
  await page.waitForFunction(() => document.querySelector<HTMLElement>("main")?.dataset.weaponTrace?.includes("7:5/32"), undefined, { timeout: 30_000, polling: 20 })
  expect(await page.locator("main").getAttribute("data-audio-starts")).toContain("Weapon_Shotgun.Single")
  await page.keyboard.press("KeyR")
  await page.waitForFunction(() => document.querySelector<HTMLElement>("main")?.dataset.weaponTrace?.includes("7:6/31"), undefined, { timeout: 30_000, polling: 20 })

  await page.keyboard.press("Digit3")
  await page.waitForFunction(() => document.querySelector<HTMLElement>("main")?.dataset.viewmodelActivity?.startsWith("ACT_MELEE_"), undefined, { timeout: 30_000, polling: 20 })
  await page.waitForTimeout(550)
  await page.mouse.click(640, 360, { button: "left" })
  await page.waitForFunction(() => document.querySelector<HTMLElement>("main")?.dataset.audioStarts?.includes("Weapon_FireAxe.Miss"), undefined, { timeout: 30_000, polling: 20 })

  const elapsedBeforeFinish = await page.evaluate((begin) => performance.now() - begin, started)
  if (elapsedBeforeFinish < SAMPLE_MILLISECONDS) await page.waitForTimeout(SAMPLE_MILLISECONDS - elapsedBeforeFinish)
  const final = await page.locator("main").evaluate((main) => ({
    phase: (main as HTMLElement).dataset.phase,
    tick: Number((main as HTMLElement).dataset.snapshotTick),
    performance: (main as HTMLElement).dataset.performance,
    activity: (main as HTMLElement).dataset.viewmodelActivity,
    particles: Number((main as HTMLElement).dataset.particleItems),
  }))
  const elapsed = await page.evaluate((begin) => performance.now() - begin, started)
  const ticksPerSecond = (final.tick - startTick) / elapsed * 1_000
  expect(final.phase).toBe("Ready")
  expect(ticksPerSecond).toBeGreaterThan(55)

  const report = {
    schema: "playsrc-tf2-pyro-stock-profile-v1",
    elapsedMilliseconds: Number(elapsed.toFixed(2)),
    ticksPerSecond: Number(ticksPerSecond.toFixed(2)),
    initial,
    flame,
    final,
    beforeSha256: createHash("sha256").update(before).digest("hex"),
    burningSha256: createHash("sha256").update(burning).digest("hex"),
  }
  const configuration = await loadLocalConfig(process.cwd())
  const directory = path.join(configuration.sourceCacheDir, "evidence", "tf2-browser-performance")
  await mkdir(directory, { recursive: true })
  await writeFile(path.join(directory, "pyro-stock-profile.json"), `${JSON.stringify(report, null, 2)}\n`)
  await testInfo.attach("headed-pyro-stock-performance", { body: JSON.stringify(report), contentType: "application/json" })
  console.log(JSON.stringify({ elapsedMilliseconds: report.elapsedMilliseconds, ticksPerSecond: report.ticksPerSecond, flameParticles: flame.particles }))
})
