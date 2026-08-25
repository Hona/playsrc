import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, test } from "./application-test"
import { chooseTf2Team } from "./team-selection-evidence"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { loadLocalConfig } from "../src/config"

const PICKUP_COUNTS = Object.freeze({
  "health:small": 5,
  "health:medium": 7,
  "health:full": 1,
  "ammo:small": 1,
  "ammo:medium": 15,
  "ammo:full": 1,
})

test("authored Upward pickups render, restore ammunition, disappear, and respawn without slowing combat", async ({ page }, testInfo) => {
  await page.addInitScript(() => {
    ;(globalThis as typeof globalThis & { __playsrcProfile?: object }).__playsrcProfile = {}
  })
  await page.goto("/")
  await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu", { timeout: 120_000 })
  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  const command = async (value: string) => { await entry.fill(value); await entry.press("Enter") }
  await command("map pl_upward")
  await page.waitForFunction(() => {
    const root = document.querySelector<HTMLElement>("main")
    return root?.dataset.teamSelectionVisible === "true" || root?.dataset.phase === "Ready" || root?.dataset.phase === "Failed"
  }, undefined, { timeout: 600_000 })
  if (await page.locator("main").getAttribute("data-team-selection-visible") === "true") {
    if (await page.locator("main").getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await chooseTf2Team(page, "blue")
    await expect(page.locator("main")).toHaveAttribute("data-phase", "Ready", { timeout: 60_000 })
    await page.keyboard.press("Backquote")
  } else {
    await expect(page.locator("main")).toHaveAttribute("data-phase", "Ready", { timeout: 60_000 })
    await command("jointeam blue")
  }
  await command("joinclass sniper")
  await command("tf_bot_add blue soldier normal")
  await expect(page.locator("main")).toHaveAttribute("data-bot-count", "1", { timeout: 30_000 })
  await command("noclip")
  await page.keyboard.press("Backquote")
  await expect(page.locator("main")).toHaveAttribute("data-console-visible", "false")
  await expect(page.locator("main")).toHaveAttribute("data-movement-mode", "1")
  await expect(page.locator("main")).toHaveAttribute("data-pickup-count", "30")

  const authored = await page.evaluate(() => {
    const profile = (globalThis as any).__playsrcProfile
    const root = document.querySelector<HTMLElement>("main")!
    const position = (root.dataset.cameraPosition ?? "").split(",").map(Number)
    position[2] = position[2]! - Number(root.dataset.viewOffset?.split(",")[2] ?? 68)
    const counts: Record<string, number> = {}
    for (const pickup of profile.pickups) counts[`${pickup.kind}:${pickup.size}`] = (counts[`${pickup.kind}:${pickup.size}`] ?? 0) + 1
    const candidates = profile.pickups.filter((pickup: any) => pickup.kind === "ammo" && pickup.available)
      .toSorted((left: any, right: any) => {
        const score = (item: any) => Math.hypot(item.origin[0] - position[0], item.origin[1] - position[1]) + Math.abs(item.origin[2] - position[2]) * 8
        return score(left) - score(right)
      })
    const pickup = candidates[0]
    if (!pickup) throw new Error("authored Upward ammunition pickups are missing")
    return { counts, pickup, initialPosition: position }
  })
  expect(authored.counts).toEqual(PICKUP_COUNTS)

  const canvas = page.locator(".world-canvas")
  await page.bringToFront()
  await canvas.click()
  await expect(page.locator("main")).toHaveAttribute("data-pointer-locked", "true", { timeout: 10_000 })
  const beforeFires = Number(await page.locator("main").getAttribute("data-fire-events"))
  await page.mouse.down({ button: "left" })
  await expect.poll(async () => Number(await page.locator("main").getAttribute("data-fire-events")), { timeout: 30_000 }).toBeGreaterThan(beforeFires)
  await page.mouse.up({ button: "left" })
  await expect.poll(async () => Number((await page.locator("main").getAttribute("data-hud-probe"))?.split(":")[4]), { timeout: 10_000 }).toBeLessThan(25)
  const reserveBefore = Number((await page.locator("main").getAttribute("data-hud-probe"))?.split(":")[4])

  await page.evaluate((target) => {
    const profile = (globalThis as any).__playsrcProfile
    profile.displacementCameraOverride = {
      position: [target.origin[0] - 85, target.origin[1], target.origin[2] + 28],
      yawDegrees: 0,
      pitchDegrees: 8,
    }
  }, authored.pickup)
  await page.waitForFunction(() => {
    const override = (globalThis as any).__playsrcProfile.displacementCameraOverride
    return document.querySelector<HTMLElement>(".world-canvas")?.dataset.displayCameraPosition === override.position.join(",")
  })
  const visible = await canvas.screenshot()
  await testInfo.attach("headed-authored-ammo-pack-visible", { body: visible, contentType: "image/png" })

  const started = Date.now()
  await page.keyboard.press("Backquote")
  await command(`setpos ${authored.pickup.origin.join(" ")}`)
  await page.keyboard.press("Backquote")
  await page.waitForFunction((identity) => !(globalThis as any).__playsrcProfile.pickups.find((pickup: any) => pickup.identity === identity)?.available, authored.pickup.identity, { timeout: 10_000 })
  const collected = await page.evaluate(({ identity, started }) => {
    const root = document.querySelector<HTMLElement>("main")!
    return {
      seconds: (Date.now() - started) / 1000,
      pickup: (globalThis as any).__playsrcProfile.pickups.find((pickup: any) => pickup.identity === identity),
      hud: root.dataset.hudProbe,
    }
  }, { identity: authored.pickup.identity, started })
  expect(collected.pickup.available).toBe(false)
  expect(Number(collected.hud.split(":")[4])).toBeGreaterThan(reserveBefore)
  await page.waitForFunction((identity) => document.querySelector<HTMLElement>("main")?.dataset.audioStarts?.includes("AmmoPack.Touch")
    && !(globalThis as any).__playsrcProfile.pickups.find((pickup: any) => pickup.identity === identity)?.available, authored.pickup.identity)
  const hidden = await canvas.screenshot()
  await testInfo.attach("headed-authored-ammo-pack-consumed", { body: hidden, contentType: "image/png" })

  const seconds = profileSampleSeconds()
  const measured = await page.evaluate(async (duration) => {
    const root = document.querySelector<HTMLElement>("main")!
    const started = performance.now(), firstTick = Number(root.dataset.snapshotTick)
    const frames: number[] = [], samples: Array<{ pickups: number; bots: number; total: number }> = []
    let previous = started
    await new Promise<void>((resolve) => {
      const frame = (now: number) => {
        frames.push(now - previous); previous = now
        if (root.dataset.performanceDetail) {
          const sample = JSON.parse(root.dataset.performanceDetail)
          samples.push({ pickups: sample.pickups, bots: sample.bots, total: sample.total })
        }
        if (now - started >= duration * 1000) resolve()
        else requestAnimationFrame(frame)
      }
      requestAnimationFrame(frame)
    })
    return { seconds: (performance.now() - started) / 1000, firstTick, lastTick: Number(root.dataset.snapshotTick), frames, samples }
  }, seconds)
  expect(measured.lastTick - measured.firstTick).toBeGreaterThan(seconds * 60)
  expect(measured.samples.every((sample) => sample.pickups === 30 && sample.bots === 1)).toBe(true)

  await page.waitForFunction((identity) => Boolean((globalThis as any).__playsrcProfile.pickups.find((pickup: any) => pickup.identity === identity)?.available), authored.pickup.identity, { timeout: 15_000 })
  const respawned = await canvas.screenshot()
  await testInfo.attach("headed-authored-ammo-pack-respawned", { body: respawned, contentType: "image/png" })
  const pixels = await page.evaluate(async ({ before, after, returned }) => {
    const image = async (encoded: string) => {
      const source = new Image(); source.src = `data:image/png;base64,${encoded}`; await source.decode()
      const scratch = document.createElement("canvas"); scratch.width = source.width; scratch.height = source.height
      const context = scratch.getContext("2d")!; context.drawImage(source, 0, 0)
      return context.getImageData(0, 0, source.width, source.height)
    }
    const [visible, hidden, respawned] = await Promise.all([image(before), image(after), image(returned)])
    let removed = 0, restored = 0
    for (let y = 180; y < visible.height - 120; y++) for (let x = 360; x < visible.width - 360; x++) {
      const at = (y * visible.width + x) * 4
      const removedDelta = Math.abs(visible.data[at]! - hidden.data[at]!) + Math.abs(visible.data[at + 1]! - hidden.data[at + 1]!) + Math.abs(visible.data[at + 2]! - hidden.data[at + 2]!)
      const restoredDelta = Math.abs(respawned.data[at]! - hidden.data[at]!) + Math.abs(respawned.data[at + 1]! - hidden.data[at + 1]!) + Math.abs(respawned.data[at + 2]! - hidden.data[at + 2]!)
      if (removedDelta > 30) removed++
      if (restoredDelta > 30) restored++
    }
    return { removed, restored }
  }, { before: visible.toString("base64"), after: hidden.toString("base64"), returned: respawned.toString("base64") })
  expect(pixels.removed).toBeGreaterThan(40)
  expect(pixels.restored).toBeGreaterThan(40)

  const report = {
    schema: "playsrc-tf2-headed-pickup-resupply-v1",
    headed: true,
    target: "pl_upward",
    counts: authored.counts,
    pickup: authored.pickup,
    collected,
    reserveBefore,
    simulation: {
      seconds: Number(measured.seconds.toFixed(3)),
      firstTick: measured.firstTick,
      lastTick: measured.lastTick,
      ticksPerSecond: Number(((measured.lastTick - measured.firstTick) / measured.seconds).toFixed(2)),
    },
    frames: summarizeFrameTimes(measured.frames),
    pixels,
  }
  const { sourceCacheDir } = await loadLocalConfig()
  const directory = path.join(sourceCacheDir, "evidence", "tf2-pickup-resupply")
  await mkdir(directory, { recursive: true })
  await Promise.all([
    writeFile(path.join(directory, "pl_upward-pickups.json"), `${JSON.stringify(report, null, 2)}\n`),
    writeFile(path.join(directory, "pl_upward-visible.png"), visible),
    writeFile(path.join(directory, "pl_upward-hidden.png"), hidden),
    writeFile(path.join(directory, "pl_upward-respawned.png"), respawned),
  ])
  console.log(`[pickup-resupply] ${JSON.stringify(report)}`)
})
