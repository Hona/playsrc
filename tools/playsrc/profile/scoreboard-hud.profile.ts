import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
import { expect, test } from "./application-test"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"

const classes = Object.freeze([
  { name: "scout", identity: 1, team: "red" },
  { name: "sniper", identity: 2, team: "blue" },
  { name: "soldier", identity: 3, team: "red" },
  { name: "demoman", identity: 4, team: "blue" },
  { name: "medic", identity: 5, team: "red" },
  { name: "heavy", identity: 6, team: "blue" },
  { name: "pyro", identity: 7, team: "red" },
  { name: "spy", identity: 8, team: "blue" },
  { name: "engineer", identity: 9, team: "blue" },
] as const)

test("authored TF2 scoreboard joins live teams, all classes, bots, spectators and CTF", async ({ page }, testInfo) => {
  await page.addInitScript(() => {
    ;(globalThis as typeof globalThis & { __playsrcProfile?: object }).__playsrcProfile = {}
  })
  const main = page.locator("main")
  await page.goto("/")
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 120_000 })
  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  const command = async (value: string) => {
    await entry.fill(value)
    await entry.press("Enter")
  }
  await command("map pl_upward")
  await expect.poll(async () => ({ phase: await main.getAttribute("data-phase"), team: await main.getAttribute("data-team-selection-visible") }), { timeout: 600_000 })
    .toMatchObject({ team: "true" })
  await command("jointeam red")
  await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 600_000 })
  if (await main.getAttribute("data-class-selection-visible") === "true") {
    await command("joinclass soldier")
    await expect(main).toHaveAttribute("data-class-selection-visible", "false")
  }
  await expect.poll(async () => await main.getAttribute("data-scoreboard-probe"), { timeout: 30_000 }).not.toBeNull()
  const initial = JSON.parse(await main.getAttribute("data-scoreboard-probe") ?? "{}")
  expect(initial).toMatchObject({ map: "pl_upward", red: { playerCount: 1 }, blue: { playerCount: 0 } })
  expect(initial.players).toMatchObject([{ name: "unnamed", team: 2, class: 3, score: 0, kills: 0, deaths: 0, ping: null }])

  await command("tf_bot_add blue sniper")
  await expect(main).toHaveAttribute("data-bot-count", "1")
  await command("tf_bot_add red medic")
  await expect(main).toHaveAttribute("data-bot-count", "2")
  await page.keyboard.press("Backquote")
  await expect(main).toHaveAttribute("data-console-visible", "false")
  const closed = await page.screenshot()
  await page.keyboard.down("Tab")
  await expect(main).toHaveAttribute("data-scoreboard-visible", "true")
  const scoreinfo = page.locator(".hud-layer [data-vgui-name='scoreinfo']")
  await expect(scoreinfo).toBeVisible()
  await expect(page.locator(".hud-layer [data-vgui-name='RedPlayerList'] [data-vgui-item]")).toHaveCount(2)
  await expect(page.locator(".hud-layer [data-vgui-name='BluePlayerList'] [data-vgui-item]")).toHaveCount(1)
  await expect(page.locator(".hud-layer [data-vgui-image='../hud/leaderboard_class_medic']")).toHaveCount(1)
  await expect(page.locator(".hud-layer [data-vgui-image='../hud/leaderboard_class_sniper']")).toHaveCount(0)
  await expect(page.locator(".hud-layer [data-vgui-image='../hud/scoreboard_ping_bot_red']")).toHaveCount(1)
  await expect(page.locator(".hud-layer [data-vgui-image='../hud/scoreboard_ping_bot_blue']")).toHaveCount(1)
  const open = await page.screenshot()
  await testInfo.attach("headed-scoreboard-red-blu-bots", { body: open, contentType: "image/png" })

  const seconds = profileSampleSeconds()
  const performance = await page.evaluate(async (sampleSeconds) => {
    const root = document.querySelector<HTMLElement>("main")!
    const start = globalThis.performance.now()
    const firstTick = Number(root.dataset.snapshotTick)
    let prior = start
    const frames: number[] = []
    await new Promise<void>((resolve) => {
      const sample = (now: number) => {
        frames.push(now - prior)
        prior = now
        if (now - start >= sampleSeconds * 1000) resolve()
        else requestAnimationFrame(sample)
      }
      requestAnimationFrame(sample)
    })
    return {
      seconds: (globalThis.performance.now() - start) / 1000,
      firstTick,
      lastTick: Number(root.dataset.snapshotTick),
      frames,
    }
  }, seconds)
  expect(performance.lastTick - performance.firstTick).toBeGreaterThan(seconds * 55)
  await page.keyboard.up("Tab")
  await expect(main).toHaveAttribute("data-scoreboard-visible", "false")
  await expect(scoreinfo).toBeHidden()

  await page.keyboard.press("Backquote")
  await command("tf_bot_kick all")
  await expect(main).toHaveAttribute("data-bot-count", "0")
  for (const [index, playerClass] of classes.entries()) {
    await command(`tf_bot_add ${playerClass.team} ${playerClass.name}`)
    await expect(main).toHaveAttribute("data-bot-count", String(index + 1))
  }
  const full = JSON.parse(await main.getAttribute("data-scoreboard-probe") ?? "{}")
  expect(full.red.playerCount).toBe(5)
  expect(full.blue.playerCount).toBe(5)
  expect(full.players).toHaveLength(10)
  expect(full.players.filter((player: { ping: unknown }) => player.ping === "bot")).toHaveLength(9)
  expect(full.players.filter((player: { team: number }) => player.team === 3)
    .every((player: { class: number | null }) => player.class === null)).toBe(true)

  await command("jointeam spectate")
  await expect.poll(async () => JSON.parse(await main.getAttribute("data-scoreboard-probe") ?? "{}").spectators).toEqual(["unnamed"])
  await page.keyboard.press("Backquote")
  await page.keyboard.down("Tab")
  await expect(main).toHaveAttribute("data-scoreboard-visible", "true")
  const spectator = JSON.parse(await main.getAttribute("data-scoreboard-probe") ?? "{}")
  expect(spectator.red.playerCount).toBe(4)
  expect(spectator.blue.playerCount).toBe(5)
  expect(spectator.players.map((player: { class: number }) => player.class).toSorted((left: number, right: number) => left - right))
    .toEqual(classes.map((playerClass) => playerClass.identity))
  await expect(page.locator(".hud-layer [data-vgui-name='Spectators']")).toContainText("1 spectator: unnamed")
  for (const image of ["scout", "sniper", "soldier", "demo", "medic", "heavy", "pyro", "spy", "engineer"]) {
    await expect(page.locator(`.hud-layer [data-vgui-image^='../hud/leaderboard_class_${image}']`)).toHaveCount(1)
  }
  const allClasses = await page.screenshot()
  await testInfo.attach("headed-scoreboard-spectator-nine-classes", { body: allClasses, contentType: "image/png" })
  await page.keyboard.up("Tab")
  await page.keyboard.press("Backquote")
  await command("tf_bot_kick all")
  await expect(main).toHaveAttribute("data-bot-count", "0")
  await command("map ctf_2fort")
  await expect.poll(async () => {
    const team = await main.getAttribute("data-team-selection-visible")
    const target = JSON.parse(await main.getAttribute("data-scoreboard-probe") ?? "{}").map
    return team === "true" || await main.getAttribute("data-phase") === "Ready" && target === "ctf_2fort"
  }, { timeout: 120_000 }).toBe(true)
  if (await main.getAttribute("data-team-selection-visible") === "true") await command("jointeam red")
  await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 120_000 })
  if (await main.getAttribute("data-class-selection-visible") === "true") {
    await command("joinclass soldier")
    await expect(main).toHaveAttribute("data-class-selection-visible", "false")
  }
  await expect.poll(async () => JSON.parse(await main.getAttribute("data-scoreboard-probe") ?? "{}").map).toBe("ctf_2fort")
  await page.keyboard.press("Backquote")
  await page.keyboard.down("Tab")
  await expect(scoreinfo).toBeVisible()
  await expect(page.locator(".hud-layer [data-vgui-name='mapname']")).toHaveText("ctf_2fort")
  const ctf = await page.screenshot()
  await testInfo.attach("headed-scoreboard-ctf-2fort", { body: ctf, contentType: "image/png" })
  await page.keyboard.up("Tab")

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
    const [closedImage, openImage] = await Promise.all([decode(before), decode(after)])
    let changed = 0
    let redHeader = 0
    let blueHeader = 0
    for (let y = 60; y < 650; y += 1) {
      for (let x = 150; x < 1120; x += 1) {
        const index = (y * openImage.width + x) * 4
        const red = openImage.data[index]!
        const green = openImage.data[index + 1]!
        const blue = openImage.data[index + 2]!
        const delta = Math.abs(red - closedImage.data[index]!)
          + Math.abs(green - closedImage.data[index + 1]!)
          + Math.abs(blue - closedImage.data[index + 2]!)
        if (delta > 40) changed += 1
        if (y < 170 && red > blue + 15 && red > green + 8) redHeader += 1
        if (y < 170 && blue > red + 8) blueHeader += 1
      }
    }
    return { changed, redHeader, blueHeader }
  }, { before: closed.toString("base64"), after: open.toString("base64") })
  expect(pixels.changed).toBeGreaterThan(20_000)
  expect(pixels.redHeader).toBeGreaterThan(500)
  expect(pixels.blueHeader).toBeGreaterThan(500)
  const report = Object.freeze({
    schema: "playsrc-tf2-headed-scoreboard-hud-v1",
    headed: true,
    targets: ["pl_upward", "ctf_2fort"],
    classes: classes.map((playerClass) => playerClass.identity),
    spectator: spectator.spectators,
    pixels,
    performance: {
      seconds: performance.seconds,
      ticks: performance.lastTick - performance.firstTick,
      ...summarizeFrameTimes(performance.frames),
    },
  })
  const config = await loadLocalConfig()
  const directory = path.join(config.sourceCacheDir, "profiles", "scoreboard")
  await mkdir(directory, { recursive: true })
  await Promise.all([
    writeFile(path.join(directory, "scoreboard.json"), `${JSON.stringify(report, null, 2)}\n`),
    writeFile(path.join(directory, "red-blu-bots.png"), open),
    writeFile(path.join(directory, "spectator-nine-classes.png"), allClasses),
    writeFile(path.join(directory, "ctf-2fort.png"), ctf),
  ])
  console.log(JSON.stringify(report))
})
