import { test, expect } from "./application-test"
import { chooseTf2Team } from "./team-selection-evidence"
import { headedProfileTarget } from "./profile-target"
import { summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"
import { readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { tf2MapBsp, tf2MapMode } from "@playsrc/game-tf2-browser/maps"
import { loadLocalConfig } from "../src/config"

test("configured map native traversal, objective roster, visible geometry and cadence", async ({ page }, testInfo) => {
  const target = headedProfileTarget(process.env, "cp_badlands")
  const config = await loadLocalConfig()
  const facts = JSON.parse(await readFile(path.join(config.sourceCacheDir, "evidence/map-runtime", `${target}.facts.json`), "utf8"))
  expect(facts.bspSha256).toBe(tf2MapBsp(target).sha256)
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = {} })
  const main = page.locator("main")
  const errors: string[] = []
  page.on("pageerror", error => errors.push(error.message))
  const command = async (value: string) => {
    if (await main.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    const entry = page.locator("[aria-label='Console command']")
    await entry.fill(value); await entry.press("Enter")
  }
  const closeConsole = async () => { if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote") }
  let revision = 0
  const spawnChecks: unknown[] = []
  const waitPlayer = async (field: "team" | "class", value: number) => {
    try {
      await page.waitForFunction(({ field, value }) => (globalThis as any).__playsrcProfile.player?.[field] === value, { field, value }, { timeout: 5000 })
    } catch (error) {
      const state = await page.evaluate(() => ({ player: (globalThis as any).__playsrcProfile.player, dataset: { ...document.querySelector<HTMLElement>("main")!.dataset }, console: document.querySelector<HTMLElement>("[aria-label='Console output']")?.innerText }))
      await writeFile(testInfo.outputPath(`${target}-spawn-failure.json`), JSON.stringify(state))
      throw error
    }
  }
  const checkSpawn = async (team: number, label: string) => {
    await waitPlayer("team", team)
    const player = await page.evaluate(() => (globalThis as any).__playsrcProfile.player)
    const candidates = facts.spawns.filter((spawn: any) => Number(spawn.team) === team && Math.hypot(spawn.position[0] - player.position[0], spawn.position[1] - player.position[1]) < 1 && Math.abs(spawn.position[2] - player.position[2]) < 128)
    expect(candidates.length, `${target}:${label} authored spawn position`).toBeGreaterThan(0)
    const yawError = (left: number, right: number) => Math.abs(((left - right + 540) % 360) - 180)
    expect(candidates.some((spawn: any) => yawError(player.camera.yawDegrees, spawn.angles[1]) < 0.001 && Math.abs(player.camera.pitchDegrees - spawn.angles[0]) < 0.001), `${target}:${label} authored spawn angles`).toBe(true)
    spawnChecks.push({ label, player, candidates })
  }
  const capture = async (label: string) => {
    await closeConsole()
    const selected = ++revision
    await page.evaluate(revision => { (globalThis as any).__playsrcProfile.geometryEvidenceRevision = revision }, selected)
    await page.waitForFunction(revision => (globalThis as any).__playsrcProfile.geometryEvidence?.revision === revision, selected)
    const geometry = await page.evaluate(() => (globalThis as any).__playsrcProfile.geometryEvidence)
    const imagePath = testInfo.outputPath(`${target}-${label}.png`)
    await page.screenshot({ path: imagePath })
    const image = decodeScreenshot(await page.locator("canvas.world-canvas").screenshot({ path: testInfo.outputPath(`${target}-${label}-world.png`) }))
    const depth = geometry.geometry.samples.filter((sample: any) => sample.family !== null && Number.isFinite(sample.depth) && sample.depth > 0).map((sample: any) => {
      const x = Math.max(0, Math.min(image.width - 1, Math.round((sample.x + 1) * image.width / 2)))
      const y = Math.max(0, Math.min(image.height - 1, Math.round((1 - sample.y) * image.height / 2)))
      const offset = (y * image.width + x) * image.channels
      return { ...sample, rgb: [...image.pixels.subarray(offset, offset + 3)] }
    })
    expect(depth.some((sample: any) => sample.disposition === "main-world" && sample.rgb.some((channel: number) => channel > 3))).toBe(true)
    const facts = await page.evaluate(() => ({ points: (globalThis as any).__playsrcProfile.controlPoints, bots: (globalThis as any).__playsrcProfile.bots, round: (globalThis as any).__playsrcProfile.round }))
    const dataPath = testInfo.outputPath(`${target}-${label}.json`)
    await writeFile(dataPath, JSON.stringify({ geometry, depth, facts }))
    await testInfo.attach(label, { path: imagePath, contentType: "image/png" })
    await testInfo.attach(`${label}-depth`, { path: dataPath, contentType: "application/json" })
  }
  await page.goto("/")
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 60_000 })
  await command(`map ${target}`)
  await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 60_000 })
  await closeConsole(); await chooseTf2Team(page, "red")
  await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 30_000 })
  await checkSpawn(2, "red-initial")
  await command("joinclass scout"); await closeConsole()
  await waitPlayer("class", 1)
  await checkSpawn(2, "red-class-respawn")
  await command("joinclass soldier"); await closeConsole()
  await waitPlayer("class", 3)
  await command("jointeam blue")
  await command("joinclass soldier"); await closeConsole()
  await checkSpawn(3, "blue-join")
  await command("joinclass scout"); await closeConsole()
  await waitPlayer("class", 1)
  await checkSpawn(3, "blue-class-respawn")
  await capture("blue-spawn")
  await command("jointeam red")
  await command("joinclass soldier"); await closeConsole()
  await waitPlayer("class", 3)
  await checkSpawn(2, "red-return")
  if (process.env.PROFILE_SPAWN_ONLY === "1") {
    await capture("red-spawn")
    await writeFile(testInfo.outputPath(`${target}-spawn-checks.json`), JSON.stringify({ target, spawnChecks }))
    return
  }
  await command("tf_bot_quota 15"); await closeConsole()
  await expect(main).toHaveAttribute("data-bot-count", "15")
  await capture("spawn")
  const before = await main.getAttribute("data-camera-position")
  await page.locator("canvas.world-canvas").click({ force: true })
  await page.keyboard.down("w"); await page.waitForTimeout(1000); await page.keyboard.up("w")
  expect(await main.getAttribute("data-camera-position")).not.toBe(before)
  await page.waitForFunction(() => !(globalThis as any).__playsrcProfile.round.waitingForPlayers, undefined, { timeout: 40_000 })
  const points = await page.evaluate(() => (globalThis as any).__playsrcProfile.controlPoints.points.map((point: any) => ({ identity: point.identity, position: point.position, owner: point.owner })))
  expect(points).toHaveLength(tf2MapMode(target) === "king-of-the-hill" ? 1 : 5)
  if (tf2MapMode(target) === "king-of-the-hill") await command("ent_fire team_control_point SetUnlockTime 1")
  const point = points.find((point: any) => point.owner === 0) ?? points[Math.floor(points.length / 2)]
  await command(`setpos ${point.position[0]} ${point.position[1]} ${point.position[2] + 8}`)
  await closeConsole()
  const sample = await page.evaluate(async () => {
    const root = document.querySelector<HTMLElement>("main")!, profile = (globalThis as any).__playsrcProfile
    const start = performance.now(), tick = Number(root.dataset.snapshotTick)
    const before = profile.bots.map((bot: any) => ({ identity: bot.identity, area: bot.area, position: bot.position }))
    const frames: number[] = []; let previous = start
    await new Promise<void>(resolve => {
      const frame = (now: number) => { frames.push(now - previous); previous = now; if (now - start >= 5000) resolve(); else requestAnimationFrame(frame) }
      requestAnimationFrame(frame)
    })
    return { seconds: (performance.now() - start) / 1000, ticks: Number(root.dataset.snapshotTick) - tick, frames, before, bots: profile.bots, points: profile.controlPoints.points }
  })
  expect(sample.bots).toHaveLength(15)
  expect(sample.bots.every((bot: any) => bot.area !== null)).toBe(true)
  expect(sample.bots.some((bot: any) => sample.before.some((prior: any) => prior.identity === bot.identity && Math.hypot(...bot.position.map((value: number, axis: number) => value - prior.position[axis])) > 32))).toBe(true)
  expect(sample.ticks / sample.seconds).toBeGreaterThan(63)
  await capture("objective")
  for (const [index, point] of points.entries()) {
    await command(`setpos ${point.position[0]} ${point.position[1]} ${point.position[2] + 8}`)
    await closeConsole(); await page.waitForTimeout(300)
    await capture(`point-${index}`)
  }
  const resultPath = testInfo.outputPath(`${target}-acceptance.json`)
  await writeFile(resultPath, JSON.stringify({ target, errors, spawnChecks, ...sample, frames: summarizeFrameTimes(sample.frames) }))
  await testInfo.attach("map-acceptance", { path: resultPath, contentType: "application/json" })
  expect(errors).toEqual([])
})
