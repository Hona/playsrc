import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
import { expect, test } from "./application-test"
import { chooseTf2Team } from "./team-selection-evidence"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"

const DISPLACEMENT_SOURCES = Object.freeze([11, 32, 34, 37, 41, 42, 43, 44, 63, 65, 66, 138, 147, 381])

test("Upward spawn-to-outdoor noclip retains depth-tested authored terrain", async ({ page }, testInfo) => {
  await page.addInitScript((sources) => {
    ;(globalThis as typeof globalThis & { __playsrcProfile?: object }).__playsrcProfile = {
      displacementSources: sources,
    }
  }, [...DISPLACEMENT_SOURCES])
  await page.goto("/")
  const main = page.locator("main")
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 120_000 })
  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  const command = async (value: string): Promise<void> => {
    await entry.fill(value)
    await entry.press("Enter")
  }
  await command("map pl_upward")
  await page.waitForFunction(() => {
    const root = document.querySelector<HTMLElement>("main")
    return root?.dataset.teamSelectionVisible === "true"
      || root?.dataset.phase === "Ready" || root?.dataset.phase === "Failed"
  }, undefined, { timeout: 600_000 })
  if (await main.getAttribute("data-team-selection-visible") === "true") {
    if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await chooseTf2Team(page, "blue")
    await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 60_000 })
    await page.keyboard.press("Backquote")
  } else {
    await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 60_000 })
    await command("jointeam blue")
  }
  await command("tf_bot_add blue soldier normal")
  await expect(main).toHaveAttribute("data-bot-count", "1", { timeout: 30_000 })
  await command("noclip")
  await command("setpos -2528 -1360 17")
  await page.waitForFunction(() => {
    const position = document.querySelector<HTMLElement>("main")?.dataset.cameraPosition?.split(",").map(Number)
    return position?.length === 3 && Math.abs(position[0]! + 2528) < 2 && Math.abs(position[1]! + 1360) < 2
  })
  await page.keyboard.press("Backquote")
  await expect(main).toHaveAttribute("data-movement-mode", "1")
  await page.waitForFunction((count) => (globalThis as any).__playsrcProfile?.displacements?.length === count, DISPLACEMENT_SOURCES.length)

  const local = await loadLocalConfig()
  const directory = path.join(local.sourceCacheDir, "evidence", "tf2-upward-outdoor-floor")
  await mkdir(directory, { recursive: true })
  const canvas = page.locator("canvas.world-canvas")
  const spawn = (await main.getAttribute("data-camera-position"))?.split(",").map(Number)
  if (!spawn || spawn.length !== 3) throw new Error("authored Upward spawn camera is unavailable")
  await writeFile(path.join(directory, "spawn.png"), await canvas.screenshot())

  await page.evaluate(async (origin) => {
    const root = document.querySelector<HTMLElement>("main")!
    window.dispatchEvent(new KeyboardEvent("keydown", { code: "KeyW", key: "w", bubbles: true }))
    try {
      const started = performance.now()
      while (performance.now() - started < 2_000) {
        await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()))
        const current = (root.dataset.cameraPosition ?? "").split(",").map(Number)
        if (current.length === 3 && Math.hypot(...current.map((value, axis) => value - origin[axis]!)) > 800) break
      }
    } finally {
      window.dispatchEvent(new KeyboardEvent("keyup", { code: "KeyW", key: "w", bubbles: true }))
    }
  }, spawn)
  const outdoor = (await main.getAttribute("data-camera-position"))?.split(",").map(Number)
  if (!outdoor || outdoor.length !== 3) throw new Error("authored Upward outdoor camera is unavailable")
  expect(Math.hypot(...outdoor.map((value, axis) => value - spawn[axis]!))).toBeGreaterThan(100)

  const positions = [
    { name: "spawn-exit", position: outdoor, yawDegrees: 0, pitchDegrees: 18 },
    { name: "blue-spawn-door", position: [-1850, -1536, 132], yawDegrees: 0, pitchDegrees: 18 },
    { name: "payload-yard", position: [-1674, -1536, 150], yawDegrees: 0, pitchDegrees: 18 },
    { name: "first-hill", position: [-1400, -1500, 210], yawDegrees: 0, pitchDegrees: 18 },
    { name: "lower-yard", position: [-1600, -2000, 150], yawDegrees: 0, pitchDegrees: 20 },
    { name: "coal-yard", position: [-128, 640, 600], yawDegrees: 0, pitchDegrees: 35 },
    { name: "upper-approach", position: [32, 1152, 540], yawDegrees: 180, pitchDegrees: 35 },
    { name: "summit-approach", position: [1900, -300, 560], yawDegrees: 0, pitchDegrees: 18 },
  ] as const
  const locations: Array<Record<string, unknown>> = []
  for (let index = 0; index < positions.length; index += 1) {
    const location = positions[index]!
    await page.evaluate(({ location, revision }) => {
      const profile = (globalThis as any).__playsrcProfile
      profile.displacementCameraOverride = location
      profile.geometryEvidenceRevision = revision
    }, { location, revision: 90_100 + index })
    await page.waitForFunction(({ position, revision }) => {
      const profile = (globalThis as any).__playsrcProfile
      return document.querySelector<HTMLElement>(".world-canvas")?.dataset.displayCameraPosition === position.join(",")
        && profile.geometryEvidence?.revision === revision
    }, { position: location.position, revision: 90_100 + index }, { timeout: 30_000 })
    const bytes = await canvas.screenshot()
    await writeFile(path.join(directory, `${location.name}.png`), bytes)
    await testInfo.attach(`headed-upward-${location.name}`, { body: bytes, contentType: "image/png" })
    const screenshot = decodeScreenshot(bytes)
    const pixels = { lowerSamples: 0, skyLike: 0, dark: 0, earthy: 0 }
    for (let y = Math.floor(screenshot.height * 0.6); y < screenshot.height - 30; y += 4) {
      for (let x = 80; x < screenshot.width - 80; x += 4) {
        const offset = (y * screenshot.width + x) * screenshot.channels
        const red = screenshot.pixels[offset]!, green = screenshot.pixels[offset + 1]!, blue = screenshot.pixels[offset + 2]!
        pixels.lowerSamples += 1
        pixels.skyLike += Number(blue > red * 1.04 && blue > green * 0.95 && blue > 70)
        pixels.dark += Number(red + green + blue < 30)
        pixels.earthy += Number(red > blue * 1.08 && green > blue * 0.9)
      }
    }
    const state = await page.evaluate(() => {
      const root = document.querySelector<HTMLElement>("main")!
      const surface = document.querySelector<HTMLElement>(".world-canvas")!
      const profile = (globalThis as any).__playsrcProfile
      const visible = new Set<number>(profile.displacementVisibility.drawSurfaces)
      const waterMaterial = profile.materialAnimation.materials.find((material: any) =>
        material.identity === "materials/maps/pl_upward/water/water_hydro_cheap_dx80_7168_-2048_128.vmt")
      return {
        camera: profile.displacementCamera,
        geometry: profile.geometryEvidence.geometry.samples,
        visibility: {
          eyeLeaf: profile.displacementVisibility.eyeLeaf,
          outsideWorld: profile.displacementVisibility.outsideWorld,
          leaves: profile.displacementVisibility.leaves.length,
          areas: profile.displacementVisibility.areas,
          worldFaces: profile.displacementVisibility.surfaces.length,
          drawnFaces: profile.displacementVisibility.drawSurfaces.length,
        },
        displacements: profile.displacements.map((displacement: any) => ({
          source: displacement.source,
          face: displacement.face,
          material: displacement.material,
          visible: visible.has(displacement.face),
          submittedTriangles: displacement.submittedTriangles,
          cull: displacement.cull,
          depthTest: displacement.depthTest,
          depthWrite: displacement.depthWrite,
        })),
        sky: surface.dataset.sky3dPass ? JSON.parse(surface.dataset.sky3dPass) : null,
        skyDisposition: surface.dataset.skyVisibilityDisposition,
        props: JSON.parse(surface.dataset.visibleMainStaticProps ?? "[]").length,
        bots: Number(root.dataset.botCount),
        water: root.dataset.waterPlan,
        authoredWater: waterMaterial ? {
          identity: waterMaterial.identity,
          frameCount: waterMaterial.textures.find((texture: any) => texture.role === 7)?.frameCount,
        } : null,
      }
    })
    const lowerGeometry = state.geometry.filter((sample: any) => sample.y <= -0.4)
    const ground = lowerGeometry.filter((sample: any) => sample.disposition === "main-world")
    locations.push({ ...location, ...state, pixels, lowerGeometry, groundHits: ground.length })
  }

  const seconds = profileSampleSeconds()
  const measurement = await page.evaluate(async (duration) => {
    const root = document.querySelector<HTMLElement>("main")!
    const started = performance.now(), firstTick = Number(root.dataset.snapshotTick)
    const frames: number[] = [], world: number[] = []
    let previous = started
    await new Promise<void>((resolve) => {
      const next = (now: number) => {
        frames.push(now - previous)
        previous = now
        if (root.dataset.performanceDetail) world.push(JSON.parse(root.dataset.performanceDetail).world)
        if (now - started >= duration * 1000) resolve()
        else requestAnimationFrame(next)
      }
      requestAnimationFrame(next)
    })
    return { seconds: (performance.now() - started) / 1000, firstTick, lastTick: Number(root.dataset.snapshotTick), frames, world }
  }, seconds)
  const report = {
    schema: "playsrc-upward-outdoor-floor-profile-v1",
    target: "pl_upward",
    headed: true,
    spawn,
    outdoor,
    locations,
    simulation: {
      seconds: measurement.seconds,
      ticks: measurement.lastTick - measurement.firstTick,
      ticksPerSecond: (measurement.lastTick - measurement.firstTick) / measurement.seconds,
    },
    frames: summarizeFrameTimes(measurement.frames),
    world: summarizeFrameTimes(measurement.world),
  }
  await writeFile(path.join(directory, "report.json"), `${JSON.stringify(report, null, 2)}\n`)
  await testInfo.attach("headed-upward-outdoor-floor", { body: JSON.stringify(report, null, 2), contentType: "application/json" })
  console.log(`PLAYSRC_UPWARD_OUTDOOR_FLOOR ${JSON.stringify({ spawn, outdoor, locations: locations.map((location) => ({ name: location.name, groundHits: location.groundHits, pixels: location.pixels, visibility: location.visibility })), frames: report.frames, world: report.world, ticksPerSecond: report.simulation.ticksPerSecond })}`)
  expect(report.simulation.ticksPerSecond).toBeGreaterThan(60)
  expect(locations.every((location) => location.bots === 1)).toBe(true)
  expect(locations.every((location) => Number(location.props) > 0)).toBe(true)
  expect(locations.every((location) => (location.sky as { stateRestored?: boolean } | null)?.stateRestored === true)).toBe(true)
  expect(locations.every((location) => (location.authoredWater as { frameCount?: number } | null)?.frameCount === 30)).toBe(true)
  expect(locations.slice(1).every((location) => Number(location.groundHits) >= 8)).toBe(true)
  expect(report.frames.p95Milliseconds).toBeLessThan(20)
  expect(report.world.p95Milliseconds).toBeLessThan(5)
  const door = locations.find((location) => location.name === "blue-spawn-door")
  if (!door) throw new Error("the authored Blue spawn-to-outdoor area portal was not sampled")
  expect((door.visibility as { areas: number[] }).areas).toContain(2)
  expect((door.visibility as { areas: number[] }).areas).toContain(30)
  expect((door.displacements as Array<{ source: number; visible: boolean }>)
    .some((displacement) => displacement.source === 43 && displacement.visible)).toBe(true)
  expect(door.groundHits).toBe(10)
  expect((door.pixels as { skyLike: number }).skyLike).toBeLessThan(600)
})
