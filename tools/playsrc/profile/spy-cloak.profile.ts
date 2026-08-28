import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, test } from "./application-test"
import { chooseTf2Team } from "./team-selection-evidence"
import { loadLocalConfig } from "../src/config"
import { decodeScreenshot } from "./screenshot-pixels"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"

test("headed stock Spy cloak preserves world refraction through local arms and weapon", async ({ page }, testInfo) => {
  page.setDefaultTimeout(10_000)
  const lifecycle = process.env.PROFILE_CLOAK_LIFECYCLE === "1"
  const team = lifecycle ? "blue" : "red"
  if (process.env.PROFILE_CLOAK_HDR === "1") await page.route(url => url.pathname.endsWith("/playsrc-config.json"), async route => {
    const response = await route.fetch(), configuration = await response.json()
    await route.fulfill({ response, json: { ...configuration, renderLevel: 2 } })
  })
  const root = page.locator("main"), canvas = page.locator("canvas.world-canvas")
  const directory = path.join((await loadLocalConfig()).sourceCacheDir, "evidence", "spy-cloak", testInfo.project.name, String(Date.now()))
  await mkdir(directory, { recursive: true })
  await page.addInitScript(installBrowserFrameProfiler)
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = {}; (globalThis as any).__cloakInput = []; for (const type of ["mousedown", "mouseup"]) addEventListener(type, event => (globalThis as any).__cloakInput.push({ type, button: (event as MouseEvent).button, trusted: event.isTrusted }), true) })
  await page.goto("/")
  await expect(root).toHaveAttribute("data-phase", "MainMenu", { timeout: 60_000 })
  const command = async (text: string) => {
    if (await root.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    await page.locator("[aria-label='Console command']").fill(text)
    await page.keyboard.press("Enter")
    if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  }
  const toggleCloak = async () => {
    await page.bringToFront(); await canvas.focus()
    if (!await page.evaluate(() => document.pointerLockElement === document.querySelector("canvas.world-canvas") && document.hasFocus())) await canvas.click()
    await expect(root).toHaveAttribute("data-pointer-locked", "true")
    await page.mouse.down({ button: "right" })
    await page.waitForTimeout(35)
    await page.mouse.up({ button: "right" })
  }
  await command("map pl_upward")
  await expect(root).toHaveAttribute("data-team-selection-visible", "true", { timeout: 60_000 })
  await chooseTf2Team(page, team)
  await page.keyboard.press("Comma")
  await expect(root).toHaveAttribute("data-class-selection-visible", "true", { timeout: 30_000 })
  await page.keyboard.press("Digit9")
  await expect(root).toHaveAttribute("data-class-selection-visible", "false")
  await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[1]).toBe("8")
  await page.bringToFront(); await canvas.focus(); await canvas.click()
  await expect(root).toHaveAttribute("data-pointer-locked", "true")
  const captures: any[] = []
  let inputs: any[] = []
  const capture = async (label: string, when?: { identity: number; factor: number }) => {
    const revision = captures.length + 1
    await page.evaluate(({ revision, when }) => { (globalThis as any).__playsrcProfile.cloakCaptureRevision = revision; (globalThis as any).__playsrcProfile.cloakCaptureCondition = when }, { revision, when })
    await page.waitForFunction(revision => (globalThis as any).__playsrcProfile.cloakCapture?.revision === revision, revision, { timeout: 5000 })
    const value = await page.evaluate(() => (globalThis as any).__playsrcProfile.cloakCapture)
    const png = Buffer.from(value.pixels.split(",")[1], "base64")
    await writeFile(path.join(directory, `${label}.png`), png)
    const image = decodeScreenshot(png)
    const pixel = (sample: any) => {
      const x = Math.min(image.width - 1, Math.floor((sample.x + 1) * 0.5 * image.width)), y = Math.min(image.height - 1, Math.floor((1 - sample.y) * 0.5 * image.height)), at = (y * image.width + x) * image.channels
      return { ...sample, rgb: [...image.pixels.subarray(at, at + 3)] }
    }
    captures.push({ ...value, label, pixels: `${label}.png`, samples: value.viewGeometry.samples.map(pixel), worldSamples: value.worldGeometry.samples.map(pixel), pixelWidth: image.width, pixelHeight: image.height })
    await writeFile(path.join(directory, `${label}.json`), JSON.stringify(captures.at(-1), (_, value) => typeof value === "bigint" ? value.toString() : value, 2))
    return captures.at(-1)
  }
  if (process.env.PROFILE_CLOAK_BOTS_ONLY !== "1") {
  await page.waitForTimeout(600)
  await capture("uncloaked")
  await toggleCloak()
  await page.waitForFunction(() => { const value = Number(document.querySelector<HTMLElement>("main")!.dataset.spyProbe!.split(":")[1]); return value > 0.2 && value < 0.7 })
  await capture("fading")
  await expect.poll(async () => (await root.getAttribute("data-spy-probe"))?.split(":")[1]).toBe("1.000")
  await capture("cloaked")
  await toggleCloak()
  await page.waitForTimeout(700)
  await capture("decloaking")
  await expect.poll(async () => (await root.getAttribute("data-spy-probe"))?.split(":")[1]).toBe("0.000")
  await capture("decloaked")
  inputs = await page.evaluate(() => (globalThis as any).__cloakInput)
  await writeFile(path.join(directory, "report.json"), JSON.stringify({ captures, inputs }, (_, value) => typeof value === "bigint" ? value.toString() : value, 2))
  console.log("[spy-cloak]", directory)
  expect(inputs.filter((event: any) => event.button === 2).every((event: any) => event.trusted)).toBe(true)
  expect(captures[0].copies).toBe(0)
  expect(captures[2].copies).toBeGreaterThan(0)
  expect(captures[2].models.filter((model: any) => model.viewModel).every((model: any) => model.cloak?.localFactor === 0.5)).toBe(true)
  expect(captures[2].samples.length).toBeGreaterThan(0)
  expect(captures[0].samples.some((sample: any, index: number) => sample.rgb.some((channel: number, axis: number) => Math.abs(channel - (captures[2].samples[index]?.rgb[axis] ?? channel)) > 10))).toBe(true)

  // Select every authored stock viewmodel while cloaked; no shader overrides.
  await toggleCloak()
  await expect.poll(async () => (await root.getAttribute("data-spy-probe"))?.split(":")[1]).toBe("1.000")
  for (const [digit, weapon] of [[2, 52], [3, 51], [4, 53]] as const) {
    await page.keyboard.press(`Digit${digit}`)
    await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[2]).toBe(String(weapon))
    await page.waitForTimeout(350)
    await capture(`${team}-cloaked-${weapon}`)
  }
  await page.keyboard.press("Digit1")
  await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[2]).toBe("50")
  if (lifecycle) {
    await page.setViewportSize({ width: 1100, height: 760 })
    await page.waitForFunction(() => document.querySelector<HTMLCanvasElement>("canvas.world-canvas")?.width === 1100 * devicePixelRatio)
    const resized = await capture("cloaked-resized")
    expect(resized.pixelWidth).toBe(1100 * Number(process.env.PLAYSRC_PROFILE_DEVICE_SCALE_FACTOR ?? 1))
    expect(resized.lightingProfile).toBe(process.env.PROFILE_CLOAK_HDR === "1" ? "hdr" : "ldr")
    await page.keyboard.press("Escape")
    await expect(root).toHaveAttribute("data-gameui", "pause")
    await page.waitForTimeout(100)
    const pausedTick = await root.getAttribute("data-snapshot-tick")
    await page.waitForTimeout(150)
    expect(await root.getAttribute("data-snapshot-tick")).toBe(pausedTick)
    await page.screenshot({ path: path.join(directory, "paused.png") })
    await page.keyboard.press("Escape")
    await expect(root).toHaveAttribute("data-gameui", "in-game")
    await page.bringToFront(); await canvas.focus(); await canvas.click()
    await expect(root).toHaveAttribute("data-pointer-locked", "true")
    await expect.poll(async () => (await root.getAttribute("data-spy-probe"))?.split(":")[1], { timeout: 16000 }).toBe("0.000")
    const exhausted = await capture("exhausted")
    expect(exhausted.copies).toBe(0)
    await page.keyboard.press("Comma"); await expect(root).toHaveAttribute("data-class-selection-visible", "true")
    await page.keyboard.press("Digit2"); await expect(root).toHaveAttribute("data-class-selection-visible", "false")
    await page.bringToFront(); await canvas.focus(); await canvas.click()
    const switched = await capture("class-switched")
    expect(switched.models.filter((model: any) => model.viewModel).every((model: any) => !model.cloak)).toBe(true)
    for (const map of ["ctf_2fort", "jump_beef"]) {
      await command(`map ${map}`)
      await expect(root).toHaveAttribute("data-team-selection-visible", "true", { timeout: 60000 })
      await chooseTf2Team(page, "blue")
      await page.bringToFront(); await canvas.focus(); await canvas.click()
      const replaced = await capture(`${map}-replaced`)
      expect(replaced.generation).toBeGreaterThan(resized.generation)
      expect(replaced.copies).toBe(0)
      if (map === "ctf_2fort") {
        await page.keyboard.press("Comma"); await expect(root).toHaveAttribute("data-class-selection-visible", "true")
        await page.keyboard.press("Digit9"); await expect(root).toHaveAttribute("data-class-selection-visible", "false")
        await page.bringToFront(); await canvas.focus(); await canvas.click()
        await expect(root).toHaveAttribute("data-pointer-locked", "true")
        await toggleCloak()
        await expect.poll(async () => (await root.getAttribute("data-spy-probe"))?.split(":")[1]).toBe("1.000")
        expect((await capture("ctf_2fort-cloaked")).copies).toBeGreaterThan(0)
      }
    }
    await writeFile(path.join(directory, "report.json"), JSON.stringify({ captures, inputs }, (_, value) => typeof value === "bigint" ? value.toString() : value, 2))
    return
  }
  await toggleCloak()
  await expect.poll(async () => (await root.getAttribute("data-spy-probe"))?.split(":")[1]).toBe("0.000")
  await capture("local-disguise-decloaked")
  }
  await command("nb_stop 1")
  await command("tf_bot_add 2 spy blue easy")
  await expect(root).toHaveAttribute("data-bot-count", "2")
  await command("tf_bot_add 2 scout red easy")
  await expect(root).toHaveAttribute("data-bot-count", "4")
  await command("setpos 512 1420 586")
  let mousePosition: { x: number; y: number } | undefined
  const capturePointer = async (yaw: number, pitch = 0) => {
    await page.bringToFront(); await canvas.focus()
    const box = (await canvas.boundingBox())!
    if (await root.getAttribute("data-pointer-locked") !== "true") {
      await canvas.click(); mousePosition = { x: box.x + box.width / 2, y: box.y + box.height / 2 }
    } else if (!mousePosition) {
      mousePosition = { x: box.x + box.width / 2, y: box.y + box.height / 2 }
      await page.mouse.move(mousePosition.x, mousePosition.y)
      await page.waitForTimeout(35)
    }
    await expect(root).toHaveAttribute("data-pointer-locked", "true")
    const current = await root.evaluate(element => ({ yaw: Number(element.dataset.cameraYaw), pitch: Number(element.dataset.cameraPitch) }))
    const wrap = (v: number) => ((v + 180) % 360 + 360) % 360 - 180
    mousePosition = { x: mousePosition!.x + wrap(current.yaw - yaw) / 0.066, y: mousePosition!.y + (pitch - current.pitch) / 0.066 }
    await page.mouse.move(mousePosition.x, mousePosition.y)
    await expect.poll(async () => Math.abs(wrap(Number(await root.getAttribute("data-camera-yaw")) - yaw))).toBeLessThan(0.1)
  }
  await capturePointer(270)
  const roster = await capture("roster")
  const spies = roster.players.filter((player: any) => player.fake && player.class === 8)
  expect(spies).toHaveLength(2)
  for (const [index, spy] of spies.entries()) {
    const x = 470 + index * 84
    await command(`bot_teleport "${spy.name}" ${x} 1300 586 0 90 0`)
    await expect.poll(async () => (await root.getAttribute("data-bot-probe"))?.split("|").find(row => row.startsWith(`${spy.identity}:`))?.split(":")[6]).toBe(`${x}.0,1300.0,586.0`)
  }
  await capturePointer(270)
  const visible = await capture("enemy-uncloaked")
  expect(visible.worldGeometry.samples.length).toBeGreaterThan(0)
  await command(`bot_command "${spies[0].name}" addcond 4`)
  await page.waitForTimeout(1100)
  await capturePointer(270)
  const actor = 0x60000000 + spies[0].identity
  const hidden = await capture("enemy-cloaked-one", { identity: actor, factor: 1 })
  expect(hidden.models.find((model: any) => model.identity === actor).cloak.worldFactor).toBe(1)
  expect(hidden.worldGeometry.samples.some((sample: any) => sample.identity === actor || sample.identity === actor + 0x10000)).toBe(false)
  expect(hidden.models.find((model: any) => model.identity === 0x60000000 + spies[1].identity).cloak.worldFactor).toBe(0)
  const target = hidden.bots.find((bot: any) => bot.identity === spies[0].identity).position
  const delta = [target[0] - hidden.camera.position[0], target[1] - hidden.camera.position[1], target[2] + 48 - hidden.camera.position[2]]
  await capturePointer((Math.atan2(delta[1], delta[0]) * 180 / Math.PI + 360) % 360, -Math.atan2(delta[2], Math.hypot(delta[0], delta[1])) * 180 / Math.PI)
  await page.mouse.down(); await page.mouse.up()
  const revealed = await capture("enemy-damage-reveal", { identity: actor, factor: Math.fround(0.85) })
  expect(revealed.bots.find((bot: any) => bot.identity === spies[0].identity).health).toBeLessThan(125)
  await capture("enemy-rehidden", { identity: actor, factor: 1 })
  await command(`bot_command "${spies[1].name}" addcond 4`)
  await page.waitForTimeout(1100)
  await command("jointeam blue")
  await expect(root).toHaveAttribute("data-team-selection-local", "3")
  await expect(root).toHaveAttribute("data-class-selection-visible", "true", { timeout: 5000 })
  await page.keyboard.press("Digit9")
  await expect(root).toHaveAttribute("data-class-selection-visible", "false")
  await command("setpos 512 1420 586")
  await capturePointer(270)
  const allied = await capture("allied-cloaked-two", { identity: actor, factor: Math.fround(0.95) })
  expect(allied.models.find((model: any) => model.identity === actor).cloak.worldFactor).toBe(Math.fround(0.95))
  expect(allied.worldGeometry.samples.some((sample: any) => sample.identity === actor)).toBe(true)
  await command(`bot_command "${spies[0].name}" removecond 4`)
  await capturePointer(270)
  const independent = await capture("allied-independent", { identity: actor, factor: 0 })
  expect(independent.models.find((model: any) => model.identity === actor).cloak.worldFactor).toBe(0)
  expect(independent.models.find((model: any) => model.identity === 0x60000000 + spies[1].identity).cloak.worldFactor).toBe(Math.fround(0.95))
  await command(`bot_teleport "${spies[1].name}" 470 1000 586 0 90 0`)
  await expect.poll(async () => (await root.getAttribute("data-bot-probe"))?.split("|").find(row => row.startsWith(`${spies[1].identity}:`))?.split(":")[6]).toBe("470.0,1000.0,586.0")
  await capturePointer(270)
  const occluded = await capture("allied-behind-door")
  expect(occluded.models.find((model: any) => model.identity === 0x60000000 + spies[1].identity).cloak.worldFactor).toBe(Math.fround(0.95))
  expect(occluded.worldGeometry.samples.some((sample: any) => sample.identity === 0x60000000 + spies[1].identity)).toBe(false)
  await page.setViewportSize({ width: 1100, height: 760 })
  await capture("resized")
  const measurements: unknown[] = []
  if (process.env.PROFILE_CLOAK_BOTS_ONLY !== "1") {
    // Restore an authored friendly spawn for the ordinary live-player workload;
    // keep the real cloak shader active during both full-roster samples.
    const spawn = roster.bots.find((bot: any) => bot.team === 3)
    await command(`setpos ${spawn.position.join(" ")}`)
    await capturePointer(spawn.yawDegrees)
  }
  await command("nb_stop 0")
  for (const [add, count] of (process.env.PROFILE_CLOAK_BOTS_ONLY === "1" ? [] : [[11, 15], [8, 23]]) as (readonly [number, number])[]) {
    await command(`tf_bot_add ${add} easy`)
    await expect(root).toHaveAttribute("data-bot-count", String(count))
    await capturePointer(270)
    if (count === 15) {
      await toggleCloak()
      await expect.poll(async () => (await root.getAttribute("data-spy-probe"))?.split(":")[1]).toBe("1.000")
    }
    const start = await page.evaluate(() => {
      const profiler = (globalThis as any).__playsrcFrameProfiler
      profiler.active = true; profiler.completedFrames = []
      Object.assign((globalThis as any).__playsrcProfile, { cloakSampleActive: true, cloakSampleCopies: 0 })
      return { at: performance.now(), tick: Number(document.querySelector<HTMLElement>("main")!.dataset.snapshotTick) }
    })
    console.log(`[spy-cloak sampling] ${count} bots, 5 seconds`)
    await page.waitForTimeout(5000)
    const measurement = await page.evaluate(start => {
      const profiler = (globalThis as any).__playsrcFrameProfiler
      profiler.active = false
      ;(globalThis as any).__playsrcProfile.cloakSampleActive = false
      const elapsed = performance.now() - start.at
      return { elapsed, simHz: (Number(document.querySelector<HTMLElement>("main")!.dataset.snapshotTick) - start.tick) * 1000 / elapsed, frames: profiler.completedFrames, cloakCopies: (globalThis as any).__playsrcProfile.cloakSampleCopies, state: { ...document.querySelector<HTMLElement>("main")!.dataset } }
    }, start)
    measurements.push({ count, ...measurement })
    expect(measurement.simHz).toBeGreaterThan(60)
    expect(measurement.cloakCopies).toBeGreaterThan(0)
  }
  await writeFile(path.join(directory, "report.json"), JSON.stringify({ captures, inputs, measurements }, (_, value) => typeof value === "bigint" ? value.toString() : value, 2))
})
