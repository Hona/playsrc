import { expect, test } from "./application-test"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"
import { chooseTf2Team } from "./team-selection-evidence"

test("authored Medic stock weapons preserve visible models, ammo, bot targets, and simulation cadence", async ({ page }, testInfo) => {
  await page.addInitScript(() => {
    ;(globalThis as typeof globalThis & { __playsrcProfile?: object }).__playsrcProfile = {}
    let locked: Element | null = null
    Object.defineProperty(document, "pointerLockElement", { configurable: true, get: () => locked })
    Object.defineProperty(Element.prototype, "requestPointerLock", { configurable: true, value(this: Element) {
      locked = this
      queueMicrotask(() => document.dispatchEvent(new Event("pointerlockchange")))
      return Promise.resolve()
    } })
    Object.defineProperty(document, "exitPointerLock", { configurable: true, value() {
      locked = null
      queueMicrotask(() => document.dispatchEvent(new Event("pointerlockchange")))
      return Promise.resolve()
    } })
  })
  await page.goto("/")
  await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu", { timeout: 120_000 })
  await page.keyboard.press("Backquote")
  const command = page.locator("[aria-label='Console command']")
  await command.fill("map pl_upward")
  await command.press("Enter")
  await page.waitForFunction(() => {
    const root = document.querySelector<HTMLElement>("main")
    return root?.dataset.teamSelectionVisible === "true" || root?.dataset.phase === "Ready" || root?.dataset.phase === "Failed"
  }, undefined, { timeout: 600_000, polling: 50 })
  if (await page.locator("main").getAttribute("data-team-selection-visible") === "true") {
    if (await page.locator("main").getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await chooseTf2Team(page, "red")
  }
  await expect(page.locator("main")).toHaveAttribute("data-phase", "Ready", { timeout: 600_000 })
  if (await page.locator("main").getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
  await command.fill("joinclass medic")
  await command.press("Enter")
  await expect.poll(async () => page.locator("main").getAttribute("data-weapon-trace"))
    .toContain("19:40/150")
  await command.fill("tf_bot_add red heavy")
  await command.press("Enter")
  await expect(page.locator("main")).toHaveAttribute("data-bot-count", "1")
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile?.bots?.some((bot: any) => bot.identity === 2))
  await command.fill("nb_stop 1")
  await command.press("Enter")
  const stopped = await page.evaluate(() => (globalThis as any).__playsrcProfile.bots.find((bot: any) => bot.identity === 2).position as [number, number, number])
  await page.waitForFunction((position) => {
    const bot = (globalThis as any).__playsrcProfile?.bots?.find((candidate: any) => candidate.identity === 2)
    return Boolean(bot && Math.hypot(bot.position[0] - position[0], bot.position[1] - position[1], bot.position[2] - position[2]) < 8)
  }, stopped, { timeout: 10_000 })
  const patientSpawn = await page.evaluate(() => (globalThis as any).__playsrcProfile.bots.find((bot: any) => bot.identity === 2).position as [number, number, number])
  await command.fill(`setpos ${patientSpawn.join(" ")}`)
  await command.press("Enter")
  await page.waitForFunction((target) => {
    const camera = document.querySelector<HTMLElement>("main")?.dataset.cameraPosition?.split(",").map(Number)
    return camera?.length === 3 && Math.hypot(camera[0]! - target[0], camera[1]! - target[1]) < 64
  }, patientSpawn, { timeout: 10_000 })
  await page.keyboard.press("Backquote")
  await expect(page.locator("main")).toHaveAttribute("data-console-visible", "false")

  await page.keyboard.press("Digit2")
  await expect.poll(async () => page.locator("main").getAttribute("data-weapon-trace"))
    .toContain("20:0/0")
  await expect.poll(async () => page.locator("main").getAttribute("data-viewmodel-activity"))
    .toContain("SECONDARY")
  await expect(page.locator("[data-vgui-name='HudMedicCharge']")).toBeVisible()
  const medigun = await page.locator(".world-canvas").screenshot()
  const medigunPixels = decodeScreenshot(medigun)
  let visibleMedigunPixels = 0
  for (let index = 0; index < medigunPixels.pixels.length; index += medigunPixels.channels) {
    if (medigunPixels.pixels[index]! > 8 || medigunPixels.pixels[index + 1]! > 8 || medigunPixels.pixels[index + 2]! > 8) visibleMedigunPixels += 1
  }
  expect(visibleMedigunPixels).toBeGreaterThan(20_000)
  await testInfo.attach("headed-authored-medigun", { body: medigun, contentType: "image/png" })

  await page.evaluate(async () => {
    const canvas = document.querySelector(".world-canvas")
    if (!canvas) throw new Error("Medic healing evidence canvas is unavailable")
    if (document.pointerLockElement !== canvas) await canvas.requestPointerLock()
    const state = { active: true, forward: false, distance: -1, yawDelta: 0, pitchDelta: 0, updates: 0 }
    ;(globalThis as typeof globalThis & { __playsrcMedicFollow?: typeof state }).__playsrcMedicFollow = state
    const follow = () => {
      if (!state.active) return
      const root = document.querySelector<HTMLElement>("main")!
      const patient = (globalThis as any).__playsrcProfile?.bots?.find((bot: any) => bot.team === 2)
      const camera = root.dataset.cameraPosition?.split(",").map(Number)
      if (patient && camera?.length === 3 && camera.every(Number.isFinite)) {
        const dx = patient.position[0] - camera[0]!
        const dy = patient.position[1] - camera[1]!
        const dz = patient.position[2] + 41 - camera[2]!
        const desiredYaw = Math.atan2(dy, dx) * 180 / Math.PI
        const desiredPitch = -Math.atan2(dz, Math.hypot(dx, dy)) * 180 / Math.PI
        const currentYaw = Number(root.dataset.cameraYaw)
        const currentPitch = Number(root.dataset.cameraPitch)
        let yawDelta = desiredYaw - currentYaw
        while (yawDelta > 180) yawDelta -= 360
        while (yawDelta < -180) yawDelta += 360
        state.distance = Math.hypot(dx, dy, dz)
        state.yawDelta = yawDelta
        state.pitchDelta = desiredPitch - currentPitch
        state.updates += 1
        const movementX = Math.round(-yawDelta / 0.066)
        const movementY = Math.round((desiredPitch - currentPitch) / 0.066)
        if (movementX !== 0 || movementY !== 0) dispatchEvent(new MouseEvent("mousemove", { movementX, movementY, bubbles: true }))
        const shouldMove = Math.hypot(dx, dy) > (state.forward ? 150 : 225)
        if (shouldMove !== state.forward) {
          state.forward = shouldMove
          dispatchEvent(new KeyboardEvent(shouldMove ? "keydown" : "keyup", { key: "w", code: "KeyW", bubbles: true }))
        }
      }
      requestAnimationFrame(follow)
    }
    requestAnimationFrame(follow)
    dispatchEvent(new MouseEvent("mousedown", { button: 0, bubbles: true }))
  })
  await expect.poll(async () => page.evaluate(() => {
    const root = document.querySelector<HTMLElement>("main")!
    const bot = (globalThis as any).__playsrcProfile?.bots?.find((candidate: any) => candidate.team === 2)
    return {
      target: root.dataset.medigunTarget ?? null,
      weapon: root.dataset.weaponTrace,
      pointerLocked: root.dataset.pointerLocked,
      camera: root.dataset.cameraPosition,
      yaw: root.dataset.cameraYaw,
      pitch: root.dataset.cameraPitch,
      bot: bot && { identity: bot.identity, position: bot.position, health: bot.health },
      follow: (globalThis as any).__playsrcMedicFollow,
    }
  }), { timeout: 30_000 }).toMatchObject({ target: "2" })
  await expect.poll(async () => Number(await page.locator("main").getAttribute("data-medigun-charge")), { timeout: 15_000 }).toBeGreaterThan(0.01)
  await expect.poll(async () => page.evaluate(() => (globalThis as any).__playsrcProfile?.bots?.find((bot: any) => bot.identity === 2)?.health), { timeout: 15_000 }).toBeGreaterThan(300)
  await expect.poll(async () => Number(await page.locator("main").getAttribute("data-particle-items")), { timeout: 15_000 }).toBeGreaterThan(0)
  const healing = await page.locator(".world-canvas").screenshot()
  await testInfo.attach("headed-authored-medigun-healing", { body: healing, contentType: "image/png" })
  await expect.poll(async () => page.evaluate(() => {
    const root = document.querySelector<HTMLElement>("main")!
    const bot = (globalThis as any).__playsrcProfile?.bots?.find((candidate: any) => candidate.identity === 2)
    return {
      charge: Number(root.dataset.medigunCharge),
      target: root.dataset.medigunTarget ?? null,
      camera: root.dataset.cameraPosition,
      bot: bot && { position: bot.position, velocity: bot.velocity, health: bot.health },
      follow: (globalThis as any).__playsrcMedicFollow,
    }
  }), {
    timeout: 120_000,
    intervals: [500, 1_000, 2_000],
  }).toMatchObject({ charge: 1, target: "2" })
  await expect.poll(async () => page.locator("main").getAttribute("data-audio-starts"))
    .toContain("WeaponMedigun.Charged")
  await page.evaluate(() => dispatchEvent(new MouseEvent("mousedown", { button: 2, bubbles: true })))
  await expect(page.locator("main")).toHaveAttribute("data-medigun-releasing", "true")
  await expect.poll(async () => Number(await page.locator("main").getAttribute("data-medigun-charge")), { timeout: 10_000 })
    .toBeLessThan(0.99)
  const uber = await page.locator(".world-canvas").screenshot()
  await testInfo.attach("headed-authored-medigun-uber", { body: uber, contentType: "image/png" })
  await page.evaluate(() => dispatchEvent(new MouseEvent("mouseup", { button: 2, bubbles: true })))
  await page.evaluate(() => {
    const follow = (globalThis as any).__playsrcMedicFollow
    follow.active = false
    if (follow.forward) dispatchEvent(new KeyboardEvent("keyup", { key: "w", code: "KeyW", bubbles: true }))
    dispatchEvent(new MouseEvent("mouseup", { button: 0, bubbles: true }))
  })
  await expect.poll(async () => page.locator("main").getAttribute("data-audio-starts")).toContain("WeaponMedigun.HealingHealer")
  await page.keyboard.press("Backquote")
  await command.fill("tf_bot_add blue soldier")
  await command.press("Enter")
  await expect(page.locator("main")).toHaveAttribute("data-bot-count", "2")
  await command.fill("nb_stop 0")
  await command.press("Enter")
  await page.keyboard.press("Backquote")

  await page.keyboard.press("Digit3")
  await expect.poll(async () => page.locator("main").getAttribute("data-viewmodel-activity"))
    .toContain("MELEE")
  await page.evaluate(async () => {
    const canvas = document.querySelector(".world-canvas")
    if (!canvas) throw new Error("Medic weapon evidence canvas is unavailable")
    if (document.pointerLockElement !== canvas) await canvas.requestPointerLock()
    dispatchEvent(new MouseEvent("mousedown", { button: 0, bubbles: true }))
  })
  await expect.poll(async () => page.locator("main").getAttribute("data-audio-starts"))
    .toContain("Weapon_BoneSaw.Miss")
  await page.evaluate(() => dispatchEvent(new MouseEvent("mouseup", { button: 0, bubbles: true })))

  await page.keyboard.press("Digit1")
  await expect.poll(async () => page.locator("main").getAttribute("data-viewmodel-activity"))
    .toContain("PRIMARY")
  await page.evaluate(() => dispatchEvent(new MouseEvent("mousedown", { button: 0, bubbles: true })))
  await expect.poll(async () => page.locator("main").getAttribute("data-fire-events"))
    .not.toBe("0")
  await page.evaluate(() => dispatchEvent(new MouseEvent("mouseup", { button: 0, bubbles: true })))
  await expect.poll(async () => page.locator("main").getAttribute("data-audio-starts"))
    .toContain("Weapon_SyringeGun.Single")
  const syringe = await page.locator(".world-canvas").screenshot()
  const syringePixels = decodeScreenshot(syringe)
  expect(syringePixels.width).toBe(medigunPixels.width)
  expect(syringePixels.height).toBe(medigunPixels.height)
  let changedPixels = 0
  for (let index = 0; index < syringePixels.pixels.length; index += syringePixels.channels) {
    if (Math.abs(syringePixels.pixels[index]! - medigunPixels.pixels[index]!) > 8
      || Math.abs(syringePixels.pixels[index + 1]! - medigunPixels.pixels[index + 1]!) > 8
      || Math.abs(syringePixels.pixels[index + 2]! - medigunPixels.pixels[index + 2]!) > 8) changedPixels += 1
  }
  expect(changedPixels).toBeGreaterThan(500)
  await testInfo.attach("headed-authored-syringe", { body: syringe, contentType: "image/png" })

  const seconds = profileSampleSeconds()
  const measurement = await page.evaluate(async (duration) => {
    const root = document.querySelector<HTMLElement>("main")!
    const first = Number(root.dataset.snapshotTick)
    const started = performance.now()
    const frames: number[] = []
    let prior = started
    await new Promise<void>((resolve) => {
      const frame = (now: number) => {
        frames.push(now - prior)
        prior = now
        if (now - started >= duration * 1_000) resolve()
        else requestAnimationFrame(frame)
      }
      requestAnimationFrame(frame)
    })
    return { seconds: (performance.now() - started) / 1_000, ticks: Number(root.dataset.snapshotTick) - first, frames }
  }, seconds)
  expect(measurement.ticks).toBeGreaterThan(seconds * 55)
  console.log(`[medic-stock-weapons] ${JSON.stringify({ headed: true, target: "pl_upward", bots: 2, healedPatient: true, deployedUber: true, visibleMedigunPixels, changedWeaponPixels: changedPixels, seconds: measurement.seconds, ticksPerSecond: measurement.ticks / measurement.seconds, frames: summarizeFrameTimes(measurement.frames) })}`)
})
