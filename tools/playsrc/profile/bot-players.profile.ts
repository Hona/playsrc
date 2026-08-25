import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
import { expect, test } from "./application-test"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"

test("authored pl_upward Scout, Soldier and Heavy bots fight, take damage and preserve real-time simulation", async ({ page }, testInfo) => {
  await page.addInitScript(() => {
    ;(globalThis as typeof globalThis & { __playsrcProfile?: object }).__playsrcProfile = {}
  })
  await page.goto("/")
  await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu", { timeout: 120_000 })
  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  const command = async (value: string) => { await entry.fill(value); await entry.press("Enter") }
  await command("map pl_upward")
  await expect(page.locator("main")).toHaveAttribute("data-phase", "Ready", { timeout: 600_000 })
  await command("joinclass heavyweapons")
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "false")
  await command("jointeam blue")
  await expect.poll(async () => (await page.locator("main").getAttribute("data-hud-probe"))?.split(":")[1]).toBe("6")

  const add = async (value: string, expected: number) => {
    await command(`tf_bot_add ${value}`)
    await expect(page.locator("main")).toHaveAttribute("data-bot-count", String(expected), { timeout: 30_000 })
    await page.waitForFunction((count) => (globalThis as any).__playsrcProfile?.bots?.length === count, expected)
  }
  await add("red scout expert", 1)
  await add("red soldier expert", 2)
  await add("blue scout normal", 3)
  await add("blue soldier normal", 4)
  const initial = await page.evaluate(() => structuredClone((globalThis as any).__playsrcProfile.bots))
  expect(initial.map((bot: any) => [bot.team, bot.class, bot.weapon?.identity, bot.objective])).toEqual([
    [2, 1, 4, 2], [2, 3, 1, 2], [3, 1, 4, 1], [3, 3, 1, 1],
  ])
  expect(initial.every((bot: any) => bot.area !== null && bot.remainingPathAreas > 0)).toBe(true)

  await command("noclip")
  await page.keyboard.press("Backquote")
  await expect(page.locator("main")).toHaveAttribute("data-console-visible", "false")
  await expect(page.locator("main")).toHaveAttribute("data-movement-mode", "1")
  const engagement = await page.evaluate(async () => {
    const root = document.querySelector<HTMLElement>("main")!
    const profile = (globalThis as any).__playsrcProfile
    const position = () => (root.dataset.cameraPosition ?? "").split(",").map(Number)
    const target = () => profile.bots.find((bot: any) => bot.team === 2 && bot.class === 1 && bot.lifecycle === 1)
      ?? profile.bots.find((bot: any) => bot.team === 2 && bot.lifecycle === 1)
    const health = () => Number(root.dataset.hudProbe?.split(":")[0] ?? 0)
    const held = new Set<string>()
    const press = (code: string, value: boolean) => {
      if (value === held.has(code)) return
      dispatchEvent(new KeyboardEvent(value ? "keydown" : "keyup", { code, key: code.slice(-1).toLowerCase(), bubbles: true }))
      if (value) held.add(code)
      else held.delete(code)
    }
    const started = performance.now(), firstHealth = health()
    let minimumRange = Number.POSITIVE_INFINITY
    try {
      while (performance.now() - started < 8_000) {
        if (root.dataset.phase === "Failed") throw new Error(root.dataset.detail)
        const bot = target()
        if (!bot || health() <= 0) break
        const current = position(), dx = bot.position[0] - current[0], dy = bot.position[1] - current[1]
        const dz = bot.position[2] + 41 - current[2]
        const range = Math.hypot(dx, dy, dz)
        minimumRange = Math.min(minimumRange, range)
        const yaw = Number(root.dataset.cameraYaw ?? 0) * Math.PI / 180
        const forward = dx * Math.cos(yaw) + dy * Math.sin(yaw)
        const left = -dx * Math.sin(yaw) + dy * Math.cos(yaw)
        press("KeyW", range > 140 && forward > 40)
        press("KeyS", range > 140 && forward < -40)
        press("KeyA", range > 140 && left > 40)
        press("KeyD", range > 140 && left < -40)
        if (profile.bots.filter((value: any) => value.team === 2).every((value: any) => value.shots > 0)
          && profile.bots.some((value: any) => value.hits > 0)) break
        await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
      }
    } finally {
      for (const code of [...held]) press(code, false)
    }
    return {
      seconds: Number(((performance.now() - started) / 1000).toFixed(3)),
      minimumRange: Number(minimumRange.toFixed(2)),
      firstHealth,
      finalHealth: health(),
      bots: structuredClone(profile.bots),
      projectiles: Number(root.dataset.projectiles ?? 0),
      explosions: Number(root.dataset.explosionEvents ?? 0),
    }
  })
  expect(engagement.minimumRange).toBeLessThan(300)
  expect(engagement.bots.filter((bot: any) => bot.team === 2).some((bot: any) => bot.shots > 0)).toBe(true)
  expect(engagement.bots.some((bot: any) => bot.hits > 0 || bot.health < bot.maximumHealth || bot.deaths > 0)
    || engagement.finalHealth < engagement.firstHealth).toBe(true)

  const seconds = profileSampleSeconds()
  const measured = await page.evaluate(async (duration) => {
    const root = document.querySelector<HTMLElement>("main")!
    const profile = (globalThis as any).__playsrcProfile
    const started = performance.now(), firstTick = Number(root.dataset.snapshotTick)
    const frames: number[] = [], samples: Array<{ bots: number; models: number; total: number }> = []
    let previous = started
    await new Promise<void>((resolve) => {
      const frame = (now: number) => {
        frames.push(now - previous); previous = now
        if (root.dataset.performanceDetail) {
          const sample = JSON.parse(root.dataset.performanceDetail)
          samples.push({ bots: sample.bots, models: sample.models, total: sample.total })
        }
        if (now - started >= duration * 1000) resolve()
        else requestAnimationFrame(frame)
      }
      requestAnimationFrame(frame)
    })
    return {
      seconds: (performance.now() - started) / 1000,
      firstTick,
      lastTick: Number(root.dataset.snapshotTick),
      frames,
      samples,
      bots: structuredClone(profile.bots),
    }
  }, seconds)
  expect(measured.lastTick - measured.firstTick).toBeGreaterThan(seconds * 63)
  expect(measured.samples.every((sample) => sample.bots === 4)).toBe(true)

  await page.evaluate(() => {
    const profile = (globalThis as any).__playsrcProfile
    const bot = profile.bots.find((value: any) => value.lifecycle === 1 && value.team === 2)
      ?? profile.bots.find((value: any) => value.lifecycle === 1)
    if (!bot) throw new Error("no living authored bot remains visible")
    const yaw = bot.yawDegrees * Math.PI / 180
    profile.displacementCameraOverride = {
      position: [bot.position[0] - Math.cos(yaw) * 150, bot.position[1] - Math.sin(yaw) * 150, bot.position[2] + 58],
      yawDegrees: bot.yawDegrees,
      pitchDegrees: 0,
    }
  })
  await page.waitForFunction(() => {
    const override = (globalThis as any).__playsrcProfile.displacementCameraOverride
    return document.querySelector<HTMLElement>(".world-canvas")?.dataset.displayCameraPosition === override.position.join(",")
  })
  const present = await page.locator(".world-canvas").screenshot()
  await testInfo.attach("headed-multi-bot-combat", { body: present, contentType: "image/png" })

  await page.keyboard.press("Backquote")
  await command("tf_bot_kick all")
  await expect(page.locator("main")).toHaveAttribute("data-bot-count", "0")
  await page.keyboard.press("Backquote")
  const absent = await page.locator(".world-canvas").screenshot()
  await testInfo.attach("headed-combat-bots-removed", { body: absent, contentType: "image/png" })
  const pixels = await page.evaluate(async ({ before, after }) => {
    const image = async (encoded: string) => {
      const source = new Image(); source.src = `data:image/png;base64,${encoded}`; await source.decode()
      const canvas = document.createElement("canvas"); canvas.width = source.width; canvas.height = source.height
      const context = canvas.getContext("2d")!; context.drawImage(source, 0, 0)
      return context.getImageData(0, 0, source.width, source.height)
    }
    const [left, right] = await Promise.all([image(before), image(after)])
    let changed = 0, playerColored = 0
    for (let y = 100; y < left.height - 100; y++) for (let x = 320; x < left.width - 320; x++) {
      const at = (y * left.width + x) * 4
      const delta = Math.abs(left.data[at]! - right.data[at]!) + Math.abs(left.data[at + 1]! - right.data[at + 1]!) + Math.abs(left.data[at + 2]! - right.data[at + 2]!)
      if (delta > 36) { changed++; if (Math.abs(left.data[at]! - left.data[at + 2]!) > 8) playerColored++ }
    }
    return { changed, playerColored }
  }, { before: present.toString("base64"), after: absent.toString("base64") })
  expect(pixels.changed).toBeGreaterThan(500)
  expect(pixels.playerColored).toBeGreaterThan(100)

  const report = {
    schema: "playsrc-tf2-headed-bot-combat-v1",
    headed: true,
    target: "pl_upward",
    authoredNavigation: { sha256: "13de0c3e2666d2194474d855683cbabb807eead1c24587fd093a5c70a04cd0b4", byteLength: 2_471_913, areas: 2_617 },
    classes: [...new Set(initial.map((bot: any) => bot.class))].toSorted(),
    teams: [...new Set(initial.map((bot: any) => bot.team))].toSorted(),
    objectives: [...new Set(initial.map((bot: any) => bot.objective))].toSorted(),
    engagement,
    bots: measured.bots,
    simulation: {
      seconds: Number(measured.seconds.toFixed(3)),
      firstTick: measured.firstTick,
      lastTick: measured.lastTick,
      ticksPerSecond: Number(((measured.lastTick - measured.firstTick) / measured.seconds).toFixed(2)),
    },
    frames: summarizeFrameTimes(measured.frames),
    models: { samples: measured.samples.length, maximumMilliseconds: Number(Math.max(...measured.samples.map((sample) => sample.models)).toFixed(3)) },
    visiblePlayerPixels: pixels,
  }
  const { sourceCacheDir } = await loadLocalConfig()
  const directory = path.join(sourceCacheDir, "evidence", "tf2-bot-combat")
  await mkdir(directory, { recursive: true })
  await Promise.all([
    writeFile(path.join(directory, "pl_upward-bot-combat.json"), `${JSON.stringify(report, null, 2)}\n`),
    writeFile(path.join(directory, "pl_upward-bot-combat.png"), present),
    writeFile(path.join(directory, "pl_upward-bots-removed.png"), absent),
  ])
  console.log(`[bot-combat] ${JSON.stringify(report)}`)
})
