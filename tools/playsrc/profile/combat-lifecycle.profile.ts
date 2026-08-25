import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
import { expect, test } from "./application-test"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { chooseTf2Team } from "./team-selection-evidence"

test("headed pl_upward local and bot combat publishes scoreboard, authored killfeed, death and team-wave respawn", async ({ page }, testInfo) => {
  await page.addInitScript(() => { ;(globalThis as any).__playsrcProfile = {} })
  await page.goto("/")
  await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu", { timeout: 120_000 })
  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  const command = async (value: string) => { await entry.fill(value); await entry.press("Enter") }
  await command("map pl_upward")
  await page.waitForFunction(() => {
    const root = document.querySelector<HTMLElement>("main")
    return root?.dataset.teamSelectionVisible === "true" || root?.dataset.phase === "Ready"
  }, undefined, { timeout: 600_000 })
  if (await page.locator("main").getAttribute("data-team-selection-visible") === "true") {
    await page.keyboard.press("Backquote")
    await chooseTf2Team(page, "red")
  }
  await expect(page.locator("main")).toHaveAttribute("data-phase", "Ready", { timeout: 120_000 })
  if (await page.locator("main").getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
  await command("joinclass scout")
  await command("tf_bot_add red scout easy")
  await expect(page.locator("main")).toHaveAttribute("data-bot-count", "1")
  await command("jointeam blue")
  await page.keyboard.press("Backquote")
  await expect(page.locator("main")).toHaveAttribute("data-team-selection-local", "3", { timeout: 30_000 })
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "true")
  await page.keyboard.press("Digit1")
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "false")
  await expect.poll(async () => page.evaluate(() => (globalThis as any).__playsrcProfile.combat?.scores?.map((score: any) => score.team))).toEqual([3, 2])
  await page.keyboard.down("Tab")
  const scoreboard = page.locator("[data-vgui-name='scoreinfo']")
  await expect(scoreboard).toBeVisible()
  const scoreboardPixels = await page.screenshot()
  await testInfo.attach("headed-team-scoreboard", { body: scoreboardPixels, contentType: "image/png" })
  await page.keyboard.up("Tab")
  await expect(scoreboard).toBeHidden()
  const initialBot = await page.evaluate(() => (globalThis as any).__playsrcProfile.bots[0])
  await page.keyboard.press("Backquote")
  await command(`setpos ${initialBot.position[0] - 55} ${initialBot.position[1]} ${initialBot.position[2]}`)
  await expect.poll(async () => page.evaluate(() => {
    const profile = (globalThis as any).__playsrcProfile
    return Math.hypot(profile.bots[0].position[0] - profile.displacementCamera.position[0],
      profile.bots[0].position[1] - profile.displacementCamera.position[1])
  }), { timeout: 10_000 }).toBeLessThan(160)
  await page.keyboard.press("Backquote")

  await page.bringToFront()
  await page.locator("canvas.world-canvas").click({ position: { x: 640, y: 360 } })
  await expect(page.locator("main")).toHaveAttribute("data-pointer-locked", "true", { timeout: 15_000 })
  await page.evaluate(async () => {
    const canvas = document.querySelector<HTMLCanvasElement>("canvas.world-canvas")!
    if (document.pointerLockElement !== canvas) throw new Error("headed gameplay lost native pointer lock")
    ;(globalThis as any).__combatFiring = false
    ;(globalThis as any).__combatAim = setInterval(() => {
      const profile = (globalThis as any).__playsrcProfile
      if (!(globalThis as any).__combatDeath && profile.bots?.[0]?.lifecycle === 2 && profile.combat?.scores?.[0]?.kills === 1) {
        ;(globalThis as any).__combatDeath = JSON.parse(JSON.stringify({ bot: profile.bots[0], scores: profile.combat.scores }))
      }
      const bot = profile.bots?.find((value: any) => value.lifecycle === 1)
      const camera = profile.displacementCamera
      if (!bot || !camera) return
      const delta = [bot.position[0] - camera.position[0], bot.position[1] - camera.position[1], bot.position[2] + 53 - camera.position[2]]
      const yaw = Math.atan2(delta[1], delta[0]) * 180 / Math.PI
      const pitch = -Math.atan2(delta[2], Math.hypot(delta[0], delta[1])) * 180 / Math.PI
      const yawDelta = ((yaw - camera.yawDegrees + 540) % 360) - 180
      const event = new MouseEvent("mousemove", { bubbles: true })
      Object.defineProperty(event, "movementX", { value: -yawDelta * 0.35 / 0.066 })
      Object.defineProperty(event, "movementY", { value: (pitch - camera.pitchDegrees) * 0.35 / 0.066 })
      dispatchEvent(event)
      const distance = Math.hypot(...delta)
      if (distance < 180 && !(globalThis as any).__combatFiring) {
        ;(globalThis as any).__combatFiring = true
        dispatchEvent(new MouseEvent("mousedown", { button: 0, bubbles: true }))
      }
    }, 40)
  })
  await expect.poll(async () => page.evaluate(() => {
    const profile = (globalThis as any).__playsrcProfile
    const root = document.querySelector<HTMLElement>("main")!
    return JSON.stringify({ kills: (globalThis as any).__combatDeath?.scores?.[0]?.kills ?? 0, bot: profile.bots?.[0],
      camera: profile.displacementCamera, health: profile.combat?.health,
      ammo: root.dataset.weaponTrace, firing: (globalThis as any).__combatFiring })
  }), { timeout: 30_000 }).toMatch(/^\{"kills":1,/u)
  await page.evaluate(() => {
    dispatchEvent(new MouseEvent("mouseup", { button: 0, bubbles: true }))
    dispatchEvent(new KeyboardEvent("keyup", { code: "KeyW", key: "w", bubbles: true }))
    clearInterval((globalThis as any).__combatAim)
  })
  const killed = await page.evaluate(() => {
    const death = (globalThis as any).__combatDeath
    return { ...death, killfeed: document.querySelector("[data-vgui-name^='DeathNotice']")?.textContent ?? "" }
  })
  expect(killed.bot.lifecycle).toBe(2)
  expect(killed.bot.health).toBe(0)
  expect(killed.scores[0]).toMatchObject({ kills: 1, damage: 125, killstreak: 1 })
  expect(killed.scores[1]).toMatchObject({ deaths: 1 })
  expect(killed.scores[1].respawnTick).not.toBeNull()
  expect(killed.killfeed).toContain("Bot")
  const killfeed = page.locator("[data-vgui-name^='DeathNotice']")
  await expect(killfeed).toBeVisible()
  const noticePixels = await killfeed.screenshot()
  expect(noticePixels.byteLength).toBeGreaterThan(250)
  await testInfo.attach("headed-visible-killfeed", { body: noticePixels, contentType: "image/png" })

  const seconds = profileSampleSeconds()
  const measurement = await page.evaluate(async (duration) => {
    const root = document.querySelector<HTMLElement>("main")!
    const started = performance.now(), firstTick = Number(root.dataset.snapshotTick)
    let prior = started
    const frames: number[] = []
    await new Promise<void>(resolve => {
      const frame = (now: number) => { frames.push(now - prior); prior = now; now - started >= duration * 1000 ? resolve() : requestAnimationFrame(frame) }
      requestAnimationFrame(frame)
    })
    return { seconds: (performance.now() - started) / 1000, firstTick, lastTick: Number(root.dataset.snapshotTick), frames }
  }, seconds)
  expect(measurement.lastTick - measurement.firstTick).toBeGreaterThan(seconds * 60)
  await expect.poll(async () => page.evaluate(() => (globalThis as any).__playsrcProfile.bots?.[0]?.lifecycle), { timeout: 30_000 }).toBe(1)
  const respawned = await page.evaluate(() => ({ bot: (globalThis as any).__playsrcProfile.bots[0], scores: (globalThis as any).__playsrcProfile.combat.scores }))
  expect(respawned.bot.health).toBe(125)
  expect(respawned.scores[1]).toMatchObject({ deaths: 1, respawnTick: null })

  const report = { schema: "playsrc-tf2-headed-combat-lifecycle-v1", headed: true, target: "pl_upward",
    killed, respawned, simulation: { seconds: measurement.seconds, firstTick: measurement.firstTick,
      lastTick: measurement.lastTick, ticksPerSecond: (measurement.lastTick - measurement.firstTick) / measurement.seconds },
    frames: summarizeFrameTimes(measurement.frames) }
  const local = await loadLocalConfig()
  const directory = path.join(local.sourceCacheDir, "evidence", "tf2-combat-lifecycle")
  await mkdir(directory, { recursive: true })
  await writeFile(path.join(directory, "pl_upward-combat.json"), `${JSON.stringify(report, null, 2)}\n`)
  console.log(`[combat-lifecycle] ${JSON.stringify(report)}`)
})
