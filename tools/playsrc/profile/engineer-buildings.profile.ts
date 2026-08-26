import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
import { expect, test } from "./application-test"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"
import { activeGameplayTraceWindow, summarizeActivePresentationSilence, summarizeCompositorTruth, type ChromiumTraceEvent } from "./compositor-truth"

test("authored Engineer build menus, stock objects and headed building pixels", async ({ page, context }, testInfo) => {
  await page.addInitScript(installBrowserFrameProfiler)
  await page.addInitScript(() => { ;(globalThis as typeof globalThis & { __playsrcProfile?: object }).__playsrcProfile = {} })
  await page.goto("/")
  await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu", { timeout: 120_000 })
  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  await entry.fill("map pl_upward")
  await entry.press("Enter")
  await page.waitForFunction(()=>{const main=document.querySelector<HTMLElement>("main");return main?.dataset.teamSelectionVisible==="true"||main?.dataset.phase==="Ready"||main?.dataset.phase==="Failed"},undefined,{timeout:600_000,polling:50})
  await expect(page.locator("main")).toHaveAttribute("data-team-selection-visible", "true")
  if(await page.locator("main").getAttribute("data-console-visible")==="true")await page.keyboard.press("Backquote")
  await page.locator(".team-selection-layer [data-vgui-name='teambutton1']").click()
  await expect(page.locator("main")).toHaveAttribute("data-team-selection-visible", "false")
  await expect(page.locator("main")).toHaveAttribute("data-phase", "Ready", { timeout: 120_000 })
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "true")
  await page.keyboard.press("Digit6")
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "false")
  await expect(page.locator("main")).toHaveAttribute("data-engineer-metal", "200", { timeout: 60_000 })
  await page.keyboard.press("Digit4")
  await expect(page.locator("main")).toHaveAttribute("data-engineer-menu", "build")
  await expect(page.locator(".engineer-layer [data-vgui-name='HudMenuEngyBuild']")).toBeVisible()
  await expect(page.locator(".engineer-layer [data-vgui-name='AccountValue']")).toHaveText("200")
  const authoredIcons=await page.locator(".engineer-layer [data-vgui-name='BuildingIcon']").count()
  expect(authoredIcons).toBeGreaterThanOrEqual(4)
  const menu = await page.screenshot()
  await testInfo.attach("headed-authored-engineer-build-menu", { body: menu, contentType: "image/png" })
  await page.keyboard.press("Digit1")
  await expect(page.locator("main")).toHaveAttribute("data-engineer-menu", "none")
  await page.waitForFunction(() => (document.querySelector<HTMLElement>("main")?.dataset.placement ?? "").startsWith("2:0:"), undefined, { timeout: 30_000 })
  let pointerX = 640
  const capturePointer = async () => {
    if (await page.locator("main").getAttribute("data-pointer-locked") === "true") return
    await page.bringToFront()
    pointerX = 640
    await page.locator(".world-canvas").click({ position: { x: 640, y: 360 } })
    await expect(page.locator("main")).toHaveAttribute("data-pointer-locked", "true", { timeout: 5_000 })
    expect(await page.evaluate(() => document.pointerLockElement === document.querySelector(".world-canvas"))).toBe(true)
  }
  const walkToValidGround = async (prefix: string) => {
    await capturePointer()
    if (!(await page.locator("main").getAttribute("data-placement"))?.startsWith(`${prefix}:0:`)) return
    const initial = (await page.locator("main").getAttribute("data-camera-position"))!.split(",").map(Number)
    const leaveFortSpawn = initial[0]! < 0 && initial[2]! > 200
    const currentYaw = Number(await page.locator("main").getAttribute("data-camera-yaw"))
    const normalizedYaw = ((currentYaw % 360) + 360) % 360
    const signedYaw = normalizedYaw > 180 ? normalizedYaw - 360 : normalizedYaw
    pointerX = Math.max(5, Math.min(1275, Math.round(640 + signedYaw / 0.066)))
    await page.mouse.move(pointerX, 360)
    await expect.poll(async () => {
      const yaw = Number(await page.locator("main").getAttribute("data-camera-yaw"))
      return Math.min(Math.abs(yaw), Math.abs(yaw - 360))
    }, { timeout: 2_000 }).toBeLessThan(5)
    const route: readonly (readonly [number, number])[] = initial[0]! < 0
      ? initial[2]! > 200 ? [[0, -1760], [1, -1440], [0, -800]] : [[1, -1260], [0, -1980], [1, -1460], [0, -1780]]
      : initial[1]! > 1000 ? [[0, 512], [1, 1230]] : [[1, 560], [0, 810]]
    for (const [axis, destination] of route) {
      if (!leaveFortSpawn && (await page.locator("main").getAttribute("data-placement"))?.startsWith(`${prefix}:1:`)) break
      if (leaveFortSpawn && axis === 0 && destination === -800) {
        for (let correction = 0; correction < 12; correction += 1) {
          const lateral = Number((await page.locator("main").getAttribute("data-camera-position"))!.split(",")[1])
          if (Math.abs(lateral + 1432) <= 8) break
          const key = lateral < -1432 ? "a" : "d"
          await page.keyboard.down(key)
          await page.waitForTimeout(60)
          await page.keyboard.up(key)
          await page.waitForTimeout(90)
        }
        expect(Math.abs(Number((await page.locator("main").getAttribute("data-camera-position"))!.split(",")[1]) + 1432)).toBeLessThanOrEqual(8)
        let correcting: "a" | "d" | undefined
        await page.keyboard.down("w")
        const began = Date.now()
        try {
          while (true) {
            const [forward, lateral] = (await page.locator("main").getAttribute("data-camera-position"))!.split(",").map(Number)
            if (forward! >= destination - 8) break
            if (Date.now() - began > 8_000) throw new Error(`Fort spawn exit remained obstructed: position=${forward},${lateral}`)
            const next = lateral! < -1440 ? "a" : lateral! > -1424 ? "d" : undefined
            if (next !== correcting) {
              if (correcting) await page.keyboard.up(correcting)
              if (next) await page.keyboard.down(next)
              correcting = next
            }
            await page.waitForTimeout(25)
          }
        } finally {
          if (correcting) await page.keyboard.up(correcting)
          await page.keyboard.up("w")
        }
        continue
      }
      const current = Number((await page.locator("main").getAttribute("data-camera-position"))!.split(",")[axis])
      const increasing = destination > current
      const yaw = Number(await page.locator("main").getAttribute("data-camera-yaw")) * Math.PI / 180
      const desired = (axis === 0 ? increasing ? [1, 0] : [-1, 0] : increasing ? [0, 1] : [0, -1]) as [number, number]
      const directions: readonly (readonly [string, number, number])[] = [["w", Math.cos(yaw), Math.sin(yaw)], ["s", -Math.cos(yaw), -Math.sin(yaw)], ["a", -Math.sin(yaw), Math.cos(yaw)], ["d", Math.sin(yaw), -Math.cos(yaw)]]
      const keys = directions.filter(([, x, y]) => x * desired[0] + y * desired[1] > 0.45).map(([key]) => key)
      for (const key of keys) await page.keyboard.down(key)
      try {
        await page.waitForFunction(({ axis, target, increasing, prefix, leaveFortSpawn }) => {
          const root = document.querySelector<HTMLElement>("main")!
          const value = Number(root.dataset.cameraPosition?.split(",")[axis])
          return !leaveFortSpawn && root.dataset.placement?.startsWith(`${prefix}:1:`) || (increasing ? value >= target - 8 : value <= target + 8)
        }, { axis, target: destination, increasing, prefix, leaveFortSpawn }, { timeout: 8_000, polling: 20 })
      } catch {
        throw new Error(`Ordinary walking did not reach valid authored ground: axis=${axis}; keys=${keys}; initial=${initial}; current=${await page.locator("main").getAttribute("data-camera-position")}; placement=${await page.locator("main").getAttribute("data-placement")}`)
      } finally {
        for (const key of keys) await page.keyboard.up(key)
      }
    }
    if (leaveFortSpawn) expect(Number((await page.locator("main").getAttribute("data-camera-position"))!.split(",")[0])).toBeGreaterThan(-880)
  }
  await walkToValidGround("2:0")
  await expect(page.locator("main")).toHaveAttribute("data-placement", /^2:0:1:/)
  const blueprint = await page.locator(".world-canvas").screenshot()
  await testInfo.attach("headed-authored-valid-sentry-blueprint", { body: blueprint, contentType: "image/png" })
  await page.mouse.click(pointerX, 360)
  await expect(page.locator("main")).toHaveAttribute("data-building-count", "1",{timeout:5000})
  await expect(page.locator("main")).toHaveAttribute("data-engineer-metal", "70")
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile?.buildings?.length === 1)
  const constructing = await page.evaluate(() => (globalThis as any).__playsrcProfile.buildings[0])
  expect(constructing.object).toEqual({ kind: 2, mode: 0 })
  expect(constructing.owner).toBe(1)
  expect(constructing.team).toBe(2)

  const seconds = profileSampleSeconds()
  const cdp = await context.newCDPSession(page)
  const traceEvents: ChromiumTraceEvent[] = []
  cdp.on("Tracing.dataCollected", ({ value }) => traceEvents.push(...value))
  await cdp.send("Tracing.start", { categories: "benchmark,viz,gpu,blink.user_timing", options: "record-as-much-as-possible" })
  const measurement = await page.evaluate(async (duration) => {
    const root = document.querySelector<HTMLElement>("main")!
    const started = performance.now(), firstTick = Number(root.dataset.snapshotTick)
    performance.mark("playsrc-active-gameplay-start")
    ;(globalThis as any).__playsrcFrameProfiler.active = true
    let previous = started
    const frames: number[] = []
    await new Promise<void>(resolve => {
      const frame = (now: number) => { frames.push(now - previous); previous = now; if (now - started >= duration * 1000) resolve(); else requestAnimationFrame(frame) }
      requestAnimationFrame(frame)
    })
    performance.mark("playsrc-active-gameplay-end")
    ;(globalThis as any).__playsrcFrameProfiler.active = false
    return { seconds: (performance.now() - started) / 1000, firstTick, lastTick: Number(root.dataset.snapshotTick), frames }
  }, seconds)
  const traceComplete = new Promise<void>(resolve => cdp.once("Tracing.tracingComplete", () => resolve()))
  await cdp.send("Tracing.end")
  await traceComplete
  const traceDirectory = path.join((await loadLocalConfig()).sourceCacheDir, "evidence", "tf2-engineer-buildings")
  await mkdir(traceDirectory, { recursive: true })
  await writeFile(path.join(traceDirectory, "engineer.trace.json"), JSON.stringify({ traceEvents }))
  const window = activeGameplayTraceWindow(traceEvents)
  const compositor = summarizeCompositorTruth(traceEvents, measurement.seconds * 1000, window)
  const silence = summarizeActivePresentationSilence(traceEvents, window)
  expect(measurement.lastTick - measurement.firstTick).toBeGreaterThan(seconds * 60)
  await expect.poll(async () => page.evaluate(() => (globalThis as any).__playsrcProfile.buildings[0]?.phase), { timeout: 12_000 }).toBe(1)
  await page.keyboard.down("w")
  await page.waitForTimeout(140)
  await page.keyboard.up("w")
  await page.mouse.click(pointerX, 360)
  await expect.poll(async () => page.evaluate(() => (globalThis as any).__playsrcProfile.buildings[0]?.upgradeMetal), { timeout: 3_000 }).toBe(25)
  await expect(page.locator("main")).toHaveAttribute("data-engineer-metal", "45")
  await expect(page.locator("main")).toHaveAttribute("data-viewmodel-activity", /IDLE/, { timeout: 5000 })
  await page.evaluate(() => { (globalThis as any).__playsrcProfile.worldLightingEvidenceRevision = 1 })
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.worldLighting?.revision === 1)
  const sentrySamples = await page.evaluate(identity => (globalThis as any).__playsrcProfile.worldLighting.worldGeometry.samples.filter((sample: any) => sample.identity === identity), constructing.identity)
  expect(sentrySamples.length, "constructed sentry has visible world-depth samples").toBeGreaterThan(0)
  const built = await page.locator(".world-canvas").screenshot()
  await testInfo.attach("headed-authored-constructed-sentry", { body: built, contentType: "image/png" })

  await page.keyboard.press("Digit5")
  await expect(page.locator("main")).toHaveAttribute("data-engineer-menu", "destroy")
  await page.keyboard.press("Digit1")
  await expect(page.locator("main")).toHaveAttribute("data-building-count", "0")
  await page.keyboard.press("Digit3")
  await expect(page.locator("main")).toHaveAttribute("data-viewmodel-activity", /IDLE/, { timeout: 5000 })
  await page.evaluate(() => { (globalThis as any).__playsrcProfile.worldLightingEvidenceRevision = 2 })
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.worldLighting?.revision === 2)
  expect(await page.evaluate(identity => (globalThis as any).__playsrcProfile.worldLighting.worldGeometry.samples.some((sample: any) => sample.identity === identity), constructing.identity)).toBe(false)
  const removed = await page.locator(".world-canvas").screenshot()
  await testInfo.attach("headed-sentry-destroyed", { body: removed, contentType: "image/png" })
  const objects: Array<{ team: number; object: { kind: number; mode: number }; metalBefore: number; metalAfter: number; position: number[] }> = [{
    team: 2, object: constructing.object, metalBefore: 200, metalAfter: 70, position: constructing.position,
  }]
  const place = async (slot: number, object: { kind: number; mode: number }, team: 2 | 3, cost: number) => {
    const metalBefore = Number(await page.locator("main").getAttribute("data-engineer-metal"))
    await page.keyboard.press("Digit4")
    await expect(page.locator("main")).toHaveAttribute("data-engineer-menu", "build")
    await page.keyboard.press(`Digit${slot}`)
    const prefix = `${object.kind}:${object.mode}`
    await expect(page.locator("main")).toHaveAttribute("data-placement", new RegExp(`^${prefix}:`))
    await walkToValidGround(prefix)
    await expect(page.locator("main")).toHaveAttribute("data-placement", new RegExp(`^${prefix}:1:`))
    await page.mouse.click(pointerX, 360)
    await expect(page.locator("main")).toHaveAttribute("data-building-count", "1", { timeout: 5_000 })
    await expect(page.locator("main")).toHaveAttribute("data-engineer-metal", String(metalBefore - cost))
    const building = await page.evaluate(() => (globalThis as any).__playsrcProfile.buildings[0])
    expect(building.object).toEqual(object)
    expect(building.owner).toBe(1)
    expect(building.team).toBe(team)
    objects.push({ team, object, metalBefore, metalAfter: metalBefore - cost, position: building.position })
    await page.keyboard.press("Digit5")
    await expect(page.locator("main")).toHaveAttribute("data-engineer-menu", "destroy")
    await page.keyboard.press(`Digit${slot}`)
    await expect(page.locator("main")).toHaveAttribute("data-building-count", "0")
  }
  const renewEngineer = async () => {
    await page.keyboard.press("Comma")
    await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "true")
    await expect(page.locator("main")).toHaveAttribute("data-pointer-locked", "false")
    await page.keyboard.press("Digit1")
    await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "false")
    await expect(page.locator("main")).toHaveAttribute("data-engineer-metal", "100")
    await page.keyboard.press("Comma")
    await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "true")
    await page.keyboard.press("Digit6")
    await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "false")
    await expect(page.locator("main")).toHaveAttribute("data-engineer-metal", "200")
  }
  await renewEngineer()
  await place(2, { kind: 0, mode: 0 }, 2, 100)
  await place(3, { kind: 1, mode: 0 }, 2, 50)
  await place(4, { kind: 1, mode: 1 }, 2, 50)

  await page.keyboard.press("Backquote")
  await entry.fill("map ctf_2fort")
  await entry.press("Enter")
  await page.waitForFunction(() => document.querySelector<HTMLElement>("main")?.dataset.teamSelectionVisible === "true", undefined, { timeout: 60_000 })
  if (await page.locator("main").getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  await expect(page.locator("main")).toHaveAttribute("data-team-selection-visible", "true")
  await page.locator(".team-selection-layer [data-vgui-name='teambutton0']").click()
  await expect(page.locator("main")).toHaveAttribute("data-team-selection-local", "3")
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "true")
  await page.keyboard.press("Digit6")
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "false")
  await expect(page.locator("main")).toHaveAttribute("data-engineer-metal", "200")
  await place(1, { kind: 2, mode: 0 }, 3, 130)
  await place(3, { kind: 1, mode: 0 }, 3, 50)
  await renewEngineer()
  await place(2, { kind: 0, mode: 0 }, 3, 100)
  await place(4, { kind: 1, mode: 1 }, 3, 50)
  expect(objects.map(value => `${value.team}:${value.object.kind}:${value.object.mode}`)).toEqual([
    "2:2:0", "2:0:0", "2:1:0", "2:1:1", "3:2:0", "3:1:0", "3:0:0", "3:1:1",
  ])
  const presentPixels=decodeScreenshot(built),removedPixels=decodeScreenshot(removed)
  let changedBuildingPixels=0,redBuildingPixels=0
  for (const sample of sentrySamples) {
    const centerX = Math.round((sample.x + 1) * presentPixels.width / 2)
    const centerY = Math.round((1 - sample.y) * presentPixels.height / 2)
    for (let y = centerY - 4; y < centerY + 4; y++) for (let x = centerX - 4; x < centerX + 4; x++) {
      const offset = (y * presentPixels.width + x) * presentPixels.channels
      const delta = Math.abs(presentPixels.pixels[offset]! - removedPixels.pixels[offset]!) + Math.abs(presentPixels.pixels[offset + 1]! - removedPixels.pixels[offset + 1]!) + Math.abs(presentPixels.pixels[offset + 2]! - removedPixels.pixels[offset + 2]!)
      if (delta > 36) { changedBuildingPixels++; if (presentPixels.pixels[offset]! > presentPixels.pixels[offset + 2]! + 8) redBuildingPixels++ }
    }
  }
  expect(changedBuildingPixels, "visible sentry pixels disappear at unchanged-camera depth witnesses").toBeGreaterThan(Math.max(8, sentrySamples.length * 4))
  const losses = await page.evaluate(() => (globalThis as any).__playsrcFrameProfiler.losses)
  const report = { schema: "playsrc-tf2-headed-engineer-buildings-v2", headed: true, targets: ["pl_upward", "ctf_2fort"], compositor, silence, losses, input: "native-pointer-lock-and-trusted-mouse", buildings: objects, pixels:{changedBuildingPixels,redBuildingPixels,authoredIcons},
    simulation: { seconds: Number(measurement.seconds.toFixed(3)), ticksPerSecond: Number(((measurement.lastTick - measurement.firstTick) / measurement.seconds).toFixed(2)) },
    frames: summarizeFrameTimes(measurement.frames), screenshots: ["authored-build-menu", "sentry-blueprint", "constructed-sentry", "destroyed-sentry"] }
  const local = await loadLocalConfig(), directory = path.join(local.sourceCacheDir, "evidence", "tf2-engineer-buildings")
  await mkdir(directory, { recursive: true })
  await Promise.all([writeFile(path.join(directory, "pl_upward-engineer.json"), `${JSON.stringify(report, null, 2)}\n`), writeFile(path.join(directory, "build-menu.png"), menu), writeFile(path.join(directory, "sentry-blueprint.png"), blueprint), writeFile(path.join(directory, "sentry-built.png"), built), writeFile(path.join(directory, "sentry-destroyed.png"), removed)])
  console.log(`[engineer-buildings] ${JSON.stringify(report)}`)
  expect(compositor.evidence).toBe("chromium-compositor-presentation-trace")
  expect(silence.maximumActiveSilenceMilliseconds).toBeLessThan(250)
  expect(losses).toEqual([])
})
