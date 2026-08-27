import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
import { expect, test } from "./application-test"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { chooseTf2Team } from "./team-selection-evidence"
import { captureDeathNotice } from "./deathnotice-evidence"

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
  // Keep genuine notices through the respawn phases to verify stacking. This
  // is the native HUD cvar, not a change to simulation time or damage.
  await command("hud_deathnotice_time 60")
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
  const initialBot = await page.evaluate(() => (globalThis as any).__playsrcProfile.combat.scores.find((player: any) => player.identity !== 1))
  await page.keyboard.press("Backquote")
  await command("setpos -2528 -1360 17")
  await command(`bot_teleport "${initialBot.name}" -2450 -1360 17 0 0 0`)
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
  expect(killed.killfeed).toContain(initialBot.name)
  const killfeed = page.locator("[data-vgui-name^='DeathNotice']")
  await expect(killfeed).toBeVisible()
  await expect(killfeed.locator("[data-death-icon='dneg_scattergun']")).toBeVisible()
  const authored = await killfeed.evaluate(element => {
    const rect = element.getBoundingClientRect()
    const names = [...element.querySelectorAll("span")].map(span => ({ text: span.textContent, color: getComputedStyle(span).color, font: getComputedStyle(span).font }))
    return { bounds: { x: rect.x, y: rect.y, width: rect.width, height: rect.height }, names,
      background: element.querySelector("polygon")?.getAttribute("fill"), viewport: { width: innerWidth, height: innerHeight, dpr: devicePixelRatio } }
  })
  expect(authored.bounds.x + authored.bounds.width).toBe(authored.viewport.width - Math.trunc(12 * authored.viewport.height / 480))
  expect(authored.names.map(name => name.color)).toEqual(["rgb(104, 124, 155)", "rgb(180, 92, 77)"])
  expect(authored.background).toBe("rgba(245,229,196,0.7843137254901961)")
  await testInfo.attach("headed-authored-killfeed-full-view", { body: await page.screenshot(), contentType: "image/png" })
  const comparison = await captureDeathNotice(page, testInfo, "blu-kills-red")
  expect(comparison.compared).toBeGreaterThan(10)
  expect(comparison.matchedFraction).toBeGreaterThan(.95)
  const noticePixels = await killfeed.screenshot()
  expect(noticePixels.byteLength).toBeGreaterThan(250)
  await testInfo.attach("headed-visible-killfeed", { body: noticePixels, contentType: "image/png" })

  const seconds = profileSampleSeconds()
  const measurement = await page.evaluate(async (duration) => {
    const root = document.querySelector<HTMLElement>("main")!
    const started = performance.now(), firstTick = Number(root.dataset.snapshotTick)
    let hudMutations = 0
    const observer = new MutationObserver(records => { hudMutations += records.length })
    observer.observe(document.querySelector("[data-tf2-deathnotice]")!, { subtree: true, attributes: true, childList: true, characterData: true })
    let prior = started
    const frames: number[] = []
    await new Promise<void>(resolve => {
      const frame = (now: number) => { frames.push(now - prior); prior = now; now - started >= duration * 1000 ? resolve() : requestAnimationFrame(frame) }
      requestAnimationFrame(frame)
    })
    observer.disconnect()
    return { seconds: (performance.now() - started) / 1000, firstTick, lastTick: Number(root.dataset.snapshotTick), frames, hudMutations }
  }, seconds)
  expect(measurement.lastTick - measurement.firstTick).toBeGreaterThan(seconds * 60)
  expect(measurement.hudMutations).toBe(0)
  await expect.poll(async () => page.evaluate(() => (globalThis as any).__playsrcProfile.bots?.[0]?.lifecycle), { timeout: 30_000 }).toBe(1)
  const respawned = await page.evaluate(() => ({ bot: (globalThis as any).__playsrcProfile.bots[0], scores: (globalThis as any).__playsrcProfile.combat.scores }))
  expect(respawned.bot.health).toBe(125)
  expect(respawned.scores[1]).toMatchObject({ deaths: 1, respawnTick: null })

  // Reverse the real combat roles; no injected HUD events or health mutation.
  await page.keyboard.press("Backquote")
  await command("setpos -2528 -1360 17")
  await command(`bot_teleport "${initialBot.name}" -2450 -1360 17 0 180 0`)
  await page.keyboard.press("Backquote")
  await expect.poll(() => page.evaluate(() => (globalThis as any).__playsrcProfile.combat.scores[0].deaths), { timeout: 15_000 }).toBe(1)
  const reverse = page.locator("[data-vgui-name^='DeathNotice']").last()
  await expect(reverse).toContainText(initialBot.name)
  expect(await reverse.locator("span").allTextContents()).toEqual([initialBot.name, "unnamed"])
  const reversePixels = await captureDeathNotice(page, testInfo, "red-kills-blu")
  expect(reversePixels.matchedFraction).toBeGreaterThan(.95)

  await expect.poll(() => page.evaluate(() => (globalThis as any).__playsrcProfile.combat.lifecycle), { timeout: 25_000 }).toBe(1)
  await page.keyboard.press("Backquote")
  await command("tf_bot_kick all")
  await command("joinclass soldier")
  await expect.poll(() => page.evaluate(() => (globalThis as any).__playsrcProfile.combat.scores[0].class)).toBe(3)
  await command("setpos -2528 -1360 17")
  await page.keyboard.press("Backquote")
  await page.locator("canvas.world-canvas").click({ position: { x: 640, y: 360 } })
  await page.evaluate(() => {
    const camera = (globalThis as any).__playsrcProfile.displacementCamera
    const event = new MouseEvent("mousemove", { bubbles: true })
    Object.defineProperty(event, "movementX", { value: 0 })
    Object.defineProperty(event, "movementY", { value: (89 - camera.pitchDegrees) / .066 })
    dispatchEvent(event)
    dispatchEvent(new MouseEvent("mousedown", { button: 0, bubbles: true }))
  })
  await expect.poll(async () => {
    const state = await page.evaluate(() => ({ lifecycle: (globalThis as any).__playsrcProfile.combat.lifecycle,
      health: (globalThis as any).__playsrcProfile.combat.health, camera: (globalThis as any).__playsrcProfile.displacementCamera,
      ammo: document.querySelector<HTMLElement>("main")!.dataset.weaponTrace }))
    if (state.ammo?.startsWith("1:0/")) await page.keyboard.press("KeyR")
    return JSON.stringify(state)
  }, { timeout: 20_000 }).toMatch(/^\{"lifecycle":2,/u)
  await page.evaluate(() => dispatchEvent(new MouseEvent("mouseup", { button: 0, bubbles: true })))
  const self = page.locator("[data-vgui-name^='DeathNotice']").last()
  await expect(self.locator("[data-death-icon='dneg_tf_projectile_rocket']")).toBeVisible()
  expect(await self.locator("span").allTextContents()).toEqual(["unnamed"])
  await captureDeathNotice(page, testInfo, "self-rocket")

  await expect.poll(() => page.evaluate(() => (globalThis as any).__playsrcProfile.combat.lifecycle), { timeout: 25_000 }).toBe(1)
  await page.keyboard.press("Backquote")
  // Configured pl_upward trigger_hurt hammerid 168111, model *28:
  // origin (-768,1088,-1072), damage 9999, damagetype DMG_FALL (32).
  await command("setpos -768 1088 -1072")
  await page.keyboard.press("Backquote")
  await expect.poll(() => page.evaluate(() => (globalThis as any).__playsrcProfile.combat.lifecycle)).toBe(2)
  const world = page.locator("[data-vgui-name^='DeathNotice']").last()
  await expect(world.locator("[data-death-icon='dneg_skull_tf']")).toBeVisible()
  await expect(world).toContainText("fell to a clumsy, painful death")
  await expect(page.locator("[data-vgui-name^='DeathNotice']")).toHaveCount(4)
  expect(await page.locator("[data-vgui-name^='DeathNotice']").evaluateAll(elements => elements.map(element => element.getBoundingClientRect().y))).toEqual([30, 60, 90, 120])
  await captureDeathNotice(page, testInfo, "world-fall")
  await page.setViewportSize({ width: 390, height: 844 })
  await captureDeathNotice(page, testInfo, "world-fall-narrow")
  const narrow = await world.boundingBox()
  expect(narrow!.x + narrow!.width).toBe(390 - Math.trunc(12 * 844 / 480))
  await page.setViewportSize({ width: 1280, height: 720 })
  await page.keyboard.press("Backquote")
  await command("hud_deathnotice_time 6")
  await expect(page.locator("[data-vgui-name^='DeathNotice']")).toHaveCount(1)
  await command("disconnect")
  await expect(page.locator("[data-vgui-name^='DeathNotice']")).toHaveCount(0)

  const report = { schema: "playsrc-tf2-headed-combat-lifecycle-v1", headed: true, target: "pl_upward",
    killed, respawned, authored, simulation: { seconds: measurement.seconds, firstTick: measurement.firstTick,
      lastTick: measurement.lastTick, ticksPerSecond: (measurement.lastTick - measurement.firstTick) / measurement.seconds },
    frames: summarizeFrameTimes(measurement.frames), idleDeathNoticeMutations: measurement.hudMutations }
  const local = await loadLocalConfig()
  const directory = path.join(local.sourceCacheDir, "evidence", "tf2-combat-lifecycle")
  await mkdir(directory, { recursive: true })
  await writeFile(path.join(directory, "pl_upward-combat.json"), `${JSON.stringify(report, null, 2)}\n`)
  console.log(`[combat-lifecycle] ${JSON.stringify(report)}`)
})
