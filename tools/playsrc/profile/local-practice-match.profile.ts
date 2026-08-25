import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
import { expect, test } from "./application-test"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"
import { chooseTf2Team } from "./team-selection-evidence"

const BLUE_FLAG = [489.005, -3348.51, -170] as const
const RED_CAPTURE = [-500, 3366, -170] as const

test("headed Training and Create Server launch a real TF2 bot match through combat, respawn, scoring, and victory", async ({ page }, testInfo) => {
  await page.addInitScript(() => {
    ;(globalThis as typeof globalThis & { __playsrcProfile?: object }).__playsrcProfile = {}
  })
  const main = page.locator("main")
  const layer = page.locator(".local-match-layer")
  await page.goto("/")
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 120_000 })
  const openPlaylist = async () => {
    await page.locator(".gameui-layer [data-vgui-name='FindAGameButton']").click()
    await expect(page.locator(".gameui-layer [data-vgui-name='TrainingEntry'] [data-vgui-name='ModeButton']")).toBeVisible()
    await expect(page.locator(".gameui-layer [data-vgui-name='CreateServerEntry'] [data-vgui-name='ModeButton']")).toBeVisible()
  }

  await openPlaylist()
  await page.locator(".gameui-layer [data-vgui-name='TrainingEntry'] [data-vgui-name='ModeButton']").click()
  await expect(main).toHaveAttribute("data-local-match-entry", "training")
  await expect(layer.locator("[data-vgui-name='TitleLabel']")).toHaveText("SELECT A TRAINING MODE")
  await expect(layer.locator("[data-vgui-name='BasicTrainingPanel'] [data-vgui-name='StartButton']")).toBeDisabled()
  await layer.locator("[data-vgui-name='OfflinePracticePanel'] [data-vgui-name='StartButton']").click()
  await expect(layer.locator("[data-vgui-name='GameModeLabel']")).toHaveText("Control Points")
  await expect(layer.locator("[data-vgui-name='SelectCurrentGameModeButton']")).toBeDisabled()
  const next = layer.locator("[data-vgui-name='OfflinePractice_ModeSelectionPanel'] [data-vgui-name='NextButton']")
  await next.click()
  await expect(layer.locator("[data-vgui-name='GameModeLabel']")).toHaveText("King of the Hill")
  await next.click()
  await expect(layer.locator("[data-vgui-name='GameModeLabel']")).toHaveText("Payload")
  await layer.locator("[data-vgui-name='SelectCurrentGameModeButton']").click()
  await expect(layer.locator("[data-vgui-name='MapNameLabel']")).toHaveText("Upward")
  await expect(layer.locator("[data-vgui-name='SuggestedPlayerCountLabel']")).toHaveText("12 - 24 Suggested")
  await expect(layer.locator("[data-vgui-name='OfflinePractice_MapSelectionPanel'] [data-vgui-name='DifficultyComboBox']")).toHaveAttribute("aria-label", "Easy")
  const practicePixels = await page.screenshot()
  await testInfo.attach("headed-authored-offline-practice-upward", { body: practicePixels, contentType: "image/png" })
  await layer.locator("[data-vgui-name='Container'] > [data-vgui-name='CancelButton']").click()
  await expect(main).toHaveAttribute("data-local-match-visible", "false")

  await openPlaylist()
  await page.locator(".gameui-layer [data-vgui-name='CreateServerEntry'] [data-vgui-name='ModeButton']").click()
  await expect(main).toHaveAttribute("data-local-match-entry", "create-server")
  const dialog = layer.getByRole("dialog", { name: "CREATE SERVER" })
  await expect(dialog).toBeVisible()
  const map = dialog.locator("[data-vgui-name='MapList']")
  await map.click()
  await expect(page.getByRole("option", { name: "pl_upward" })).toBeVisible()
  await page.getByRole("option", { name: "ctf_2fort" }).click()
  await dialog.getByRole("tab", { name: "GAME" }).click()
  const gameplay = dialog.locator("[data-vgui-name='GameplayPage']")
  const difficulty = gameplay.locator("[data-vgui-name='DifficultyComboBox']")
  await expect(difficulty).toHaveAttribute("aria-label", "Normal")
  await difficulty.click()
  await page.getByRole("option", { name: "Hard" }).click()
  await gameplay.locator("[data-vgui-name='NumPlayersTextEntry']").fill("5")
  const fill = gameplay.locator("[data-vgui-name='TeamFillComboBox']")
  await fill.click()
  await page.getByRole("option", { name: "fill" }).click()
  await expect(fill).toHaveAttribute("aria-label", "fill")
  const serverPixels = await page.screenshot()
  await testInfo.attach("headed-authored-create-server-bot-settings", { body: serverPixels, contentType: "image/png" })
  await dialog.getByRole("button", { name: "Start" }).click()
  await expect.poll(async () => ({
    phase: await main.getAttribute("data-phase"),
    team: await main.getAttribute("data-team-selection-visible"),
  }), { timeout: 600_000 }).toMatchObject({ team: "true" })
  const launch = JSON.parse(await main.getAttribute("data-local-match-settings") ?? "null")
  expect(launch).toEqual({
    entry: "create-server",
    mapIdentity: "ctf_2fort",
    configuration: {
      quota: 5,
      maximumPlayers: 32,
      mode: "fill",
      difficulty: 2,
      joinAfterPlayer: true,
      autoVacate: true,
      offlinePractice: false,
    },
  })
  await chooseTf2Team(page, "red")
  await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 120_000 })
  await expect(main).toHaveAttribute("data-bot-count", "4", { timeout: 30_000 })
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile?.bots?.length === 4)
  const initialBots = await page.evaluate(() => structuredClone((globalThis as any).__playsrcProfile.bots))
  expect(initialBots.every((bot: { difficulty: number }) => bot.difficulty === 2)).toBe(true)
  expect(initialBots.every((bot: { objective: number; area: number | null; remainingPathAreas: number }) =>
    bot.objective === 3 && bot.area !== null && bot.remainingPathAreas > 0)).toBe(true)
  const initialScoreboard = JSON.parse(await main.getAttribute("data-scoreboard-probe") ?? "{}")
  expect(initialScoreboard.map).toBe("ctf_2fort")
  expect(initialScoreboard.red.playerCount + initialScoreboard.blue.playerCount).toBe(5)
  expect(Math.abs(initialScoreboard.red.playerCount - initialScoreboard.blue.playerCount)).toBeLessThanOrEqual(1)
  await page.keyboard.down("Tab")
  await expect(main).toHaveAttribute("data-scoreboard-visible", "true")
  await expect(page.locator(".hud-layer [data-vgui-name='scoreinfo']")).toBeVisible()
  await expect(page.locator(".hud-layer [data-vgui-name='mapname']")).toHaveText("ctf_2fort")
  const scoreboardPixels = await page.screenshot()
  await testInfo.attach("headed-live-local-match-scoreboard", { body: scoreboardPixels, contentType: "image/png" })
  await page.keyboard.up("Tab")

  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  const command = async (text: string) => { await entry.fill(text); await entry.press("Enter") }
  const enemy = await page.evaluate(() => {
    const bot = (globalThis as any).__playsrcProfile.bots.find((value: any) => value.team === 3 && value.class === 9)
    if (!bot) throw new Error("authored enemy Engineer was not admitted to the local match")
    return [...bot.position] as [number, number, number]
  })
  await command(`setpos ${enemy[0]} ${enemy[1] - 60} ${enemy[2]}`)
  await expect.poll(async () => {
    const scoreboard = JSON.parse(await main.getAttribute("data-scoreboard-probe") ?? "{}")
    return scoreboard.players?.find((player: { identity: number }) => player.identity === 1)?.deaths ?? 0
  }, { timeout: 20_000 }).toBeGreaterThanOrEqual(1)
  const death = JSON.parse(await main.getAttribute("data-scoreboard-probe") ?? "{}")
  expect(death.players.some((player: { team: number; kills: number }) => player.team === 3 && player.kills >= 1)).toBe(true)
  await expect.poll(async () => {
    const scoreboard = JSON.parse(await main.getAttribute("data-scoreboard-probe") ?? "{}")
    return scoreboard.players?.find((player: { identity: number }) => player.identity === 1)?.alive ?? false
  }, { timeout: 30_000 }).toBe(true)
  const respawn = JSON.parse(await main.getAttribute("data-scoreboard-probe") ?? "{}")
  expect(respawn.players.find((player: { identity: number }) => player.identity === 1)).toMatchObject({ deaths: 1, alive: true })

  await command("noclip")
  await expect(main).toHaveAttribute("data-movement-mode", "1")
  for (let captures = 1; captures <= 3; captures += 1) {
    await command(`setpos ${BLUE_FLAG.join(" ")}`)
    await expect.poll(async () => main.getAttribute("data-ctf"), { timeout: 10_000 }).toMatch(/,3,1,1(?:,|\||$)/)
    await command(`setpos ${RED_CAPTURE.join(" ")}`)
    await expect.poll(async () => main.getAttribute("data-ctf"), { timeout: 10_000 })
      .toMatch(new RegExp(`^${captures}:0:3:${captures === 3 ? 2 : 0}:`))
  }
  await expect(page.locator(".hud-layer [data-vgui-name='WinPanel']")).toBeVisible()
  await expect(page.locator(".hud-layer [data-vgui-name='WinningTeamLabel']")).toHaveText("RED TEAM WINS!")
  const victory = JSON.parse(await main.getAttribute("data-scoreboard-probe") ?? "{}")
  expect(victory.red.score).toBeGreaterThanOrEqual(3)
  expect(victory.players.find((player: { identity: number }) => player.identity === 1)?.score).toBeGreaterThan(0)
  await page.keyboard.press("Backquote")
  const victoryPixels = await page.screenshot()
  await testInfo.attach("headed-local-bot-match-ctf-victory", { body: victoryPixels, contentType: "image/png" })

  const seconds = profileSampleSeconds()
  const performance = await page.evaluate(async (duration) => {
    const root = document.querySelector<HTMLElement>("main")!
    const started = globalThis.performance.now()
    const firstTick = Number(root.dataset.snapshotTick)
    let previous = started
    const frames: number[] = []
    const models: number[] = []
    await new Promise<void>((resolve) => {
      const frame = (now: number) => {
        frames.push(now - previous)
        previous = now
        const detail = root.dataset.performanceDetail
        if (detail) models.push(JSON.parse(detail).models)
        if (now - started >= duration * 1000) resolve()
        else requestAnimationFrame(frame)
      }
      requestAnimationFrame(frame)
    })
    return {
      seconds: (globalThis.performance.now() - started) / 1000,
      firstTick,
      lastTick: Number(root.dataset.snapshotTick),
      frames,
      models,
      bots: Number(root.dataset.botCount),
    }
  }, seconds)
  expect(performance.lastTick - performance.firstTick).toBeGreaterThan(seconds * 55)
  expect(performance.bots).toBe(4)

  const countVisible = (bytes: Buffer): number => {
    const decoded = decodeScreenshot(bytes)
    let nonBlack = 0
    for (let index = 0; index < decoded.pixels.length; index += decoded.channels) {
      if (decoded.pixels[index]! > 24 || decoded.pixels[index + 1]! > 24 || decoded.pixels[index + 2]! > 24) nonBlack += 1
    }
    return nonBlack
  }
  const pixels = {
    practice: countVisible(practicePixels),
    createServer: countVisible(serverPixels),
    scoreboard: countVisible(scoreboardPixels),
    victory: countVisible(victoryPixels),
  }
  expect(Math.min(...Object.values(pixels))).toBeGreaterThan(20_000)
  const report = {
    schema: "playsrc-tf2-headed-local-practice-match-v1",
    headed: true,
    maps: ["pl_upward", "ctf_2fort"],
    practice: { map: "pl_upward", difficulties: ["Easy", "Normal", "Hard", "Expert"], recommended: [12, 24] },
    launch,
    bots: initialBots.map((bot: { identity: number; team: number; class: number; difficulty: number; objective: number }) => ({
      identity: bot.identity, team: bot.team, class: bot.class, difficulty: bot.difficulty, objective: bot.objective,
    })),
    combat: { localDeaths: respawn.players.find((player: { identity: number }) => player.identity === 1)?.deaths, enemyKills: death.players.filter((player: { team: number; kills: number }) => player.team === 3).map((player: { kills: number }) => player.kills) },
    captures: { red: 3, blue: 0, winner: "red" },
    scoreboard: victory,
    pixels,
    simulation: {
      seconds: Number(performance.seconds.toFixed(3)),
      ticks: performance.lastTick - performance.firstTick,
      ticksPerSecond: Number(((performance.lastTick - performance.firstTick) / performance.seconds).toFixed(2)),
    },
    frames: summarizeFrameTimes(performance.frames),
    modelPreparation: summarizeFrameTimes(performance.models),
  }
  const local = await loadLocalConfig()
  const directory = path.join(local.sourceCacheDir, "profiles", "local-practice")
  await mkdir(directory, { recursive: true })
  await Promise.all([
    writeFile(path.join(directory, "local-practice-match.json"), `${JSON.stringify(report, null, 2)}\n`),
    writeFile(path.join(directory, "offline-practice-upward.png"), practicePixels),
    writeFile(path.join(directory, "create-server-ctf-bots.png"), serverPixels),
    writeFile(path.join(directory, "live-bot-scoreboard.png"), scoreboardPixels),
    writeFile(path.join(directory, "ctf-victory.png"), victoryPixels),
  ])
  console.log(`[local-practice] ${JSON.stringify(report)}`)
})
