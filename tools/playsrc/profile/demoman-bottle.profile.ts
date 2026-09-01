import { createHash } from "node:crypto"
import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
import { expect, test } from "./demoman-test"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"
import { chooseTf2Team } from "./team-selection-evidence"

test("Demoman stock Bottle damages a visible enemy using real pointer-locked input", async ({ page, nativeGameplay }, testInfo) => {
  test.setTimeout(150_000)
  await page.addInitScript(() => {
    ;(globalThis as typeof globalThis & { __playsrcProfile?: object }).__playsrcProfile = {}
  })
  await page.goto("/")
  const root = page.locator("main")
  await page.waitForFunction(() => ["Startup", "MainMenu", "Failed"].includes(document.querySelector<HTMLElement>("main")?.dataset.phase ?? ""))
  if (await root.getAttribute("data-phase") === "Startup") await page.keyboard.press("Escape")
  await expect(root).toHaveAttribute("data-phase", "MainMenu", { timeout: 30_000 })
  const entry = page.locator("[aria-label='Console command']")
  const command = async (value: string, close = true) => {
    if (await root.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    await expect(entry).toBeVisible()
    await entry.fill(value)
    await entry.press("Enter")
    if (close && await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  }
  await command("map pl_upward", false)
  await page.waitForFunction(() => {
    const data = document.querySelector<HTMLElement>("main")?.dataset
    return data?.teamSelectionVisible === "true" || data?.phase === "Ready" || data?.phase === "Failed"
  }, undefined, { timeout: 90_000 })
  if (await root.getAttribute("data-team-selection-visible") === "true") {
    if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await chooseTf2Team(page, "red")
  }
  await expect(root).toHaveAttribute("data-phase", "Ready", { timeout: 30_000 })
  if (await root.getAttribute("data-class-selection-visible") === "true") await command("joinclass demoman")
  else await command("class demoman")
  const local = await loadLocalConfig()
  const directory = path.join(local.sourceCacheDir, "evidence", "tf2-demoman-bottle", path.basename(process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!))
  await mkdir(directory, { recursive: true })
  await page.keyboard.press("Digit3")
  await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":").slice(1, 3).join(":"))
    .toBe("4:17")
  await expect.poll(() => root.getAttribute("data-viewmodel-activity")).toContain("MELEE")
  await expect(page.locator("[data-vgui-name='HudWeaponAmmo']")).toBeHidden()
  const canvas = page.locator("canvas.world-canvas")
  await canvas.screenshot({ path: path.join(directory, "17-bottle.png") })

  await command("tf_bot_add blue demoman normal")
  await expect(root).toHaveAttribute("data-bot-count", "1")
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile?.bots?.length === 1)
  const enemySpawn = await page.evaluate(() => [...(globalThis as any).__playsrcProfile.bots[0].position] as [number, number, number])
  await page.waitForFunction((spawn) => {
    const bot = (globalThis as any).__playsrcProfile?.bots?.[0]
    return !!bot && Math.hypot(...bot.position.map((value: number, axis: number) => value - spawn[axis]!)) > 320
  }, enemySpawn, { timeout: 15_000 })
  await command("noclip")
  await expect(root).toHaveAttribute("data-movement-mode", "1")
  const pointer = await nativeGameplay.lockPointer()
  const observe = () => page.evaluate(() => {
    const data = document.querySelector<HTMLElement>("main")!.dataset
    const enemy = (globalThis as any).__playsrcProfile?.bots?.[0] as { identity: number; health: number; position: [number, number, number] } | undefined
    if (!enemy || data.phase !== "Ready" || Number(data.hudProbe?.split(":")[0] ?? 0) <= 0
      || !document.pointerLockElement?.matches("canvas.world-canvas")) {
      throw new Error(`Demoman combat lost its live player, enemy, or pointer lock: ${data.detail}; ${data.hudProbe}`)
    }
    return { enemy, position: (data.cameraPosition ?? "").split(",").map(Number), yaw: Number(data.cameraYaw), pitch: Number(data.cameraPitch), audio: data.audioStarts ?? "" }
  })
  let mouseX = pointer.x, mouseY = pointer.y
  const turn = async (state: Awaited<ReturnType<typeof observe>>, goal: readonly number[]) => {
    const x = goal[0]! - state.position[0]!, y = goal[1]! - state.position[1]!, z = goal[2]! - state.position[2]!
    const yaw = Math.atan2(y, x) * 180 / Math.PI, pitch = -Math.atan2(z, Math.hypot(x, y)) * 180 / Math.PI
    mouseX += (((state.yaw - yaw + 180) % 360 + 360) % 360 - 180) / 0.066
    mouseY += (pitch - state.pitch) / 0.066
    await page.mouse.move(mouseX, mouseY)
    return Math.hypot(x, y, z)
  }
  const started = Date.now(), start = await observe()
  const altitude = Math.max(start.position[2]!, start.enemy.position[2] + 120) + 256
  for (let stage = 0; stage < 3; stage++) {
    let walking = false
    try {
      for (;;) {
        const state = await observe()
        const goal = stage === 0 ? [start.position[0]!, start.position[1]!, altitude]
          : [state.enemy.position[0], state.enemy.position[1], stage === 1 ? altitude : state.enemy.position[2] + 120]
        if (Date.now() - started > 30_000) throw new Error(`Demoman approach exceeded its bound: ${JSON.stringify(state)}`)
        if (await turn(state, goal) <= (stage === 2 ? 30 : 24)) break
        if (!walking) { await page.keyboard.down("KeyW"); walking = true }
        await page.waitForTimeout(stage === 2 ? 12 : 20)
        if (stage === 2) { await page.keyboard.up("KeyW"); walking = false; await page.waitForTimeout(24) }
      }
    } finally { if (walking) await page.keyboard.up("KeyW") }
  }
  const approached = await observe()
  const approach = { identity: approached.enemy.identity, health: approached.enemy.health, milliseconds: Date.now() - started }
  await turn(approached, [approached.enemy.position[0], approached.enemy.position[1], approached.enemy.position[2] + 75])
  await nativeGameplay.capture("bottle-before")
  const before = await canvas.screenshot()
  const combatStarted = Date.now()
  let walking = false, current = await observe()
  await page.mouse.down({ button: "left" })
  try {
    while (current.enemy.health === approach.health) {
      if (Date.now() - combatStarted > 8_000) throw new Error(`Demoman Bottle did not damage its live enemy: ${JSON.stringify(current)}`)
      await turn(current, [current.enemy.position[0], current.enemy.position[1], current.enemy.position[2] + 75])
      const distance = Math.hypot(current.enemy.position[0] - current.position[0]!, current.enemy.position[1] - current.position[1]!)
      if (distance > 22 && !walking) { await page.keyboard.down("KeyW"); walking = true }
      else if (distance < 14 && walking) { await page.keyboard.up("KeyW"); walking = false }
      await page.waitForTimeout(15)
      current = await observe()
    }
  } finally {
    await page.mouse.up({ button: "left" })
    if (walking) await page.keyboard.up("KeyW")
  }
  const combat = { health: current.enemy.health, milliseconds: Date.now() - combatStarted, audio: current.audio }
  await nativeGameplay.capture("bottle-after")
  const after = await canvas.screenshot()
  expect(approach.health - combat.health).toBe(65)
  expect(combat.audio).toContain("Weapon_Bottle.Miss")
  expect(combat.audio).toContain("Weapon_Bottle.HitFlesh")
  const beforePixelsSha256 = createHash("sha256").update(decodeScreenshot(before).pixels).digest("hex")
  const afterPixelsSha256 = createHash("sha256").update(decodeScreenshot(after).pixels).digest("hex")
  expect(beforePixelsSha256).not.toBe(afterPixelsSha256)
  await Promise.all([
    testInfo.attach("headed-demoman-bottle-before-enemy-impact", { body: before, contentType: "image/png" }),
    testInfo.attach("headed-demoman-bottle-after-enemy-impact", { body: after, contentType: "image/png" }),
    writeFile(path.join(directory, "bot-before-impact.png"), before),
    writeFile(path.join(directory, "bot-after-impact.png"), after),
  ])

  const seconds = profileSampleSeconds()
  const sample = await page.evaluate(async (seconds) => {
    const root = document.querySelector<HTMLElement>("main")!
    const firstTick = Number(root.dataset.snapshotTick)
    const started = performance.now()
    let previous = started
    const frames: number[] = []
    const models: number[] = []
    await new Promise<void>((resolve) => {
      const frame = (now: number) => {
        frames.push(now - previous)
        previous = now
        const detail = root.dataset.performanceDetail
        if (detail) models.push(Number((JSON.parse(detail) as { models?: number }).models ?? 0))
        if (now - started >= seconds * 1_000) resolve()
        else requestAnimationFrame(frame)
      }
      requestAnimationFrame(frame)
    })
    return { seconds: (performance.now() - started) / 1_000, firstTick, lastTick: Number(root.dataset.snapshotTick), frames, models }
  }, seconds)
  expect(sample.lastTick - sample.firstTick).toBeGreaterThan(seconds * 60)
  const pointerLocked = await page.evaluate(() => document.pointerLockElement?.matches("canvas.world-canvas") ?? false)
  expect(pointerLocked).toBe(true)
  const report = {
    schema: "playsrc-tf2-headed-demoman-bottle-v2",
    headed: true,
    target: "pl_upward",
    weapon: "Bottle",
    pointerLocked,
    enemy: {
      identity: approach.identity,
      initialHealth: approach.health,
      finalHealth: combat.health,
      damage: approach.health - combat.health,
      swingAudio: "Weapon_Bottle.Miss",
      impactAudio: "Weapon_Bottle.HitFlesh",
      approachMilliseconds: Number(approach.milliseconds.toFixed(3)),
      combatMilliseconds: Number(combat.milliseconds.toFixed(3)),
      beforePixelsSha256,
      afterPixelsSha256,
    },
    simulation: {
      seconds: Number(sample.seconds.toFixed(3)),
      firstTick: sample.firstTick,
      lastTick: sample.lastTick,
      ticksPerSecond: Number(((sample.lastTick - sample.firstTick) / sample.seconds).toFixed(2)),
    },
    frames: summarizeFrameTimes(sample.frames),
    models: { samples: sample.models.length, maximumMilliseconds: Number(Math.max(...sample.models).toFixed(3)) },
  }
  await writeFile(path.join(directory, "pl_upward-demoman-bottle.json"), `${JSON.stringify(report, null, 2)}\n`)
  console.log(`[demoman-bottle] ${JSON.stringify(report)}`)
})
