import { expect, test } from "./application-test"
import { chooseTf2Team } from "./team-selection-evidence"
import { summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"
import { writeFile } from "node:fs/promises"
import { Tf2BrowserAutomation } from "../../../apps/web/tf2/src/browser-automation"
import { tf2MapBsp } from "@playsrc/game-tf2-browser/maps"

test("headed Viaduct local KOTH capture, contest, overtime, victory and restart with independent team clocks", async ({ page }, testInfo) => {
  const performanceOnly = process.env.PROFILE_KOTH_PERFORMANCE === "1"
  const skyVisualOnly = process.env.PROFILE_KOTH_SKY_VISUAL === "1"
  let bspRequests = 0
  const bspHash = tf2MapBsp("koth_viaduct").sha256
  page.on("request", request => { if (request.url().includes(bspHash)) bspRequests++ })
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = {} })
  const main = page.locator("main")
  const automation = new Tf2BrowserAutomation({
    evaluate: async <T>(expression: string): Promise<T> => page.evaluate(expression) as Promise<T>,
    press: key => page.keyboard.press(key), click: selector => page.locator(selector).click(),
    focus: selector => page.locator(selector).focus(), fill: (selector, value) => page.locator(selector).fill(value),
    waitFor: async (expression, timeout) => { await page.waitForFunction(expression, undefined, { timeout }) },
    activateCurrentTab: () => page.bringToFront(),
  })
  await page.goto("/")
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 60_000 })
  const command = async (text: string) => {
    if (await main.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    const entry = page.locator("[aria-label='Console command']")
    await entry.fill(text); await entry.press("Enter")
  }
  const closeConsole = async () => { if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote") }
  if (process.env.PROFILE_KOTH_TRAINING === "1") {
    const layer = page.locator(".local-match-layer")
    await page.locator(".gameui-layer [data-vgui-name='FindAGameButton']").click()
    await page.locator(".gameui-layer [data-vgui-name='TrainingEntry'] [data-vgui-name='ModeButton']").click()
    await layer.locator("[data-vgui-name='OfflinePracticePanel'] [data-vgui-name='StartButton']").click()
    await layer.locator("[data-vgui-name='OfflinePractice_ModeSelectionPanel'] [data-vgui-name='NextButton']").click()
    await expect(layer.locator("[data-vgui-name='GameModeLabel']")).toHaveText("King of the Hill")
    await layer.locator("[data-vgui-name='SelectCurrentGameModeButton']").click()
    await expect(layer.locator("[data-vgui-name='MapNameLabel']")).toHaveText("Viaduct")
    await layer.locator("[data-vgui-name='OfflinePractice_MapSelectionPanel'] [data-vgui-name='DifficultyComboBox']").click()
    await page.getByRole("option", { name: "Normal", exact: true }).click()
    await layer.locator("[data-vgui-name='OfflinePractice_MapSelectionPanel'] [data-vgui-name='NumPlayersTextEntry']").fill("16")
    const path = testInfo.outputPath("headed-koth-authored-training.png")
    await page.screenshot({ path })
    await testInfo.attach("headed-koth-authored-training", { path, contentType: "image/png" })
    await layer.locator("[data-vgui-name='StartOfflinePracticeButton']").click()
  } else {
    await command("map koth_viaduct")
  }
  await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 60_000 })
  await closeConsole()
  await chooseTf2Team(page, "red")
  await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 30_000 })
  const loading = JSON.parse(await main.getAttribute("data-load-performance") ?? "null")
  expect(loading?.totalMilliseconds).toBeGreaterThan(0)
  if (skyVisualOnly) {
    await command("tf_bot_quota 0")
    await command("setpos -1710 0 230")
    await closeConsole()
    await page.waitForTimeout(5000)
    await page.evaluate(() => { (globalThis as any).__playsrcProfile.geometryEvidenceRevision = 1 })
    await page.waitForFunction(() => (globalThis as any).__playsrcProfile.geometryEvidence?.revision === 1)
    const geometry = await page.evaluate(() => (globalThis as any).__playsrcProfile.geometryEvidence)
    const imagePath = testInfo.outputPath("headed-viaduct-authored-sky.png")
    const bytes = await page.screenshot({ path: imagePath })
    const image = decodeScreenshot(bytes)
    const colors = new Set<number>()
    for (let y = 40; y < 125; y++) for (let x = 970; x < 1240; x++) {
      const offset = (y * image.width + x) * image.channels
      colors.add((image.pixels[offset]! << 16) | (image.pixels[offset + 1]! << 8) | image.pixels[offset + 2]!)
    }
    await testInfo.attach("headed-viaduct-authored-sky", { path: imagePath, contentType: "image/png" })
    const evidencePath = testInfo.outputPath("viaduct-sky-pixels-and-depth.json")
    await writeFile(evidencePath, JSON.stringify({ colors: colors.size, bounds: [970, 40, 1240, 125], geometry }))
    await testInfo.attach("viaduct-sky-pixels-and-depth", { path: evidencePath, contentType: "application/json" })
    expect(colors.size).toBeGreaterThan(16)
    return
  }
  await command("tf_bot_quota 15")
  await command("setpos -1800 0 230")
  await closeConsole()
  await expect(main).toHaveAttribute("data-bot-count", "15")

  let geometryRevision = 0
  const pixels = async (name: string) => {
    await closeConsole()
    const movement = await page.evaluate(() => {
      const d = document.querySelector<HTMLElement>("main")!.dataset
      const [x, y, z] = d.cameraPosition!.split(",").map(Number)
      const yaw = Math.atan2(-y!, -1536 - x!) * 180 / Math.PI
      const pitch = Math.atan2(z! - 276, Math.hypot(-1536 - x!, y!)) * 180 / Math.PI
      const wrap = (value: number) => ((value + 180) % 360 + 360) % 360 - 180
      return { x: wrap(Number(d.cameraYaw) - yaw) / 0.066, y: (pitch - Number(d.cameraPitch)) / 0.066 }
    })
    if (Math.abs(movement.x) + Math.abs(movement.y) > 0.001) await automation.player.lookBy(movement)
    const revision = ++geometryRevision
    await page.evaluate(revision => { (globalThis as any).__playsrcProfile.geometryEvidenceRevision = revision }, revision)
    await page.waitForFunction(revision => (globalThis as any).__playsrcProfile.geometryEvidence?.revision === revision, revision, { timeout: 5_000 })
    const geometry = await page.evaluate(() => (globalThis as any).__playsrcProfile.geometryEvidence)
    const worldPixels = decodeScreenshot(await page.locator("canvas.world-canvas").screenshot({ path: testInfo.outputPath(`${name}-world.png`) }))
    const depthPixels = geometry.geometry.samples.filter((sample: any) => sample.family !== null && Number.isFinite(sample.depth) && sample.depth > 0).map((sample: any) => {
      const x = Math.max(0, Math.min(worldPixels.width - 1, Math.round((sample.x + 1) * worldPixels.width / 2)))
      const y = Math.max(0, Math.min(worldPixels.height - 1, Math.round((1 - sample.y) * worldPixels.height / 2)))
      const offset = (y * worldPixels.width + x) * worldPixels.channels
      return { family: sample.family, disposition: sample.disposition, depth: sample.depth, rgb: [...worldPixels.pixels.subarray(offset, offset + 3)] }
    })
    expect(depthPixels.some((sample: any) => sample.disposition === "main-world" && sample.rgb.some((channel: number) => channel > 3))).toBe(true)
    const imagePath = testInfo.outputPath(`${name}.png`)
    const bytes = await page.screenshot({ path: imagePath })
    const image = decodeScreenshot(bytes)
    let visible = 0
    for (let i = 0; i < image.pixels.length; i += image.channels) if (image.pixels[i]! + image.pixels[i + 1]! + image.pixels[i + 2]! > 72) visible++
    expect(visible).toBeGreaterThan(20_000)
    await testInfo.attach(name, { path: imagePath, contentType: "image/png" })
    const depthPath = testInfo.outputPath(`${name}-depth.json`)
    await writeFile(depthPath, JSON.stringify({ camera: geometry.camera, identities: geometry.identities, depthPixels }))
    await testInfo.attach(`${name}-depth`, { path: depthPath, contentType: "application/json" })
  }
  const sample = async (bots: number) => {
    await expect(main).toHaveAttribute("data-bot-count", String(bots))
    await closeConsole()
    await pixels(`headed-koth-${bots}-bots-sample-start`)
    const result = await page.evaluate(async () => {
      const root = document.querySelector<HTMLElement>("main")!
      const tick = Number(root.dataset.snapshotTick), start = performance.now()
      const initialBots = (globalThis as any).__playsrcProfile.bots.map((bot: any) => ({ identity: bot.identity, position: bot.position, shots: bot.shots }))
      const initialCamera = root.dataset.cameraPosition
      let previous = start
      const frames: number[] = []
      await new Promise<void>(resolve => {
        const frame = (now: number) => { frames.push(now - previous); previous = now; if (now - start >= 5000) resolve(); else requestAnimationFrame(frame) }
        requestAnimationFrame(frame)
      })
      return { elapsed: (performance.now() - start) / 1000, ticks: Number(root.dataset.snapshotTick) - tick, frames, quality: (globalThis as any).__playsrcProfile.videoQuality,
        initialBots, initialCamera, finalCamera: root.dataset.cameraPosition, point: (globalThis as any).__playsrcProfile.controlPoints.points[0],
        bots: (globalThis as any).__playsrcProfile.bots.map((bot: any) => ({ identity: bot.identity, team: bot.team, objective: bot.objective, area: bot.area, position: bot.position, shots: bot.shots })) }
    })
    expect(result.ticks / result.elapsed).toBeGreaterThan(63)
    expect(result.bots).toHaveLength(bots)
    expect(result.bots.every((bot: any) => bot.area !== null)).toBe(true)
    expect(result.bots.some((bot: any) => [8, 9, 10].includes(bot.objective))).toBe(true)
    expect(result.bots.some((bot: any) => {
      const previous = result.initialBots.find((value: any) => value.identity === bot.identity)
      return previous && (bot.shots > previous.shots || Math.hypot(...bot.position.map((value: number, index: number) => value - previous.position[index])) > 1)
    })).toBe(true)
    const workers = await Promise.all(page.workers().map(worker => Promise.race([
      worker.evaluate(() => ({ heapBytes: (performance as any).memory?.usedJSHeapSize ?? null, memory: (globalThis as any).__playsrcWorkerMemory ?? null })).catch(() => null),
      new Promise<null>(resolve => setTimeout(() => resolve(null), 1000)),
    ])))
    const gameplayMemory = workers.find(worker => worker?.memory)?.memory
    expect(gameplayMemory).toBeTruthy()
    expect(gameplayMemory.linearBytes).toBeLessThan(2 * 1024 ** 3)
    expect(gameplayMemory.copiedModelSourceBytes).toBe(0)
    await pixels(`headed-koth-${bots}-bots-world-and-hud`)
    return { ...result, workers, frames: summarizeFrameTimes(result.frames) }
  }
  if (performanceOnly) {
    await page.waitForFunction(() => !(globalThis as any).__playsrcProfile.round.waitingForPlayers && (globalThis as any).__playsrcProfile.round.state === 4, undefined, { timeout: 40_000 })
    await command("ent_fire control_point_1 SetUnlockTime 1")
    await command("setpos -1800 0 230")
    const captureBot = await page.evaluate(() => {
      const profile = (globalThis as any).__playsrcProfile
      const bot = profile.bots.find((bot: any) => bot.team === 2 && bot.health > 0)
      const roster = JSON.parse(document.querySelector<HTMLElement>("main")!.dataset.scoreboardProbe!)
      const player = roster.players.find((player: any) => player.identity === bot.identity)
      return { identity: bot.identity as number, name: player.name as string, from: bot.position }
    })
    // Seed a live capture through Source's bot_teleport input, not point ownership
    // or progress counters. The unchanged bot AI then moves, captures and defends.
    await command(`bot_teleport "${captureBot.name}" -1536 0 232 0 90 0`)
    await closeConsole()
    await page.waitForFunction(() => {
      const point = (globalThis as any).__playsrcProfile.controlPoints.points[0]
      return point.owner !== 0 || point.capturingTeam !== 0
    }, undefined, { timeout: 10_000 })
    const samples = [await sample(15)]
    await command("tf_bot_quota 23")
    samples.push(await sample(23))
    expect(bspRequests).toBeLessThanOrEqual(1)
    const reportPath = testInfo.outputPath("koth-source-clock-samples.json")
    await writeFile(reportPath, JSON.stringify({ schema: "playsrc-koth-headed-v1", bspRequests, loading, captureBot, samples }))
    await testInfo.attach("koth-source-clock-samples", { path: reportPath, contentType: "application/json" })
    return
  }
  await pixels("headed-koth-fifteen-bot-local-world")
  // Deterministic gameplay below uses real brush contact and a real opposing bot.
  // Entity inputs shorten match duration, not clocks, tick rate, or capture time.
  await command("tf_bot_kick all")
  await expect(main).toHaveAttribute("data-bot-count", "0")
  await page.waitForFunction(() => !(globalThis as any).__playsrcProfile.round.waitingForPlayers && (globalThis as any).__playsrcProfile.round.state === 4, undefined, { timeout: 40_000 })
  await command("ent_fire control_point_1 SetUnlockTime 1")
  await command("setpos -1710 0 230")
  await closeConsole()
  const timerRoot = page.locator(".hud-layer [data-vgui-name='HudKothTimeStatus']")
  await expect(timerRoot).toBeVisible()
  await page.waitForFunction(() => !(globalThis as any).__playsrcProfile.round.kothTimers[0].paused, undefined, { timeout: 20_000 })
  await expect(timerRoot.locator("[data-vgui-name='BlueTimer'] [data-vgui-name='TimePanelValue']")).toHaveText("3:00")
  await pixels("headed-koth-red-captured")
  await command("tf_bot_add 1 blue soldier hard")
  await expect(main).toHaveAttribute("data-bot-count", "1")
  const enemy = await page.evaluate(() => (globalThis as any).__playsrcProfile.bots.find((bot: any) => bot.team === 3).identity as number)
  const scoreboard = JSON.parse(await main.getAttribute("data-scoreboard-probe") ?? "{}")
  const enemyName = scoreboard.players.find((player: any) => player.identity === enemy).name as string
  await command("setpos -1800 0 230")
  await command(`bot_teleport "${enemyName}" -1536 0 230 0 90 0`)
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.controlPoints.points[0].capturingTeam === 3 && (globalThis as any).__playsrcProfile.controlPoints.points[0].progress > 0, undefined, { timeout: 5_000 })
  await command("ent_fire tf_logic_koth SetRedTimer 3")
  await command("setpos -1710 0 230")
  await closeConsole()
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.round.inOvertime, undefined, { timeout: 10_000 })
  await expect(timerRoot.locator("[data-vgui-name='RedTimer'] [data-vgui-name='OvertimeLabel']")).toBeVisible()
  await pixels("headed-koth-live-brush-contest-overtime")
  await command(`bot_whack "${enemyName}"`)
  await closeConsole()
  await expect(page.locator(".hud-layer [data-vgui-name='WinningTeamLabel']")).toHaveText("RED TEAM WINS!", { timeout: 10_000 })
  await expect(page.locator(".hud-layer [data-vgui-name='WinReasonLabel']")).toContainText("control points")
  await expect(page.locator(".hud-layer [data-vgui-name='DetailsLabel']")).toContainText("Winning capture:")
  await pixels("headed-koth-red-victory")
  await command("tf_bot_kick all")
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.round.state === 3, undefined, { timeout: 20_000 })
  await pixels("headed-koth-round-restart")
  await command("jointeam blue")
  await closeConsole()
  await expect(main).toHaveAttribute("data-class-selection-visible", "true")
  await page.keyboard.press("Digit2")
  await expect(main).toHaveAttribute("data-class-selection-visible", "false")
  await expect(main).toHaveAttribute("data-team-selection-local", "3")
  await command("ent_fire control_point_1 SetUnlockTime 1")
  await command("setpos -1710 0 230")
  await closeConsole()
  await page.waitForFunction(() => !(globalThis as any).__playsrcProfile.round.kothTimers[1].paused, undefined, { timeout: 25_000 })
  await expect(timerRoot.locator("[data-vgui-name='RedTimer'] [data-vgui-name='TimePanelValue']")).toHaveText("3:00")
  await pixels("headed-koth-blue-captured-after-restart")
  await command("ent_fire tf_logic_koth SetBlueTimer 1")
  await closeConsole()
  await expect(page.locator(".hud-layer [data-vgui-name='WinningTeamLabel']")).toHaveText("BLU TEAM WINS!", { timeout: 10_000 })
  await expect(page.locator(".hud-layer [data-vgui-name='WinReasonLabel']")).toContainText("control points")
  await expect(page.locator(".hud-layer [data-vgui-name='DetailsLabel']")).toContainText("Winning capture:")
  await pixels("headed-koth-blue-victory")
  const audioStarts = (await main.getAttribute("data-audio-starts") ?? "").split("|")
  for (const definition of ["Announcer.AM_CapEnabledRandom", "Announcer.Success", "Hologram.Start", "Hologram.Stop", "Game.Overtime", "Game.YourTeamWon"]) {
    expect(audioStarts.some(value => value.startsWith(`${definition}:`)), definition).toBe(true)
  }
  const audioPath = testInfo.outputPath("koth-announcer-playback.json")
  await writeFile(audioPath, JSON.stringify({ audioStarts }))
  await testInfo.attach("koth-announcer-playback", { path: audioPath, contentType: "application/json" })
})
