import { writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, test } from "./application-test"
import { chooseTf2Team } from "./team-selection-evidence"
import { loadLocalConfig } from "../src/config"
import { macPageAdmission, requireMacPageAdmission } from "./macos-page-admission"
import { createHash } from "node:crypto"
import { damageIndicatorPixels } from "./damage-indicator-pixels"

test("authored radial damage follows real bot hits and camera bearing", async ({ page }) => {
  const directory = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!
  const dynamic = process.env.PROFILE_DAMAGE_DYNAMIC === "1"
  const selfDamage = process.env.PROFILE_DAMAGE_SELF === "1"
  const yaw = Number(process.env.PROFILE_DAMAGE_YAW ?? 90), pitch = selfDamage ? 89 : Number(process.env.PROFILE_DAMAGE_PITCH ?? 0)
  if (!directory) throw new Error("Use the checked headed profile runner")
  await page.addInitScript(() => {
    (globalThis as any).__playsrcProfile = { captureMelee: true, captureDamageIndicators: true }
    addEventListener("mousemove", event => {
      if (document.pointerLockElement) (globalThis as any).__damagePointer = { x: event.movementX, y: event.movementY, trusted: event.isTrusted }
    })
  })
  await page.goto("/")
  await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu")
  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  const command = async (value: string) => { await entry.fill(value); await entry.press("Enter") }
  await command("map pl_upward")
  await expect(page.locator("main")).toHaveAttribute("data-team-selection-visible", "true")
  await page.keyboard.press("Backquote")
  await chooseTf2Team(page, "blue")
  await expect(page.locator("main")).toHaveAttribute("data-phase", "Ready")
  await page.keyboard.press("Backquote")
  await command(`joinclass ${selfDamage ? "soldier" : "heavyweapons"}`)
  await command("nb_stop 1")
  await command("setpos -2528 -1360 17")
  if (!selfDamage) {
    await command(`tf_bot_add red ${dynamic ? "sniper" : "scout"} easy`)
    await expect(page.locator("main")).toHaveAttribute("data-bot-count", "1")
    const bot = await page.evaluate(() => (globalThis as any).__playsrcProfile.combat.scores.find((p: any) => p.identity !== 1))
    await command(`bot_teleport "${bot.name}" -2450 -1360 17 0 180 0`)
  }
  await page.keyboard.press("Backquote")
  await page.bringToFront()
  await page.locator("canvas.world-canvas").click({ position: { x: 640, y: 360 } })
  await expect(page.locator("main")).toHaveAttribute("data-pointer-locked", "true")
  const camera = await page.evaluate(() => (globalThis as any).__playsrcProfile.displacementCamera)
  // Trusted browser mouse input, through the normal pointer-lock camera owner.
  await page.mouse.move(640 - (((yaw - camera.yawDegrees + 540) % 360) - 180) / .066, 360 + (pitch - camera.pitchDegrees) / .066)
  await expect.poll(() => page.evaluate(target => {
    const p = (globalThis as any).__playsrcProfile.displacementCamera
    return Math.abs(((p.yawDegrees - target + 540) % 360) - 180)
  }, yaw)).toBeLessThan(.1)
  const native = await macPageAdmission(page, (await loadLocalConfig()).sourceCacheDir)
  const cdp = await page.context().newCDPSession(page)
  const records: unknown[] = []
  const captures: unknown[] = []
  const capture = async (name: string, bearing?: number, expectedPitch?: number) => {
    const state = await page.evaluate(() => {
      const p = (globalThis as any).__playsrcProfile
      const element = [...document.querySelectorAll<HTMLElement>("[data-tf2-damage-indicator]")].at(-1)!
      if (!element) throw new Error("Real gameplay damage indicator expired before capture")
      const s = element.style
      return { camera: p.displacementCamera, health: p.combat.health, lifecycle: p.combat.lifecycle, events: p.meleeTimeline,
        viewport: { width: innerWidth, height: innerHeight, dpr: devicePixelRatio }, material: element.dataset.sourceMaterial,
        texture: s.backgroundImage.match(/base64,([^\"]+)/)![1]!, opacity: Number(s.opacity),
        quad: { x: parseFloat(s.left), y: parseFloat(s.top), width: parseFloat(s.width), height: parseFloat(s.height), rotation: Number(s.transform.match(/rotate\((.*)rad\)/)![1]) },
        pivot: getComputedStyle(element).transformOrigin }
    })
    // Like the scope profile, read the headed compositor without Playwright's
    // device-metrics restoration (which can recenter a locked native pointer).
    const png = Buffer.from((await cdp.send("Page.captureScreenshot", { format: "png", fromSurface: true, captureBeyondViewport: false })).data, "base64")
    await writeFile(path.join(directory, `${name}.png`), png)
    const pixels = damageIndicatorPixels(png, Buffer.from(state.texture, "base64"), state.quad, state.viewport)
    const { texture, ...facts } = state
    const record = { name, ...facts, pixels, sha256: createHash("sha256").update(png).digest("hex"), bytes: png.length }
    captures.push(record)
    await writeFile(path.join(directory, "captures.json"), JSON.stringify(captures, null, 2))
    expect(state.material).toBe("materials/vgui/damageindicator.vmt")
    expect(state.viewport.dpr).toBe(Number(process.env.PLAYSRC_PROFILE_DEVICE_SCALE_FACTOR ?? 1))
    expect(state.health).toBeLessThan(selfDamage ? 200 : 300)
    expect(pixels.texture).toMatchObject({ width: 128, height: 64 })
    expect(pixels.texture.alphaWeightedV).toBeGreaterThan(.6)
    const pivot = state.pivot.split(" ").map(parseFloat)
    expect(pivot[0]).toBeCloseTo(state.quad.width / 2, 2)
    expect(pivot[1]).toBeCloseTo(state.quad.height / 2, 2)
    expect(pixels.redPixels).toBeGreaterThan(50)
    expect(pixels.inwardCosine).toBeGreaterThan(.7)
    if (bearing !== undefined) {
      const relative = Math.atan2(-(state.quad.x + state.quad.width / 2 - state.viewport.width / 2), -(state.quad.y + state.quad.height / 2 - state.viewport.height / 2)) * 180 / Math.PI
      expect(Math.abs(((relative - bearing + 540) % 360) - 180)).toBeLessThan(1)
    }
    if (expectedPitch !== undefined) expect(Math.abs(state.camera.pitchDegrees - expectedPitch)).toBeLessThan(.2)
  }
  try {
    if (native) await expect.poll(async () => {
      const record = await native.read()
      return record.error ?? record.occluders?.map(w => w.id).join(",") ?? ""
    }, { timeout: 8_000 }).toBe("")
    const admission = await native?.read(path.join(directory, "private-desktop.png"))
    if (admission) { records.push(admission); requireMacPageAdmission(admission) }
    if (selfDamage) await page.mouse.down()
    else {
      await page.keyboard.press("Backquote")
      await command("nb_stop 0")
      await page.keyboard.press("Backquote")
    }
    if (dynamic) {
      await page.locator("canvas.world-canvas").click({ position: { x: 640, y: 360 } })
      if (native) {
        await expect.poll(async () => {
          const record = await native.read()
          return record.error ?? record.occluders?.map(w => w.id).join(",") ?? ""
        }, { timeout: 8_000 }).toBe("")
        const record = await native.read()
        records.push(record); requireMacPageAdmission(record)
      }
    }
    await page.waitForSelector("[data-tf2-damage-indicator]", { timeout: 10_000 })
    if (selfDamage) await page.mouse.up()
    if (dynamic) {
      let pointerX = 640, pointerY = 360
      const views = [...[0, 90, 180, 270, 45, 135, 225, 315].map(bearing => ({ bearing, pitch })),
        { bearing: 45, pitch: -89 }, { bearing: 225, pitch: 89 }]
      for (const view of views) {
        const { bearing } = view
        await page.waitForSelector("[data-tf2-damage-indicator]", { timeout: 2_000 })
        for (let attempt = 0; attempt < 6; attempt++) {
          // Wait for a fresh real hit if this one is fading; do not stretch its
          // lifetime or slow simulation for screenshot extraction.
          await page.waitForFunction(() => {
            const e = [...document.querySelectorAll<HTMLElement>("[data-tf2-damage-indicator]")].at(-1)
            return e && Number(e.style.opacity) > .8
          }, undefined, { timeout: 2_000 })
          const delta = await page.evaluate(target => {
            const e = [...document.querySelectorAll<HTMLElement>("[data-tf2-damage-indicator]")].at(-1)!
            const s = e.style, x = parseFloat(s.left) + parseFloat(s.width) / 2 - innerWidth / 2, y = parseFloat(s.top) + parseFloat(s.height) / 2 - innerHeight / 2
            return { yaw: ((Math.atan2(-x, -y) * 180 / Math.PI - target + 540) % 360) - 180,
              pitch: (globalThis as any).__playsrcProfile.displacementCamera.pitchDegrees }
          }, bearing)
          if (Math.abs(delta.yaw) < .2 && Math.abs(delta.pitch - view.pitch) < .2) break
          const revision = Number(await page.locator(".world-canvas").getAttribute("data-display-mouse-revision"))
          pointerX -= delta.yaw / .066; pointerY += (view.pitch - delta.pitch) / .066
          await page.mouse.move(pointerX, pointerY)
          await page.waitForFunction(revision => Number(document.querySelector<HTMLElement>(".world-canvas")?.dataset.displayMouseRevision) > revision, revision, { timeout: 2_000 })
          const delivered = await page.evaluate(() => (globalThis as any).__damagePointer)
          expect(delivered.trusted).toBe(true)
        }
        await capture(`bearing-${bearing}-pitch-${view.pitch}`, bearing, view.pitch)
      }
    }
    await page.keyboard.press("Backquote")
    await command("nb_stop 1")
    await page.keyboard.press("Backquote")
    if (!dynamic) {
      await capture("known-source")
      if (process.env.PROFILE_DAMAGE_RESIZE === "1") {
        await page.setViewportSize({ width: 997, height: 613 })
        await page.evaluate(() => new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))))
        await capture("resized")
      }
    }
    const state = await page.evaluate(() => {
      const p = (globalThis as any).__playsrcProfile
      return { camera: p.displacementCamera, combat: p.combat, bots: p.bots, events: p.meleeTimeline,
        indicators: [...document.querySelectorAll<HTMLElement>("[data-tf2-damage-indicator]")].map(e => ({ style: e.getAttribute("style"), bounds: e.getBoundingClientRect().toJSON() })) }
    })
    await writeFile(path.join(directory, "known-source.json"), JSON.stringify(state, null, 2))
    expect(state.indicators.length).toBeGreaterThan(0)
    expect(state.combat.health).toBeLessThan(300)
    if (selfDamage) {
      const hit = state.events.find((event: any) => event.kind === 6 && event.detail === 1)
      expect(hit).toBeTruthy()
      const z = new DataView(Uint32Array.of(hit.subject).buffer).getFloat32(0, true)
      const delta = [hit.values[2] - camera.position[0], hit.values[3] - camera.position[1], z - camera.position[2]]
      expect(delta[2]).toBeLessThan(0)
      expect(Math.abs(delta[2]!) / Math.hypot(...delta)).toBeGreaterThan(.9)
    }
    const endpoint = await native?.read(path.join(directory, "private-desktop-end.png"))
    if (endpoint) { records.push(endpoint); requireMacPageAdmission(endpoint) }
    await expect(page.locator("[data-tf2-damage-indicator]")).toHaveCount(0, { timeout: 3_000 })
    if (process.env.PROFILE_DAMAGE_LIFECYCLE === "1") {
      await page.keyboard.press("Backquote")
      await command("nb_stop 0")
      await page.keyboard.press("Backquote")
      await expect.poll(() => page.evaluate(() => (globalThis as any).__playsrcProfile.combat.lifecycle), { timeout: 15_000 }).toBe(2)
      await page.keyboard.press("Backquote")
      await command("nb_stop 1")
      await page.keyboard.press("Backquote")
      await expect(page.locator("[data-tf2-damage-indicator]")).toHaveCount(0, { timeout: 3_000 })
      await expect.poll(() => page.evaluate(() => (globalThis as any).__playsrcProfile.combat.lifecycle), { timeout: 25_000 }).toBe(1)
      await expect(page.locator("[data-tf2-damage-indicator]")).toHaveCount(0)
      await writeFile(path.join(directory, "respawn.json"), JSON.stringify(await page.evaluate(() => (globalThis as any).__playsrcProfile.combat)))
    }
    await page.keyboard.press("Backquote")
    await command("disconnect")
    await expect(page.locator("[data-tf2-damage-indicator]")).toHaveCount(0)
  } finally {
    await writeFile(path.join(directory, "terminal-game.json"), JSON.stringify(await page.evaluate(() => {
      const p = (globalThis as any).__playsrcProfile
      return { camera: p.displacementCamera, combat: p.combat, bots: p.bots, events: p.meleeTimeline,
        console: document.querySelector("[aria-label='Console output']")?.textContent }
    }), null, 2))
    await writeFile(path.join(directory, "native-admission.json"), JSON.stringify({ performanceSample: false, records }, null, 2))
    await native?.close()
    await cdp.detach()
  }
})
