import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
import { expect, test } from "./application-test"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"

test("authored pl_upward bot players join, navigate, render and leave", async ({ page }, testInfo) => {
  await page.addInitScript(() => {
    ;(globalThis as typeof globalThis & { __playsrcProfile?: object }).__playsrcProfile = {}
  })
  await page.goto("/")
  await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu", { timeout: 120_000 })
  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  await entry.fill("map pl_upward")
  await entry.press("Enter")
  await expect(page.locator("main")).toHaveAttribute("data-phase", "Ready", { timeout: 600_000 })
  if (await page.locator("main").getAttribute("data-class-selection-visible") === "true") {
    await entry.fill("joinclass soldier")
    await entry.press("Enter")
    await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "false")
  }
  await expect(page.locator("main")).toHaveAttribute("data-bot-count", "0")
  await entry.fill("tf_bot_add blue soldier normal")
  await entry.press("Enter")
  await expect(page.locator("main")).toHaveAttribute("data-bot-count", "1", { timeout: 30_000 })
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile?.bots?.length === 1)
  const initial = await page.evaluate(() => {
    const profile = (globalThis as any).__playsrcProfile
    const bot = profile.bots?.[0]
    if (!bot) throw new Error("bot facts are absent from the real headed presentation")
    return bot as {
      identity: number; class: number; team: number; objective: number; area: number | null
      remainingPathAreas: number; position: [number, number, number]; tick: string
    }
  })
  expect(initial.class).toBe(3)
  expect(initial.team).toBe(3)
  expect(initial.objective).toBe(1)
  expect(initial.area).not.toBeNull()
  expect(initial.remainingPathAreas).toBeGreaterThan(1)
  await entry.fill("tf_bot_add red demoman hard")
  await entry.press("Enter")
  await expect(page.locator("main")).toHaveAttribute("data-bot-count", "2")
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile?.bots?.length === 2)
  const defender = await page.evaluate(() => (globalThis as any).__playsrcProfile.bots?.find((bot: any) => bot.team === 2))
  expect(defender).toMatchObject({ class: 4, team: 2, difficulty: 2, objective: 2, health: 175 })
  await page.keyboard.press("Backquote")
  await expect(page.locator("main")).toHaveAttribute("data-console-visible", "false")
  const warmupTick = Number(await page.locator("main").getAttribute("data-snapshot-tick"))
  await page.waitForFunction((first) => Number(document.querySelector<HTMLElement>("main")?.dataset.snapshotTick) >= first + 40, warmupTick)

  const sampleSeconds = profileSampleSeconds()
  const measurement = await page.evaluate(async (seconds) => {
    const root = document.querySelector<HTMLElement>("main")!
    const profile = (globalThis as any).__playsrcProfile
    const start = performance.now()
    const firstTick = Number(root.dataset.snapshotTick)
    let previous = start
    const frames: number[] = []
    const samples: Array<{ tick: number; bots: number; total: number; models: number }> = []
    await new Promise<void>((resolve) => {
      const frame = (now: number) => {
        frames.push(now - previous)
        previous = now
        const detail = root.dataset.performanceDetail
        if (detail) {
          const value = JSON.parse(detail)
          samples.push({ tick: Number(value.tick), bots: value.bots, total: value.total, models: value.models })
        }
        if (now - start >= seconds * 1000) resolve()
        else requestAnimationFrame(frame)
      }
      requestAnimationFrame(frame)
    })
    const bot = profile.bots?.[0]
    return {
      seconds: (performance.now() - start) / 1000,
      firstTick,
      lastTick: Number(root.dataset.snapshotTick),
      frames,
      samples,
      bot,
    }
  }, sampleSeconds)
  expect(measurement.bot).toBeDefined()
  expect(measurement.lastTick - measurement.firstTick).toBeGreaterThan(sampleSeconds * 63)
  expect(measurement.samples.every((sample) => sample.bots === 2)).toBe(true)
  const distance = Math.hypot(...measurement.bot.position.map((value: number, index: number) => value - initial.position[index]!))
  expect(distance).toBeGreaterThan(10)

  await page.evaluate(() => {
    const profile = (globalThis as any).__playsrcProfile
    const bot = profile.bots[0]
    const yaw = bot.yawDegrees * Math.PI / 180
    profile.displacementCameraOverride = {
      position: [bot.position[0] - Math.cos(yaw) * 180, bot.position[1] - Math.sin(yaw) * 180, bot.position[2] + 58],
      yawDegrees: bot.yawDegrees,
      pitchDegrees: 0,
    }
  })
  await expect.poll(async () => page.locator(".world-canvas").getAttribute("data-display-frame")).not.toBeNull()
  await page.waitForFunction(() => {
    const profile = (globalThis as any).__playsrcProfile
    const camera = profile.displacementCameraOverride
    const shown = document.querySelector<HTMLElement>(".world-canvas")?.dataset.displayCameraPosition
    return shown === camera.position.join(",")
  })
  const visible = await page.locator(".world-canvas").screenshot()
  await testInfo.attach("headed-visible-blu-soldier", { body: visible, contentType: "image/png" })

  await page.keyboard.press("Backquote")
  await entry.fill("tf_bot_kick blue")
  await entry.press("Enter")
  await expect(page.locator("main")).toHaveAttribute("data-bot-count", "1")
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile?.bots?.length === 1 && (globalThis as any).__playsrcProfile.bots[0].team === 2)
  expect(await page.evaluate(() => (globalThis as any).__playsrcProfile.bots[0].team)).toBe(2)
  const admittedClasses = [3, 4]
  for (const [name, identity] of [["scout", 1], ["sniper", 2], ["medic", 5], ["heavy", 6], ["pyro", 7], ["spy", 8], ["engineer", 9]] as const) {
    await entry.fill(`tf_bot_add blue ${name}`)
    await entry.press("Enter")
    await expect(page.locator("main")).toHaveAttribute("data-bot-count", "2")
    await page.waitForFunction((expected) => (globalThis as any).__playsrcProfile?.bots?.some((bot: any) => bot.team === 3 && bot.class === expected), identity)
    expect(await page.evaluate(() => (globalThis as any).__playsrcProfile.bots.find((bot: any) => bot.team === 3)?.class)).toBe(identity)
    admittedClasses.push(identity)
    await entry.fill("tf_bot_kick blue")
    await entry.press("Enter")
    await expect(page.locator("main")).toHaveAttribute("data-bot-count", "1")
  }
  await entry.fill("tf_bot_add blue 3")
  await entry.press("Enter")
  await expect(page.locator("main")).toHaveAttribute("data-bot-count", "4")
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile?.bots?.length === 4)
  const presetRoster = await page.evaluate(() => (globalThis as any).__playsrcProfile.bots.filter((bot: any) => bot.team === 3).map((bot: any) => bot.class))
  expect(presetRoster).toEqual([5, 9, 3])
  await entry.fill("tf_bot_kick all")
  await entry.press("Enter")
  await expect(page.locator("main")).toHaveAttribute("data-bot-count", "0")
  await page.keyboard.press("Backquote")
  await expect(page.locator("main")).toHaveAttribute("data-console-visible", "false")
  const absent = await page.locator(".world-canvas").screenshot()
  await testInfo.attach("headed-bot-removed", { body: absent, contentType: "image/png" })
  const pixels = await page.evaluate(async ({ present, removed }) => {
    const decode = async (encoded: string): Promise<ImageData> => {
      const image = new Image()
      image.src = `data:image/png;base64,${encoded}`
      await image.decode()
      const surface = document.createElement("canvas")
      surface.width = image.width
      surface.height = image.height
      const context = surface.getContext("2d")!
      context.drawImage(image, 0, 0)
      return context.getImageData(0, 0, image.width, image.height)
    }
    const [before, after] = await Promise.all([decode(present), decode(removed)])
    let changed = 0
    let bluePlayer = 0
    for (let y = 100; y < before.height - 100; y += 1) {
      for (let x = 360; x < before.width - 360; x += 1) {
        const index = (y * before.width + x) * 4
        const delta = Math.abs(before.data[index]! - after.data[index]!)
          + Math.abs(before.data[index + 1]! - after.data[index + 1]!)
          + Math.abs(before.data[index + 2]! - after.data[index + 2]!)
        if (delta > 36) {
          changed += 1
          if (before.data[index + 2]! > before.data[index]! + 8) bluePlayer += 1
        }
      }
    }
    return { changed, bluePlayer }
  }, { present: visible.toString("base64"), removed: absent.toString("base64") })
  expect(pixels.changed).toBeGreaterThan(500)
  expect(pixels.bluePlayer).toBeGreaterThan(100)

  const report = {
    schema: "playsrc-tf2-headed-bot-players-v1",
    headed: true,
    target: "pl_upward",
    authoredNavigation: {
      sha256: "13de0c3e2666d2194474d855683cbabb807eead1c24587fd093a5c70a04cd0b4",
      byteLength: 2_471_913,
      areas: 2_617,
    },
    bot: { ...initial, final: measurement.bot, distance },
    defender,
    admittedClasses: admittedClasses.toSorted((left, right) => left - right),
    presetRoster,
    simulation: {
      seconds: Number(measurement.seconds.toFixed(3)),
      firstTick: measurement.firstTick,
      lastTick: measurement.lastTick,
      ticksPerSecond: Number(((measurement.lastTick - measurement.firstTick) / measurement.seconds).toFixed(2)),
    },
    frames: summarizeFrameTimes(measurement.frames),
    models: {
      samples: measurement.samples.length,
      maximumMilliseconds: Number(Math.max(...measurement.samples.map((sample) => sample.models)).toFixed(3)),
    },
    visiblePlayerPixelsChangedAfterRemoval: true,
    visiblePlayerPixels: pixels,
  }
  const local = await loadLocalConfig()
  const directory = path.join(local.sourceCacheDir, "evidence", "tf2-bot-players")
  await mkdir(directory, { recursive: true })
  await Promise.all([
    writeFile(path.join(directory, "pl_upward-bot-players.json"), `${JSON.stringify(report, null, 2)}\n`),
    writeFile(path.join(directory, "pl_upward-blu-soldier.png"), visible),
    writeFile(path.join(directory, "pl_upward-bot-removed.png"), absent),
  ])
  console.log(`[bot-players] ${JSON.stringify(report)}`)
})
