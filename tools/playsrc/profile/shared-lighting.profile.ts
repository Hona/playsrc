import { test, expect } from "./application-test"
import { loadLocalConfig, repositoryRoot } from "../src/config"
import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { decodeScreenshot } from "./screenshot-pixels"
import { sanityViewAngles } from "./map-sanity"
import { summarizeFrameTimes } from "./profile-window"

test("known-direction Source normals survive clockwise culling and bone/object transforms", async ({ page }, testInfo) => {
  page.on("pageerror", error => console.error(error))
  page.on("console", message => { if (message.type() === "error" || message.type() === "warning") console.error(message.text()) })
  await page.route("**/lighting-probe", route => route.fulfill({ contentType: "text/html", body: '<body style="margin:0;background:#222"><h1 style="color:white">Source authored normals: above / below / front</h1></body>' }))
  await page.goto("/lighting-probe")
  await page.evaluate(async (root) => {
    const { createLightingProbe } = await import(`${root}/packages/presentation/rendering/tests/fixtures/model-lighting-probe.ts`)
    ;(globalThis as any).__lightingProbe = await createLightingProbe()
  }, `/@fs${repositoryRoot.replace(/\/$/, "")}`)
  const output = path.join((await loadLocalConfig()).sourceCacheDir, "profiles", "shared-lighting", process.env.PROFILE_LIGHTING_LABEL ?? "current")
  await mkdir(output, { recursive: true })
  const failures: string[] = []
  try {
    for (const mode of ["above", "below", "front", "ambient", "normals"]) {
      const result = await page.evaluate(mode => (globalThis as any).__lightingProbe.draw(mode), mode) as Array<{ name: string; x: number; y: number; depth: number; expected: number[] }>
      const screenshot = await page.locator("canvas").screenshot()
      const image = decodeScreenshot(screenshot)
      const measured = result.map(probe => ({ ...probe, actual: Array.from(image.pixels.subarray((Math.round(probe.y) * image.width + Math.round(probe.x)) * image.channels, (Math.round(probe.y) * image.width + Math.round(probe.x)) * image.channels + 3)) }))
      await writeFile(path.join(output, `${mode}.png`), screenshot)
      await writeFile(path.join(output, `${mode}.json`), JSON.stringify(measured, null, 2))
      await testInfo.attach(mode, { body: screenshot, contentType: "image/png" })
      for (const probe of measured) {
        expect(probe.depth, probe.name).toBeGreaterThan(0)
        if (probe.actual.some((value, channel) => Math.abs(value - probe.expected[channel]!) > 4)) failures.push(`${mode}:${probe.name}: expected=${probe.expected} actual=${probe.actual}`)
      }
    }
  } finally { await page.evaluate(() => (globalThis as any).__lightingProbe.dispose()) }
  expect(failures.slice(0, 10), `${failures.length} incorrect normal/light probes`).toEqual([])
})

