import { createHash } from "node:crypto"
import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
import { expect, test } from "./application-test"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"
import { chooseTf2Team } from "./team-selection-evidence"

test("Demoman stock Bottle damages a visible enemy without approximating projectile physics", async ({ page }, testInfo) => {
  await page.addInitScript(() => {
    ;(globalThis as typeof globalThis & { __playsrcProfile?: object }).__playsrcProfile = {}
    let locked: Element | null = null
    Object.defineProperty(document, "pointerLockElement", { configurable: true, get: () => locked })
    Object.defineProperty(Element.prototype, "requestPointerLock", {
      configurable: true,
      value(this: Element): Promise<void> {
        locked = this
        queueMicrotask(() => document.dispatchEvent(new Event("pointerlockchange")))
        return Promise.resolve()
      },
    })
    Object.defineProperty(document, "exitPointerLock", {
      configurable: true,
      value(): Promise<void> {
        locked = null
        queueMicrotask(() => document.dispatchEvent(new Event("pointerlockchange")))
        return Promise.resolve()
      },
    })
  })
  await page.goto("/")
  await page.waitForFunction(() => ["Startup", "MainMenu", "Failed"].includes(document.querySelector<HTMLElement>("main")?.dataset.phase ?? ""))
  if (await page.locator("main").getAttribute("data-phase") === "Startup") await page.keyboard.press("Escape")
  await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu", { timeout: 120_000 })
  const root = page.locator("main")
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
    const root = document.querySelector<HTMLElement>("main")
    return root?.dataset.teamSelectionVisible === "true" || root?.dataset.phase === "Ready" || root?.dataset.phase === "Failed"
  }, undefined, { timeout: 600_000 })
  if (await root.getAttribute("data-team-selection-visible") === "true") {
    if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await chooseTf2Team(page, "red")
  }
  await expect(root).toHaveAttribute("data-phase", "Ready", { timeout: 120_000 })
  if (await root.getAttribute("data-class-selection-visible") === "true") await command("joinclass demoman")
  else await command("class demoman")
  await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":").slice(1, 3).join(":"))
    .toBe("4:18")

  const local = await loadLocalConfig()
  const directory = path.join(local.sourceCacheDir, "evidence", "tf2-demoman-bottle")
  await mkdir(directory, { recursive: true })
  const loadout: Array<{ weapon: number; name: string; clip: number; reserve: number; activity: string; ammoVisible: boolean; pixelsSha256: string; blockedState: string | null }> = []
  for (const [key, weapon, name] of [["Digit1", 18, "Grenade Launcher"], ["Digit2", 3, "Stickybomb Launcher"], ["Digit3", 17, "Bottle"]] as const) {
    await page.keyboard.press(key)
    await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[2]).toBe(String(weapon))
    await expect.poll(async () => await root.getAttribute("data-viewmodel-activity"))
      .toContain(weapon === 17 ? "MELEE" : weapon === 3 ? "SECONDARY" : "PRIMARY")
    const screenshot = await page.locator("canvas.world-canvas").screenshot()
    const before = await root.evaluate((element) => ({
      trace: (element as HTMLElement).dataset.weaponTrace ?? "",
      projectiles: Number((element as HTMLElement).dataset.projectiles ?? 0),
      fires: Number((element as HTMLElement).dataset.fireEvents ?? 0),
    }))
    let blockedState: string | null = null
    if (weapon !== 17) {
      await page.evaluate(async () => {
        const canvas = document.querySelector(".world-canvas")
        if (!canvas) throw new Error("Demoman weapon canvas is unavailable")
        if (document.pointerLockElement !== canvas) await canvas.requestPointerLock()
        dispatchEvent(new MouseEvent("mousedown", { button: 0, bubbles: true }))
      })
      blockedState = weapon === 18 ? "GrenadePhysicsSolverUnavailable" : "StickyPhysicsSolverUnavailable"
      await expect.poll(async () => await root.getAttribute("data-unsupported-state")).toBe(blockedState)
      await page.evaluate(() => dispatchEvent(new MouseEvent("mouseup", { button: 0, bubbles: true })))
      const after = await root.evaluate((element) => ({
        trace: (element as HTMLElement).dataset.weaponTrace ?? "",
        projectiles: Number((element as HTMLElement).dataset.projectiles ?? 0),
        fires: Number((element as HTMLElement).dataset.fireEvents ?? 0),
      }))
      expect(after).toEqual(before)
    }
    const observation = await root.evaluate((element) => {
      const data = (element as HTMLElement).dataset
      const record = (data.weaponTrace ?? "").split("|").find((value) => value.startsWith(`${data.hudProbe?.split(":")[2]}:`)) ?? ""
      const [clip, reserve] = (record.split(":")[1] ?? "0/0").split("/").map(Number)
      return {
        clip: clip ?? 0,
        reserve: reserve ?? 0,
        activity: data.viewmodelActivity ?? "",
        ammoVisible: document.querySelector<HTMLElement>("[data-vgui-name='HudWeaponAmmo']")?.style.display !== "none",
      }
    })
    expect(observation.ammoVisible).toBe(weapon !== 17)
    loadout.push({ weapon, name, ...observation, pixelsSha256: createHash("sha256").update(decodeScreenshot(screenshot).pixels).digest("hex"), blockedState })
    await writeFile(path.join(directory, `${weapon}-${name.toLowerCase().replaceAll(" ", "-")}.png`), screenshot)
  }
  expect(new Set(loadout.map((weapon) => weapon.pixelsSha256)).size).toBe(3)

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
  await page.evaluate(async () => {
    const canvas = document.querySelector(".world-canvas")
    if (!canvas) throw new Error("Demoman bot-combat canvas is unavailable")
    if (document.pointerLockElement !== canvas) await canvas.requestPointerLock()
  })
  const approach = await page.evaluate(async () => {
    const root = document.querySelector<HTMLElement>("main")!
    const profile = (globalThis as any).__playsrcProfile
    const position = () => (root.dataset.cameraPosition ?? "").split(",").map(Number)
    const bot = () => profile.bots?.[0] as { identity: number; health: number; position: [number, number, number] } | undefined
    const target = () => {
      const current = bot()
      if (!current) throw new Error("Demoman enemy bot disappeared during approach")
      return [current.position[0], current.position[1], current.position[2] + 120]
    }
    const distance = (goal = target()) => Math.hypot(...position().map((value, axis) => value - goal[axis]!))
    const turn = (goal: readonly number[]) => {
      const current = position(), x = goal[0]! - current[0]!, y = goal[1]! - current[1]!, z = goal[2]! - current[2]!
      const yaw = Math.atan2(y, x) * 180 / Math.PI, pitch = -Math.atan2(z, Math.hypot(x, y)) * 180 / Math.PI
      const wrap = (value: number) => ((value + 180) % 360 + 360) % 360 - 180
      const event = new MouseEvent("mousemove", { bubbles: true })
      Object.defineProperties(event, {
        movementX: { value: wrap(Number(root.dataset.cameraYaw) - yaw) / 0.066 },
        movementY: { value: (pitch - Number(root.dataset.cameraPitch)) / 0.066 },
      })
      dispatchEvent(event)
    }
    const started = performance.now()
    const initialHealth = Number(root.dataset.hudProbe?.split(":")[0] ?? 0)
    if (initialHealth <= 0) throw new Error(`Demoman is not alive before the bot approach: ${root.dataset.hudProbe}`)
    const start = position()
    const altitude = Math.max(start[2]!, target()[2]!) + 256
    const goals = [
      () => [start[0]!, start[1]!, altitude],
      () => { const current = target(); return [current[0]!, current[1]!, altitude] },
    ]
    turn(goals[0]!())
    dispatchEvent(new KeyboardEvent("keydown", { code: "KeyW", key: "w", bubbles: true }))
    try {
      for (const goal of goals) {
        while (distance(goal()) > 24) {
          if (performance.now() - started > 30_000) throw new Error(`Demoman bot approach exceeded its bound: ${distance()}; camera=${root.dataset.cameraPosition}; bot=${bot()?.position.join(",")}`)
          if (root.dataset.phase !== "Ready") throw new Error(`Demoman bot approach failed: ${root.dataset.detail}`)
          if (Number(root.dataset.hudProbe?.split(":")[0] ?? 0) <= 0) throw new Error(`Demoman died during the real bot approach: camera=${root.dataset.cameraPosition}; enemy=${bot()?.position.join(",")}; distance=${distance()}; bot=${JSON.stringify(bot())}; probe=${root.dataset.hudProbe}; events=${root.dataset.botProbe}`)
          turn(goal())
          await new Promise((resolve) => setTimeout(resolve, 20))
        }
      }
    } finally {
      dispatchEvent(new KeyboardEvent("keyup", { code: "KeyW", key: "w", bubbles: true }))
    }
    while (distance() > 30) {
      if (performance.now() - started > 30_000) throw new Error(`Demoman bot descent exceeded its bound: ${distance()}; camera=${root.dataset.cameraPosition}`)
      if (Number(root.dataset.hudProbe?.split(":")[0] ?? 0) <= 0) throw new Error(`Demoman died while descending toward its enemy: camera=${root.dataset.cameraPosition}; enemy=${bot()?.position.join(",")}`)
      turn(target())
      dispatchEvent(new KeyboardEvent("keydown", { code: "KeyW", key: "w", bubbles: true }))
      await new Promise((resolve) => setTimeout(resolve, 12))
      dispatchEvent(new KeyboardEvent("keyup", { code: "KeyW", key: "w", bubbles: true }))
      await new Promise((resolve) => setTimeout(resolve, 24))
    }
    const current = bot()!
    return { identity: current.identity, health: current.health, milliseconds: performance.now() - started, distance: distance() }
  })
  const before = await page.locator("canvas.world-canvas").screenshot()
  const combat = await page.evaluate(async ({ identity, initialHealth }) => {
    const root = document.querySelector<HTMLElement>("main")!
    const profile = (globalThis as any).__playsrcProfile
    const bot = () => profile.bots?.find((candidate: { identity: number }) => candidate.identity === identity) as { health: number; position: [number, number, number] } | undefined
    const position = () => (root.dataset.cameraPosition ?? "").split(",").map(Number)
    const aim = () => {
      const enemy = bot()
      if (!enemy) throw new Error("Demoman enemy bot disappeared before impact")
      const current = position(), x = enemy.position[0] - current[0]!, y = enemy.position[1] - current[1]!, z = enemy.position[2] + 75 - current[2]!
      const yaw = Math.atan2(y, x) * 180 / Math.PI, pitch = -Math.atan2(z, Math.hypot(x, y)) * 180 / Math.PI
      const event = new MouseEvent("mousemove", { bubbles: true })
      Object.defineProperties(event, {
        movementX: { value: (((Number(root.dataset.cameraYaw) - yaw + 180) % 360 + 360) % 360 - 180) / 0.066 },
        movementY: { value: (pitch - Number(root.dataset.cameraPitch)) / 0.066 },
      })
      dispatchEvent(event)
      return Math.hypot(x, y)
    }
    const canvas = document.querySelector(".world-canvas")
    if (!canvas) throw new Error("Demoman bot-combat canvas disappeared")
    if (document.pointerLockElement !== canvas) await canvas.requestPointerLock()
    const started = performance.now()
    let walking = false
    dispatchEvent(new MouseEvent("mousedown", { button: 0, bubbles: true }))
    try {
      while ((bot()?.health ?? initialHealth) === initialHealth) {
        if (performance.now() - started > 8_000) throw new Error(`Demoman Bottle did not damage its live enemy: distance=${aim()}; health=${bot()?.health}; audio=${root.dataset.audioStarts}; activity=${root.dataset.viewmodelActivity}; weapon=${root.dataset.hudProbe}; pointer=${document.pointerLockElement === canvas}; wish=${root.dataset.wishSpeed}; gameui=${root.dataset.gameui}`)
        const distance = aim()
        if (distance > 22 && !walking) {
          dispatchEvent(new KeyboardEvent("keydown", { code: "KeyW", key: "w", bubbles: true }))
          walking = true
        } else if (distance < 14 && walking) {
          dispatchEvent(new KeyboardEvent("keyup", { code: "KeyW", key: "w", bubbles: true }))
          walking = false
        }
        await new Promise((resolve) => setTimeout(resolve, 15))
      }
    } finally {
      dispatchEvent(new MouseEvent("mouseup", { button: 0, bubbles: true }))
      if (walking) dispatchEvent(new KeyboardEvent("keyup", { code: "KeyW", key: "w", bubbles: true }))
    }
    return { health: bot()!.health, milliseconds: performance.now() - started, audio: root.dataset.audioStarts ?? "" }
  }, { identity: approach.identity, initialHealth: approach.health })
  const after = await page.locator("canvas.world-canvas").screenshot()
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
  const report = {
    schema: "playsrc-tf2-headed-demoman-bottle-v1",
    headed: true,
    target: "pl_upward",
    loadout,
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
