import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
import { expect, test } from "./application-test"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"
import { chooseTf2Team } from "./team-selection-evidence"

type Viewpoint = Readonly<{
  name: string
  position: readonly [number, number, number]
  yawDegrees: number
  pitchDegrees: number
}>

test("headed ctf_2fort spawn, intelligence, bridge, courtyard, sewer, water, and sky presentation", async ({ page }, testInfo) => {
  await page.addInitScript(() => {
    ;(globalThis as typeof globalThis & { __playsrcProfile?: object }).__playsrcProfile = {}
  })
  await page.goto("/")
  const root = page.locator("main")
  await expect(root).toHaveAttribute("data-phase", "MainMenu", { timeout: 120_000 })
  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  const command = async (value: string): Promise<void> => {
    await entry.fill(value)
    await entry.press("Enter")
  }
  await command("map ctf_2fort")
  await page.waitForFunction(() => {
    const main = document.querySelector<HTMLElement>("main")
    return main?.dataset.teamSelectionVisible === "true" || main?.dataset.phase === "Ready" || main?.dataset.phase === "Failed"
  }, undefined, { timeout: 600_000 })
  if (await root.getAttribute("data-team-selection-visible") === "true") {
    if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await chooseTf2Team(page, "red")
  }
  await expect(root).toHaveAttribute("data-phase", "Ready", { timeout: 600_000 })
  if (await root.getAttribute("data-class-selection-visible") === "true") {
    await page.keyboard.press("Digit2")
    await expect(root).toHaveAttribute("data-class-selection-visible", "false")
  }
  if (await root.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
  await command("noclip")
  await expect(root).toHaveAttribute("data-movement-mode", "1")
  await command("tf_bot_add red scout normal")
  await expect(root).toHaveAttribute("data-bot-count", "1")
  await page.waitForFunction(() => {
    const bot = (globalThis as typeof globalThis & { __playsrcProfile?: any }).__playsrcProfile?.bots?.[0]
    return bot?.team === 2 && bot.class === 1 && bot.area !== null && bot.remainingPathAreas > 0
  }, undefined, { timeout: 30_000 })
  await page.keyboard.press("Backquote")

  const source = await page.evaluate(() => {
    const profile = (globalThis as typeof globalThis & { __playsrcProfile: any }).__playsrcProfile
    const main = document.querySelector<HTMLElement>("main")!
    return {
      spawn: (main.dataset.cameraPosition ?? "").split(",").map(Number),
      yaw: Number(main.dataset.cameraYaw),
      pitch: Number(main.dataset.cameraPitch),
      coverage: profile.coverageSamples,
      water: profile.materialAnimation.volumes,
      surfaces: profile.materialAnimation.surfaces,
      sky: profile.materialAnimation.skyController,
      staticProps: JSON.parse(document.querySelector<HTMLElement>(".world-canvas")!.dataset.staticProps!),
      bots: profile.bots,
      blockers: JSON.parse(main.dataset.blockers!),
    }
  })
  console.log(`[2fort-source] ${JSON.stringify({ spawn: source.spawn, coverage: source.coverage.length, water: source.water.map((value: any) => ({ bounds: value.bounds, surfaceZ: value.surfaceZ, minimumZ: value.minimumZ })), waterSurfaces: source.surfaces.length, staticProps: source.staticProps, bots: source.bots.map((bot: any) => ({ team: bot.team, class: bot.class, objective: bot.objective, area: bot.area })), blockers: source.blockers.length })}`)
  const mainWater = source.water.find((volume: any) => volume.surfaceZ === -180)
  if (!mainWater) throw new Error("ctf_2fort has no authored main-world water volume")
  const waterLeaves = new Set<number>(mainWater.leaves)
  const submerged = source.coverage
    .filter((sample: any) => waterLeaves.has(sample.leaf)
      && sample.position[2] < mainWater.surfaceZ - 12
      && sample.position[2] > mainWater.minimumZ)
    .toSorted((left: any, right: any) => Math.hypot(left.position[0], left.position[1])
      - Math.hypot(right.position[0], right.position[1]))[0]
  if (!submerged) throw new Error("ctf_2fort has no authored collision-valid underwater coverage position")

  const views: readonly Viewpoint[] = [
    { name: "blue-spawn", position: source.spawn as Viewpoint["position"], yawDegrees: source.yaw, pitchDegrees: source.pitch },
    { name: "red-spawn", position: [1888, 1368, 324], yawDegrees: 180, pitchDegrees: 0 },
    { name: "red-intelligence", position: [-490, 3175, -110], yawDegrees: 90, pitchDegrees: 4 },
    { name: "blue-intelligence", position: [489, -3175, -110], yawDegrees: -90, pitchDegrees: 4 },
    { name: "red-courtyard", position: [0, 1150, 90], yawDegrees: -90, pitchDegrees: 5 },
    { name: "bridge", position: [523, -439, 318], yawDegrees: 126, pitchDegrees: -16 },
    { name: "blue-courtyard", position: [0, -1150, 90], yawDegrees: 90, pitchDegrees: 5 },
    { name: "sewer", position: [0, 450, -200], yawDegrees: -90, pitchDegrees: 0 },
    { name: "water-above", position: [submerged.position[0], submerged.position[1], mainWater.surfaceZ + 40], yawDegrees: 90, pitchDegrees: 60 },
    { name: "water-below", position: submerged.position, yawDegrees: 90, pitchDegrees: 0 },
    { name: "outdoor-sky", position: [0, 350, 150], yawDegrees: -90, pitchDegrees: -20 },
  ]
  const local = await loadLocalConfig()
  const directory = path.join(local.sourceCacheDir, "evidence", "tf2-2fort-visual")
  await mkdir(directory, { recursive: true })
  const captures: any[] = []
  for (const [index, view] of views.entries()) {
    const revision = index + 1
    await page.evaluate(({ view, revision }) => {
      const profile = (globalThis as typeof globalThis & { __playsrcProfile: any }).__playsrcProfile
      profile.displacementCameraOverride = view
      profile.geometryEvidenceRevision = revision
    }, { view, revision })
    await page.waitForFunction(({ revision, position }) => {
      const profile = (globalThis as typeof globalThis & { __playsrcProfile?: any }).__playsrcProfile
      const canvas = document.querySelector<HTMLElement>(".world-canvas")
      return profile?.geometryEvidence?.revision === revision
        && profile.geometryEvidence.camera.position.join(",") === position.join(",")
        && canvas?.dataset.displayCameraPosition === position.join(",")
    }, { revision, position: view.position }, { timeout: 30_000 })
    const evidence = await page.evaluate(() => {
      const main = document.querySelector<HTMLElement>("main")!
      const canvas = document.querySelector<HTMLElement>(".world-canvas")!
      const profile = (globalThis as typeof globalThis & { __playsrcProfile: any }).__playsrcProfile
      return {
        geometry: profile.geometryEvidence,
        water: main.dataset.waterPlan,
        waterPasses: main.dataset.waterPasses,
        overlay: main.dataset.waterOverlay ?? null,
        sky: canvas.dataset.sky3dPass ? JSON.parse(canvas.dataset.sky3dPass) : null,
        props: canvas.dataset.visibleMainStaticProps ? JSON.parse(canvas.dataset.visibleMainStaticProps) : [],
        detail: main.dataset.performanceDetail ? JSON.parse(main.dataset.performanceDetail) : null,
        ctf: main.dataset.ctf,
      }
    })
    const screenshot = await page.locator("canvas.world-canvas").screenshot()
    await writeFile(path.join(directory, `${view.name}.png`), screenshot)
    const pixels = decodeScreenshot(screenshot)
    let nonBlack = 0, dark = 0, color = 0, magenta = 0
    for (let offset = 0; offset < pixels.pixels.length; offset += pixels.channels) {
      const red = pixels.pixels[offset]!, green = pixels.pixels[offset + 1]!, blue = pixels.pixels[offset + 2]!
      nonBlack += Number(red + green + blue > 12)
      dark += Number(red + green + blue < 15)
      color += Number(Math.max(red, green, blue) - Math.min(red, green, blue) > 12)
      magenta += Number(red > 180 && blue > 180 && green < 50)
    }
    const pixelDepth = evidence.geometry.geometry.samples
      .filter((sample: any) => sample.family !== null)
      .map((sample: any) => {
        const x = Math.max(0, Math.min(pixels.width - 1, Math.round((sample.x + 1) * pixels.width / 2)))
        const y = Math.max(0, Math.min(pixels.height - 1, Math.round((1 - sample.y) * pixels.height / 2)))
        const offset = (y * pixels.width + x) * pixels.channels
        const rgb = [pixels.pixels[offset]!, pixels.pixels[offset + 1]!, pixels.pixels[offset + 2]!]
        return { family: sample.family, depth: sample.depth, primitive: sample.primitive, object: sample.object, x, y, rgb }
      })
    const familyPixels = [...new Map(pixelDepth.filter((sample: any) => sample.rgb.some((channel: number) => channel > 3))
      .map((sample: any) => [sample.family, sample])).values()]
    const floorX = Math.floor(pixels.width / 2), floorY = Math.round(pixels.height * 0.79)
    const floorOffset = (floorY * pixels.width + floorX) * pixels.channels
    const floorPixel = [pixels.pixels[floorOffset]!, pixels.pixels[floorOffset + 1]!, pixels.pixels[floorOffset + 2]!]
    const hits = evidence.geometry.geometry.samples.filter((sample: any) => sample.disposition === "main-world")
    const staticHits = evidence.geometry.geometry.samples.filter((sample: any) => sample.disposition === "static-prop")
    const dynamicHits = evidence.geometry.geometry.samples.filter((sample: any) => sample.disposition === "dynamic-prop")
    const capture = {
      name: view.name,
      position: view.position,
      eyeLeaf: evidence.geometry.visibility.eyeLeaf,
      outside: evidence.geometry.visibility.outsideWorld,
      surfaces: evidence.geometry.visibility.drawSurfaces.length,
      areas: evidence.geometry.visibility.areas,
      worldHits: hits.length,
      staticPropHits: staticHits.length,
      dynamicPropHits: dynamicHits.length,
      visibleStaticProps: evidence.props.length,
      families: [...new Set(evidence.geometry.geometry.samples.map((sample: any) => sample.family).filter(Boolean))],
      familyPixels,
      floorPixel,
      materials: [...new Set(hits.map((sample: any) => sample.material))],
      depth: hits.length ? [Math.min(...hits.map((sample: any) => sample.depth)), Math.max(...hits.map((sample: any) => sample.depth))] : null,
      skyHits: evidence.geometry.skyGeometry?.samples.filter((sample: any) => sample.disposition === "main-world").length ?? 0,
      sky: evidence.sky && {
        skySurfaces: evidence.sky.skySurfaces,
        skyProps: evidence.sky.skyProps,
        mainProps: evidence.sky.mainProps,
        stateRestored: evidence.sky.stateRestored,
      },
      water: evidence.water,
      waterPasses: evidence.waterPasses,
      overlay: evidence.overlay,
      total: evidence.detail?.total,
      nonBlack,
      dark,
      color,
      magenta,
    }
    captures.push(capture)
    console.log(`[2fort-view] ${JSON.stringify(capture)}`)
    await testInfo.attach(view.name, { body: screenshot, contentType: "image/png" })
    expect(capture.outside, `${view.name} remains in an authored BSP leaf`).toBe(false)
    expect(capture.surfaces, `${view.name} retains visible authored faces`).toBeGreaterThan(0)
    expect(capture.nonBlack, `${view.name} contains actual headed pixels`).toBeGreaterThan(100_000)
    expect(capture.magenta, `${view.name} has no missing-texture magenta`).toBe(0)
    if (view.name.endsWith("intelligence")) {
      expect(capture.floorPixel.reduce((total, value) => total + value, 0), `${view.name} retains its depth-tested visible floor`).toBeGreaterThan(12)
    }
    if (view.name === "water-below" || view.name === "sewer") {
      expect(capture.water, `${view.name} selects authored underwater volume`).toMatch(/^below:/)
      expect(capture.overlay, `${view.name} presents its authored underwater overlay`).toBe("materials/effects/water_warp_2fort.vmt")
    }
  }

  expect(captures.some((capture) => capture.staticPropHits > 0)).toBe(true)
  expect(captures.some((capture) => capture.families.includes("displacement"))).toBe(true)
  expect(captures.some((capture) => capture.families.includes("water"))).toBe(true)
  expect(captures.some((capture) => capture.skyHits > 0 || capture.sky?.skySurfaces > 0)).toBe(true)
  expect(captures.some((capture) => capture.areas.length > 1), "authored open 2Fort area portals connect their visible areas").toBe(true)
  for (const family of ["world", "displacement", "static-prop", "dynamic-prop", "water"]) {
    expect(captures.some((capture) => capture.familyPixels.some((pixel: any) => pixel.family === family
      && Number.isFinite(pixel.depth) && pixel.depth > 0)), `${family} has authored visible color and actual ordered depth`).toBe(true)
  }

  await page.keyboard.press("Backquote")
  await command("setpos 523 -439 270")
  await page.evaluate(() => {
    delete (globalThis as typeof globalThis & { __playsrcProfile?: any }).__playsrcProfile?.displacementCameraOverride
  })
  await page.keyboard.press("Backquote")
  await page.waitForFunction(() => {
    const position = document.querySelector<HTMLElement>("main")?.dataset.cameraPosition?.split(",").map(Number)
    return Boolean(position && Math.abs(position[0]! - 523) < 1 && Math.abs(position[1]! + 439) < 1)
  }, undefined, { timeout: 30_000 })

  const seconds = profileSampleSeconds()
  const performance = await page.evaluate(async (duration) => {
    const root = document.querySelector<HTMLElement>("main")!
    const start = window.performance.now(), firstTick = Number(root.dataset.snapshotTick)
    const records: Array<{ total: number; models: number; visibility: number; world: number }> = [], frames: number[] = []
    const position = (): number[] => (root.dataset.cameraPosition ?? "").split(",").map(Number)
    let lastPosition = position(), traveled = 0, reversed = false
    const key = (kind: "keydown" | "keyup", code: "KeyW" | "KeyS") => {
      dispatchEvent(new KeyboardEvent(kind, { code, key: code === "KeyW" ? "w" : "s", bubbles: true }))
    }
    let previous = start, previousTick = ""
    key("keydown", "KeyW")
    try {
      await new Promise<void>((resolve, reject) => {
        const frame = (now: number): void => {
          if (root.dataset.phase !== "Ready") { reject(new Error(`2Fort outdoor noclip entered ${root.dataset.phase}: ${root.dataset.detail}`)); return }
          frames.push(now - previous)
          previous = now
          const current = position()
          traveled += Math.hypot(...current.map((value, index) => value - lastPosition[index]!))
          lastPosition = current
          const detail = root.dataset.performanceDetail
          if (detail) {
            const sample = JSON.parse(detail)
            if (sample.tick !== previousTick) {
              records.push({ total: sample.total, models: sample.models, visibility: sample.visibility, world: sample.world })
              previousTick = sample.tick
            }
          }
          if (!reversed && now - start >= duration * 500) {
            key("keyup", "KeyW")
            key("keydown", "KeyS")
            reversed = true
          }
          if (now - start >= duration * 1000) resolve()
          else requestAnimationFrame(frame)
        }
        requestAnimationFrame(frame)
      })
    } finally {
      key("keyup", "KeyW")
      key("keyup", "KeyS")
    }
    return { elapsed: window.performance.now() - start, ticks: Number(root.dataset.snapshotTick) - firstTick, traveled, records, frames }
  }, seconds)
  const report = {
    target: "ctf_2fort",
    captures,
    performance: {
      seconds: performance.elapsed / 1000,
      ticks: performance.ticks,
      traveled: Number(performance.traveled.toFixed(2)),
      work: summarizeFrameTimes(performance.records.map((record) => record.total)),
      models: summarizeFrameTimes(performance.records.map((record) => record.models)),
      visibility: summarizeFrameTimes(performance.records.map((record) => record.visibility)),
      world: summarizeFrameTimes(performance.records.map((record) => record.world)),
      cadence: summarizeFrameTimes(performance.frames),
    },
    bots: await root.getAttribute("data-bot-count"),
    ctf: await root.getAttribute("data-ctf"),
    hud: await root.getAttribute("data-hud-probe"),
  }
  await writeFile(path.join(directory, "ctf_2fort-visual.json"), `${JSON.stringify(report, null, 2)}\n`)
  console.log(`[2fort-visual] ${JSON.stringify(report.performance)}`)
  expect(performance.ticks).toBeGreaterThan(seconds * 55)
  expect(report.performance.traveled).toBeGreaterThan(400)
  expect(report.performance.work.frames).toBeGreaterThan(30)
  expect(report.performance.work.p95Milliseconds, "outdoor Source rendering retains its full-quality 120 Hz work budget").toBeLessThan(1000 / 120)
  await expect(root).toHaveAttribute("data-bot-count", "1")
  await expect(page.locator("[data-vgui-name='HudObjectiveStatus']")).toBeVisible()
  expect(await root.getAttribute("data-phase")).toBe("Ready")
})