test("configured maps retain world, viewmodel, and panel illumination", async ({ page }) => {
  const output = path.join((await loadLocalConfig()).sourceCacheDir, "profiles", "shared-lighting", process.env.PROFILE_LIGHTING_LABEL ?? "current")
  await mkdir(output, { recursive: true })
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = {} })
  await page.goto("/", { waitUntil: "domcontentloaded" })
  const main = page.locator("main"), entry = page.locator("[aria-label='Console command']")
  await expect(main).toHaveAttribute("data-phase", "MainMenu")
  const command = async (text: string) => {
    if (await main.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    await entry.fill(text)
    await entry.press("Enter")
  }
  let revision = 0
  const capture = async (label: string) => {
    const requested = ++revision
    await page.evaluate(value => { (globalThis as any).__playsrcProfile.worldLightingEvidenceRevision = value }, requested)
    await page.waitForFunction(value => (globalThis as any).__playsrcProfile.worldLighting?.revision === value, requested)
    const evidence = await page.evaluate(() => ({
      lighting: (globalThis as any).__playsrcProfile.worldLighting,
      state: { ...document.querySelector<HTMLElement>("main")!.dataset },
      canvas: { ...document.querySelector<HTMLElement>("canvas.world-canvas")!.dataset },
    }))
    await page.locator("canvas.world-canvas").screenshot({ path: path.join(output, `${label}.png`) })
    await writeFile(path.join(output, `${label}.json`), JSON.stringify(evidence, null, 2))
    expect(evidence.lighting.viewmodel).not.toBeNull()
    expect(evidence.lighting.geometry.samples.length).toBeGreaterThan(0)
    expect(evidence.lighting.depthIsolated).toBe(true)
    return evidence
  }
  const aim = async (target: readonly number[]) => {
    if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await page.bringToFront()
    await page.locator("canvas.world-canvas").click()
    await expect(main).toHaveAttribute("data-pointer-locked", "true")
    const camera = (await main.getAttribute("data-camera-position"))!.split(",").map(Number) as [number, number, number]
    const angles = sanityViewAngles(camera, target as [number, number, number])
    await page.evaluate(angles => {
      const main = document.querySelector<HTMLElement>("main")!, canvas = document.querySelector<HTMLElement>("canvas.world-canvas")!
      const yaw = Number(canvas.dataset.displayCameraYaw ?? main.dataset.cameraYaw), pitch = Number(canvas.dataset.displayCameraPitch ?? main.dataset.cameraPitch)
      const event = new MouseEvent("mousemove", { bubbles: true })
      Object.defineProperties(event, {
        movementX: { value: (((yaw - angles.yaw + 180) % 360 + 360) % 360 - 180) / 0.066 },
        movementY: { value: (angles.pitch - pitch) / 0.066 },
      })
      dispatchEvent(event)
    }, angles)
    await expect.poll(async () => Math.abs(Number(await page.locator("canvas.world-canvas").getAttribute("data-display-camera-pitch")) - angles.pitch)).toBeLessThan(0.1)
  }
  const maps = ["jump_beef", "pl_upward", "ctf_2fort"].filter(map => !process.env.PROFILE_LIGHTING_TARGET || process.env.PROFILE_LIGHTING_TARGET.split(",").includes(map))
  expect(maps.length).toBeGreaterThan(0)
  for (const map of maps) {
    await command(`map ${map}`)
    await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 45_000 })
    if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await page.locator(".team-selection-layer [data-vgui-name='teambutton1']").click()
    await expect(main).toHaveAttribute("data-phase", "Ready")
    await expect(main).toHaveAttribute("data-class-selection-visible", "true")
    await expect.poll(() => main.getAttribute("data-class-selection-models"), { timeout: 15_000 }).toMatch(/TFPlayerModel:[^|]+:[0-9]+:[1-9][0-9]*/)
    await page.screenshot({ path: path.join(output, `${map}-panel.png`) })
    await page.keyboard.press("Digit2")
    await expect(main).toHaveAttribute("data-class-selection-visible", "false")
    const tick = Number(await main.getAttribute("data-snapshot-tick"))
    await page.waitForFunction(tick => Number(document.querySelector<HTMLElement>("main")!.dataset.snapshotTick) >= tick + 40, tick)
    await capture(`${map}-world`)
    if (map === "pl_upward") {
      await command("noclip")
      await expect(main).toHaveAttribute("data-movement-mode", "1")
      for (const [team, classname, classId] of [["red", "soldier", 3], ["blue", "engineer", 9]] as const) {
        await command(`tf_bot_add ${team} ${classname} normal`)
        await page.waitForFunction(id => (globalThis as any).__playsrcProfile.bots?.some((bot: any) => bot.class === id), classId)
        const target = await page.evaluate(id => {
          const profile = (globalThis as any).__playsrcProfile
          const bot = profile.bots.find((bot: any) => bot.class === id)
          const nearby = profile.coverageSamples.filter((sample: any) => {
            const distance = Math.hypot(...sample.position.map((v: number, i: number) => v - bot.position[i]))
            return distance >= 64 && distance <= 300
          }).sort((a: any, b: any) => Math.hypot(...a.position.map((v: number, i: number) => v - bot.position[i])) - Math.hypot(...b.position.map((v: number, i: number) => v - bot.position[i])))[0]
          if (!nearby) throw new Error("authored bot camera unavailable")
          return { identity: bot.identity, position: bot.position, camera: nearby.position }
        }, classId)
        const offset = Number((await main.getAttribute("data-view-offset"))!.split(",")[2])
        await command(`setpos ${target.camera[0]} ${target.camera[1]} ${target.camera[2] - offset}`)
        await aim([target.position[0], target.position[1], target.position[2] + 52])
        const evidence = await capture(`${map}-${team}-${classname}`)
        expect(evidence.lighting.worldGeometry.samples.some((sample: any) => sample.identity === 0x6000_0000 + target.identity)).toBe(true)
      }
    }
  }
  const cadence = await page.evaluate(() => new Promise<{ frames: number[]; ticks: number; milliseconds: number }>(resolve => {
    const main = document.querySelector<HTMLElement>("main")!, startTick = Number(main.dataset.snapshotTick), start = performance.now(), frames: number[] = []
    let previous = start
    const sample = (now: number) => {
      frames.push(now - previous); previous = now
      if (now - start < 5_000) requestAnimationFrame(sample)
      else resolve({ frames, ticks: Number(main.dataset.snapshotTick) - startTick, milliseconds: now - start })
    }
    requestAnimationFrame(sample)
  }))
  const performance = { ...summarizeFrameTimes(cadence.frames), simulationHz: cadence.ticks * 1000 / cadence.milliseconds }
  await writeFile(path.join(output, "cadence.json"), JSON.stringify(performance, null, 2))
  expect(performance.simulationHz).toBeGreaterThan(55)
  expect(performance.p95Milliseconds).toBeLessThan(20)
})
