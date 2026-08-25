import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
import { expect, test } from "./application-test"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { chooseTf2Team } from "./team-selection-evidence"

test("headed ctf_2fort nine-class bots follow Source NAV, intelligence routes, and live combat", async ({ page }, testInfo) => {
  await page.addInitScript(() => {
    ;(globalThis as typeof globalThis & { __playsrcProfile?: object }).__playsrcProfile = {}
  })
  await page.goto("/")
  const root = page.locator("main")
  await expect(root).toHaveAttribute("data-phase", "MainMenu", { timeout: 120_000 })
  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  const command = async (value: string): Promise<void> => {
    await entry.fill(value)
    await entry.press("Enter")
  }
  await command("map ctf_2fort")
  await page.waitForFunction(() => {
    const main = document.querySelector<HTMLElement>("main")
    return main?.dataset.teamSelectionVisible === "true"
      || main?.dataset.phase === "Ready"
      || main?.dataset.phase === "Failed"
  }, undefined, { timeout: 600_000 })
  if (await root.getAttribute("data-team-selection-visible") === "true") {
    if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await chooseTf2Team(page, "red")
  }
  await expect(root).toHaveAttribute("data-phase", "Ready", { timeout: 60_000 })
  if (await root.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
  await command("joinclass heavyweapons")
  await expect(root).toHaveAttribute("data-class-selection-visible", "false")
  await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[1]).toBe("6")
  await expect(root).toHaveAttribute("data-ctf", /^0:0:3:0:/)

  const add = async (value: string, count: number): Promise<void> => {
    await command(`tf_bot_add ${value}`)
    await expect(root).toHaveAttribute("data-bot-count", String(count), { timeout: 30_000 })
    await page.waitForFunction((expected) => {
      const bots = (globalThis as any).__playsrcProfile?.bots
      return bots?.length === expected && bots.every((bot: any) => bot.area !== null && bot.remainingPathAreas > 0)
    }, count, { timeout: 30_000 })
  }
  await add("red soldier expert", 1)
  await add("blue scout expert", 2)
  const initial = await page.evaluate(() => structuredClone((globalThis as any).__playsrcProfile.bots))
  expect(initial.map((bot: any) => [bot.team, bot.class, bot.objective])).toEqual([[2, 3, 3], [3, 1, 3]])
  expect(initial.find((bot: any) => bot.team === 2).position[1]).toBeGreaterThan(0)
  expect(initial.find((bot: any) => bot.team === 3).position[1]).toBeLessThan(0)
  expect(initial.every((bot: any) => bot.remainingPathAreas > 4)).toBe(true)

  const classes = [1]
  for (const [name, identity] of [
    ["sniper", 2], ["soldier", 3], ["demoman", 4], ["medic", 5],
    ["heavy", 6], ["pyro", 7], ["spy", 8], ["engineer", 9],
  ] as const) {
    await command("tf_bot_kick blue")
    await expect(root).toHaveAttribute("data-bot-count", "1")
    await add(`blue ${name} expert`, 2)
    const bot = await page.evaluate(() => (globalThis as any).__playsrcProfile.bots.find((value: any) => value.team === 3))
    expect(bot).toMatchObject({ class: identity, team: 3, objective: 3 })
    expect(bot.position[1]).toBeLessThan(0)
    classes.push(identity)
  }
  await command("tf_bot_kick blue")
  await expect(root).toHaveAttribute("data-bot-count", "1")
  await add("blue scout expert", 2)
  await add("blue soldier expert", 3)

  const bluePosition = await page.evaluate(() => {
    const bot = (globalThis as any).__playsrcProfile.bots.find((value: any) => value.team === 3 && value.class === 1)
    if (!bot) throw new Error("BLU Scout is absent from the live 2Fort roster")
    return bot.position as [number, number, number]
  })
  await command("noclip")
  await expect(root).toHaveAttribute("data-movement-mode", "1")
  await command(`setpos ${bluePosition[0]} ${bluePosition[1]} ${bluePosition[2] + 1}`)
  const firstHealth = Number((await root.getAttribute("data-hud-probe"))?.split(":")[0])
  await page.keyboard.press("Backquote")
  await expect(root).toHaveAttribute("data-console-visible", "false")
  await page.waitForFunction(() => {
    const main = document.querySelector<HTMLElement>("main")
    if (main?.dataset.phase === "Failed") throw new Error(main.dataset.detail)
    const bots = (globalThis as any).__playsrcProfile?.bots ?? []
    const health = Number(main?.dataset.hudProbe?.split(":")[0] ?? 0)
    return bots.some((bot: any) => bot.team === 3 && bot.shots > 0)
      && (bots.some((bot: any) => bot.hits > 0) || health < 300)
  }, undefined, { timeout: 15_000 })
  const engagement = await page.evaluate(() => ({
    health: Number(document.querySelector<HTMLElement>("main")?.dataset.hudProbe?.split(":")[0] ?? 0),
    bots: structuredClone((globalThis as any).__playsrcProfile.bots),
  }))
  expect(engagement.bots.some((bot: any) => bot.team === 3 && bot.shots > 0)).toBe(true)
  expect(engagement.bots.some((bot: any) => bot.hits > 0) || engagement.health < firstHealth).toBe(true)

  const sampleSeconds = profileSampleSeconds()
  const measurement = await page.evaluate(async (seconds) => {
    const main = document.querySelector<HTMLElement>("main")!
    const profile = (globalThis as any).__playsrcProfile
    const started = performance.now()
    const firstTick = Number(main.dataset.snapshotTick)
    const frames: number[] = []
    const samples: Array<{ bots: number; models: number; total: number }> = []
    let previous = started
    await new Promise<void>((resolve) => {
      const frame = (now: number): void => {
        frames.push(now - previous)
        previous = now
        if (main.dataset.performanceDetail) {
          const detail = JSON.parse(main.dataset.performanceDetail)
          samples.push({ bots: detail.bots, models: detail.models, total: detail.total })
        }
        if (now - started >= seconds * 1000) resolve()
        else requestAnimationFrame(frame)
      }
      requestAnimationFrame(frame)
    })
    return {
      seconds: (performance.now() - started) / 1000,
      firstTick,
      lastTick: Number(main.dataset.snapshotTick),
      frames,
      samples,
      bots: structuredClone(profile.bots),
    }
  }, sampleSeconds)
  console.log(`[2fort-bot-sample] ${JSON.stringify({
    seconds: Number(measurement.seconds.toFixed(3)),
    ticks: measurement.lastTick - measurement.firstTick,
    frames: summarizeFrameTimes(measurement.frames),
    modelMaximumMilliseconds: Number(Math.max(...measurement.samples.map((sample) => sample.models)).toFixed(3)),
    totalMaximumMilliseconds: Number(Math.max(...measurement.samples.map((sample) => sample.total)).toFixed(3)),
  })}`)
  expect(measurement.lastTick - measurement.firstTick).toBeGreaterThan(sampleSeconds * 63)
  expect(measurement.samples.length).toBeGreaterThan(0)
  expect(measurement.samples.every((sample) => sample.bots === 3)).toBe(true)

  await page.evaluate(() => {
    const profile = (globalThis as any).__playsrcProfile
    const bot = profile.bots.find((value: any) => value.lifecycle === 1 && value.team === 3)
      ?? profile.bots.find((value: any) => value.lifecycle === 1)
    if (!bot) throw new Error("no living 2Fort bot remains visible")
    const yaw = bot.yawDegrees * Math.PI / 180
    profile.displacementCameraOverride = {
      position: [bot.position[0] - Math.cos(yaw) * 96, bot.position[1] - Math.sin(yaw) * 96, bot.position[2] + 58],
      yawDegrees: bot.yawDegrees,
      pitchDegrees: 0,
    }
  })
  await page.waitForFunction(() => {
    const override = (globalThis as any).__playsrcProfile.displacementCameraOverride
    return document.querySelector<HTMLElement>(".world-canvas")?.dataset.displayCameraPosition === override.position.join(",")
  })
  const visible = await page.locator(".world-canvas").screenshot()
  await testInfo.attach("headed-2fort-source-navigation-bot", { body: visible, contentType: "image/png" })

  await page.keyboard.press("Backquote")
  await command("tf_bot_kick all")
  await expect(root).toHaveAttribute("data-bot-count", "0")
  await page.keyboard.press("Backquote")
  await expect(root).toHaveAttribute("data-console-visible", "false")
  const absent = await page.locator(".world-canvas").screenshot()
  await testInfo.attach("headed-2fort-bots-removed", { body: absent, contentType: "image/png" })
  const pixels = await page.evaluate(async ({ before, after }) => {
    const decode = async (encoded: string): Promise<ImageData> => {
      const image = new Image()
      image.src = `data:image/png;base64,${encoded}`
      await image.decode()
      const canvas = document.createElement("canvas")
      canvas.width = image.width
      canvas.height = image.height
      const context = canvas.getContext("2d")!
      context.drawImage(image, 0, 0)
      return context.getImageData(0, 0, image.width, image.height)
    }
    const [left, right] = await Promise.all([decode(before), decode(after)])
    let changed = 0
    let playerColored = 0
    for (let y = 100; y < left.height - 100; y += 1) {
      for (let x = 320; x < left.width - 320; x += 1) {
        const index = (y * left.width + x) * 4
        const delta = Math.abs(left.data[index]! - right.data[index]!)
          + Math.abs(left.data[index + 1]! - right.data[index + 1]!)
          + Math.abs(left.data[index + 2]! - right.data[index + 2]!)
        if (delta > 36) {
          changed += 1
          if (Math.abs(left.data[index]! - left.data[index + 2]!) > 8) playerColored += 1
        }
      }
    }
    return { changed, playerColored }
  }, { before: visible.toString("base64"), after: absent.toString("base64") })
  expect(pixels.changed).toBeGreaterThan(500)
  expect(pixels.playerColored).toBeGreaterThan(100)

  const report = {
    schema: "playsrc-tf2-headed-2fort-bots-v1",
    headed: true,
    target: "ctf_2fort",
    sourceNavigation: {
      sha256: "6c1e5b37b3cffb9ad97c554aa9e104119a5c5fb38bd6c9d2903a4d405f609017",
      byteLength: 307_701,
      areas: 1_128,
      connections: 4_233,
      hidingSpots: 385,
      visibilityReferences: 32_700,
    },
    classes,
    initial,
    engagement,
    bots: measurement.bots,
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
    visiblePlayerPixels: pixels,
  }
  const { sourceCacheDir } = await loadLocalConfig()
  const directory = path.join(sourceCacheDir, "evidence", "tf2-2fort-bots")
  await mkdir(directory, { recursive: true })
  await Promise.all([
    writeFile(path.join(directory, "ctf_2fort-bots.json"), `${JSON.stringify(report, null, 2)}\n`),
    writeFile(path.join(directory, "ctf_2fort-bot-visible.png"), visible),
    writeFile(path.join(directory, "ctf_2fort-bots-removed.png"), absent),
  ])
  console.log(`[2fort-bots] ${JSON.stringify(report)}`)
})
