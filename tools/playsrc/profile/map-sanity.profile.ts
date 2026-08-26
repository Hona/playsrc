import { createHash } from "node:crypto"
import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import type { Locator, Page } from "@playwright/test"
import { loadLocalConfig } from "../src/config"
import { expect, test } from "./application-test"
import { sanityViewAngles, selectSanityCheckpoints, type SanityLandmarks, type SanityPosition } from "./map-sanity"
import { divideProfileWindow, profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"
import { chooseTf2Team } from "./team-selection-evidence"

const ALL_TARGETS = ["jump_beef", "ctf_2fort", "pl_upward"] as const
const selectedTarget = process.env.PROFILE_MAP_SANITY_TARGET
if (selectedTarget && !ALL_TARGETS.includes(selectedTarget as typeof ALL_TARGETS[number])) {
  throw new Error(`PROFILE_MAP_SANITY_TARGET is not a configured map: ${selectedTarget}`)
}
const TARGETS = selectedTarget
  ? ALL_TARGETS.filter((target) => target === selectedTarget)
  : ALL_TARGETS

type GeometrySample = Readonly<{
  x: number
  y: number
  disposition: string
  depth: number | null
  primitive: number | null
  object: number | null
  material: string | null
  lightmap: readonly [number, number, number] | null
}>

type LightingObservation = Readonly<{
  revision: number
  profile: "ldr" | "hdr"
  exposure: Readonly<{ current: number; goal: number; submittedHistograms: number }>
  viewmodel: null | Readonly<{
    origin: readonly [number, number, number]
    ambientCube: readonly (readonly [number, number, number])[]
    localLights: readonly Readonly<{ kind: string; color: readonly [number, number, number] }>[]
    environment: string | null
  }>
  models: readonly Readonly<{ identity: number; origin: readonly [number, number, number]; localLights: number; eyes: number }>[]
  geometry: Readonly<{
    samples: readonly Readonly<{
      x: number
      y: number
      identity: number
      material: string
      modelDepth: number
      worldDepth: number | null
    }>[]
  }>
  worldGeometry: LightingObservation["geometry"]
  depthIsolated: boolean
}>

type CheckpointGeometry = Readonly<{
  generation: number
  target: string
  near: number
  eyeLeaf: number | null
  outsideWorld: boolean
  drawSurfaces: number[]
  samples: GeometrySample[]
  position: SanityPosition
  waterPlan: string
  waterPasses: string
  tick: number
}>

async function consoleCommand(page: Page, entry: Locator, command: string): Promise<void> {
  if (await page.locator("main").getAttribute("data-console-visible") !== "true") {
    await page.keyboard.press("Backquote")
  }
  await expect(entry).toBeVisible()
  await entry.fill(command)
  await entry.press("Enter")
}

function visiblePixels(png: Buffer, hits: readonly Readonly<{ x: number; y: number }>[]) {
  const image = decodeScreenshot(png)
  let nonBackgroundPixels = 0
  let totalLuma = 0
  let measured = 0
  for (const hit of hits) {
    const centerX = Math.max(4, Math.min(image.width - 5, Math.round((hit.x + 1) * image.width / 2)))
    const centerY = Math.max(4, Math.min(image.height - 5, Math.round((1 - hit.y) * image.height / 2)))
    for (let y = centerY - 4; y < centerY + 4; y += 1) {
      for (let x = centerX - 4; x < centerX + 4; x += 1) {
        const offset = (y * image.width + x) * image.channels
        const red = image.pixels[offset]!
        const green = image.pixels[offset + 1]!
        const blue = image.pixels[offset + 2]!
        const luma = 0.2126 * red + 0.7152 * green + 0.0722 * blue
        if (luma > 4) nonBackgroundPixels += 1
        totalLuma += luma
        measured += 1
      }
    }
  }
  return {
    width: image.width,
    height: image.height,
    measuredPixels: measured,
    nonBackgroundPixels,
    meanLuma: Number((totalLuma / measured).toFixed(3)),
    sha256: createHash("sha256").update(png).digest("hex"),
  }
}

test("headed bounded three-map authored noclip visual and frame sanity", async ({ page }, testInfo) => {
  const seconds = profileSampleSeconds()
  const windows = divideProfileWindow(seconds, TARGETS.length)
  const local = await loadLocalConfig()
  const output = path.join(local.sourceCacheDir, "profiles", "map-sanity")
  await mkdir(output, { recursive: true })
  const gpuValidationErrors: string[] = []
  page.on("console", (message) => {
    if (message.text().startsWith("[map-replacement]")) console.log(message.text())
    if (/GPUValidationError|Destroyed texture/.test(message.text())) gpuValidationErrors.push(message.text())
  })

  await page.addInitScript(() => {
    let locked: Element | null = null
    Object.defineProperty(document, "pointerLockElement", { configurable: true, get: () => locked })
    Object.defineProperty(Element.prototype, "requestPointerLock", {
      configurable: true,
      value(this: Element) {
        locked = this
        queueMicrotask(() => document.dispatchEvent(new Event("pointerlockchange")))
        return Promise.resolve()
      },
    })
    Object.defineProperty(document, "exitPointerLock", {
      configurable: true,
      value() {
        locked = null
        queueMicrotask(() => document.dispatchEvent(new Event("pointerlockchange")))
        return Promise.resolve()
      },
    })
    ;(globalThis as typeof globalThis & { __playsrcProfile?: Record<string, unknown> }).__playsrcProfile = {}
  })

  await page.goto("/", { waitUntil: "load", timeout: 30_000 })
  await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu", { timeout: 180_000 })
  const catalog = await (await page.request.get("/playsrc-config.json")).json() as { targets: readonly { target: string }[] }
  expect(catalog.targets.map((target) => target.target)).toEqual(["jump_beef", "pl_upward", "ctf_2fort"])

  const root = page.locator("main")
  const canvas = page.locator("canvas.world-canvas")
  const entry = page.locator("[aria-label='Console command']")
  const maps: Record<string, unknown>[] = []
  const allFrames: number[] = []
  let revision = 0

  for (const [index, target] of TARGETS.entries()) {
    const loadStarted = performance.now()
    await consoleCommand(page, entry, `map ${target}`)
    await page.waitForFunction((identity) => {
      const main = document.querySelector<HTMLElement>("main")
      const console = document.querySelector<HTMLElement>("[aria-label='Console output']")?.innerText ?? ""
      return main?.dataset.phase === "Ready" && main.dataset.detail === `Playing ${identity}`
        || main?.dataset.phase === "Failed" || console.includes("ERROR: Map replacement failed")
        || main?.dataset.teamSelectionVisible === "true" || main?.dataset.classSelectionVisible === "true"
    }, target, { timeout: 600_000, polling: 25 })
    const consoleOutput = await page.locator("[aria-label='Console output']").innerText()
    if (consoleOutput.includes("ERROR: Map replacement failed")) {
      throw new Error(`${target} authored map replacement failed: ${consoleOutput.split("\n").at(-1)}`)
    }
    if (await root.getAttribute("data-team-selection-visible") === "true") {
      if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
      await chooseTf2Team(page, "red")
    }
    if (await root.getAttribute("data-class-selection-visible") === "true") {
      if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
      await page.keyboard.press("Digit2")
    }
    await expect(root).toHaveAttribute("data-phase", "Ready", { timeout: 600_000 })
    const staticProps = JSON.parse((await canvas.getAttribute("data-static-props")) ?? "null") as null | {
      total: number
      main: number
      sky3d: number
      runtimeLit: number
    }
    const expectedProps = { jump_beef: 0, ctf_2fort: 2265, pl_upward: 1244 }[target]
    if (!staticProps || staticProps.total !== expectedProps || staticProps.main + staticProps.sky3d !== expectedProps) {
      throw new Error(`${target} lost authored static-prop lighting or ownership: ${JSON.stringify(staticProps)}`)
    }
    await consoleCommand(page, entry, "noclip")
    await expect(root).toHaveAttribute("data-movement-mode", "1")

    await page.waitForFunction(() => {
      const profile = (globalThis as any).__playsrcProfile
      return profile?.coverageSamples?.length > 0 && Array.isArray(profile.objectives) && Array.isArray(profile.pickups)
    }, undefined, { timeout: 30_000 })
    const landmarks = await page.evaluate((identity) => {
      const main = document.querySelector<HTMLElement>("main")!
      const profile = (globalThis as any).__playsrcProfile
      const position = (main.dataset.cameraPosition ?? "").split(",").map(Number)
      position[2] = position[2]! - Number((main.dataset.viewOffset ?? "0,0,68").split(",")[2])
      if (position.length !== 3 || !position.every(Number.isFinite)) throw new Error("authored spawn position is unavailable")
      return {
        target: identity,
        spawn: position,
        samples: profile.coverageSamples,
        water: profile.materialAnimation?.volumes ?? [],
        skyArea: profile.materialAnimation?.skyController?.area ?? null,
        objectives: profile.objectives.map((objective: any) => objective.position),
        pickups: profile.pickups.map((pickup: any) => pickup.origin),
      }
    }, target) as SanityLandmarks
    const checkpoints = selectSanityCheckpoints(landmarks)
    console.log(`[map-sanity] ${target} authored cameras=${checkpoints.map((checkpoint) => checkpoint.kind).join(",")}`)
    const loadMilliseconds = Number((performance.now() - loadStarted).toFixed(3))

    const observations: Record<string, unknown>[] = []
    for (const checkpoint of checkpoints) {
      if (checkpoint.kind !== "spawn") {
        const viewOffset = Number(((await root.getAttribute("data-view-offset")) ?? "0,0,68").split(",")[2])
        const playerOrigin = [checkpoint.position[0], checkpoint.position[1], checkpoint.position[2] - viewOffset]
        await consoleCommand(page, entry, `setpos ${playerOrigin.join(" ")}`)
        await page.waitForFunction((position) => {
          const main = document.querySelector<HTMLElement>("main")
          const current = (main?.dataset.cameraPosition ?? "").split(",").map(Number)
          return main?.dataset.phase === "Failed" || current.length === 3
            && Math.hypot(current[0]! - position[0], current[1]! - position[1]) < 2
        }, checkpoint.position, { timeout: 15_000, polling: 10 })
      }
      if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
      await canvas.click()
      await expect(root).toHaveAttribute("data-pointer-locked", "true")

      const direction = sanityViewAngles(checkpoint.position, checkpoint.focus)
      let geometry: CheckpointGeometry | undefined
      let hits: GeometrySample[] = []
      for (const angles of [direction, { pitch: 45, yaw: direction.yaw + 90 }, { pitch: 65, yaw: direction.yaw + 180 }]) {
        revision += 1
        await page.evaluate(({ angles, revision }) => {
          const main = document.querySelector<HTMLElement>("main")!
          const canvas = document.querySelector<HTMLElement>("canvas.world-canvas")!
          const yaw = Number(canvas.dataset.displayCameraYaw ?? main.dataset.cameraYaw)
          const pitch = Number(canvas.dataset.displayCameraPitch ?? main.dataset.cameraPitch)
          const wrap = (value: number) => ((value + 180) % 360 + 360) % 360 - 180
          const mouse = new MouseEvent("mousemove", { bubbles: true })
          Object.defineProperties(mouse, {
            movementX: { value: wrap(yaw - angles.yaw) / 0.066 },
            movementY: { value: (angles.pitch - pitch) / 0.066 },
          })
          dispatchEvent(mouse)
          ;(globalThis as any).__playsrcProfile.geometryEvidenceRevision = revision
        }, { angles, revision })
        await page.waitForFunction(({ expected, identity }) => {
          const main = document.querySelector<HTMLElement>("main")
          const evidence = (globalThis as any).__playsrcProfile?.geometryEvidence
          return main?.dataset.phase === "Failed" || evidence?.revision === expected && evidence.target === identity
        }, { expected: revision, identity: target }, { timeout: 20_000, polling: 10 })
        await expect(root).toHaveAttribute("data-phase", "Ready")

        geometry = await page.evaluate(() => {
          const main = document.querySelector<HTMLElement>("main")!
          const evidence = (globalThis as any).__playsrcProfile.geometryEvidence
          return {
            generation: evidence.generation,
            target: evidence.target,
            near: evidence.camera.near,
            eyeLeaf: evidence.visibility.eyeLeaf,
            outsideWorld: evidence.visibility.outsideWorld,
            drawSurfaces: evidence.visibility.drawSurfaces,
            samples: evidence.geometry.samples,
            position: evidence.camera.position,
            waterPlan: main.dataset.waterPlan ?? "",
            waterPasses: main.dataset.waterPasses ?? "",
            tick: Number(main.dataset.snapshotTick),
          }
        }) as CheckpointGeometry
        const admitted = new Set(geometry.drawSurfaces)
        hits = geometry.samples.filter((sample) => sample.disposition === "main-world"
          && sample.depth !== null && sample.depth > geometry.near
          && sample.primitive !== null && admitted.has(sample.primitive)
          && Number.isSafeInteger(sample.object) && Boolean(sample.material))
        if (!geometry.outsideWorld && geometry.eyeLeaf !== null && hits.length > 0) break
      }
      if (!geometry || geometry.outsideWorld || geometry.eyeLeaf === null || hits.length === 0) {
        throw new Error(`${target} ${checkpoint.kind} has no visible authored world depth: ${JSON.stringify({
          position: geometry?.position,
          eyeLeaf: geometry?.eyeLeaf,
          outsideWorld: geometry?.outsideWorld,
          drawSurfaces: geometry?.drawSurfaces.length,
        })}`)
      }

      const screenshot = await canvas.screenshot()
      const pixels = visiblePixels(screenshot, hits)
      if (pixels.nonBackgroundPixels < pixels.measuredPixels / 4 || pixels.meanLuma <= 4) {
        throw new Error(`${target} ${checkpoint.kind} has no visible authored world pixels: ${JSON.stringify(pixels)}`)
      }
      revision += 1
      await page.evaluate((value) => {
        ;(globalThis as any).__playsrcProfile.worldLightingEvidenceRevision = value
      }, revision)
      await page.waitForFunction((value) =>
        (globalThis as any).__playsrcProfile?.worldLighting?.revision === value,
      revision, { timeout: 20_000, polling: 10 })
      const lighting = await page.evaluate(() => (globalThis as any).__playsrcProfile.worldLighting) as LightingObservation
      if (lighting.profile !== "hdr" || !lighting.viewmodel || !lighting.depthIsolated) {
        throw new Error(`${target} ${checkpoint.kind} lacks exact HDR viewmodel lighting or world-depth isolation`)
      }
      const ambient = lighting.viewmodel.ambientCube.reduce((sum, side) =>
        sum + side[0] * 0.2125 + side[1] * 0.7154 + side[2] * 0.0721, 0) / 6
      if (!Number.isFinite(ambient) || ambient <= 0) {
        throw new Error(`${target} ${checkpoint.kind} has no authored BSP leaf ambient cube`)
      }
      const launcher = lighting.geometry.samples.filter((sample) => sample.material.toLowerCase().includes("rocket"))
      if (launcher.length === 0) {
        throw new Error(`${target} ${checkpoint.kind} has no visible authored Soldier rocket-launcher depth`)
      }
      const weaponPixels = visiblePixels(screenshot, launcher)
      await writeFile(path.join(output, `${target}-${checkpoint.kind}.png`), screenshot)
      console.log(`[map-lighting] ${target}:${checkpoint.kind} ambient=${ambient.toFixed(4)} lights=${lighting.viewmodel.localLights.length} exposure=${lighting.exposure.current.toFixed(4)} launcher=${weaponPixels.meanLuma.toFixed(2)}`)
      const authoredMinimum = Math.min(8, ambient * 255)
      if (weaponPixels.meanLuma <= authoredMinimum ||
        (ambient >= 0.03 && weaponPixels.nonBackgroundPixels < weaponPixels.measuredPixels / 4)) {
        throw new Error(`${target} ${checkpoint.kind} Soldier launcher remains nearly black: ${JSON.stringify(weaponPixels)}`)
      }
      const authoredIrradiance = hits.filter((hit) => hit.lightmap !== null).map((hit) => {
        const [red, green, blue] = hit.lightmap!
        return red * 0.2125 + green * 0.7154 + blue * 0.0721
      })
      if (authoredIrradiance.length === 0 || authoredIrradiance.every((value) => value <= 0)) {
        throw new Error(`${target} ${checkpoint.kind} has no selected authored world lightmap exposure`)
      }
      observations.push({
        kind: checkpoint.kind,
        position: geometry.position,
        eyeLeaf: geometry.eyeLeaf,
        authoredLeaf: checkpoint.leaf,
        worldSamples: hits.length,
        depthRange: [Math.min(...hits.map((sample) => sample.depth!)), Math.max(...hits.map((sample) => sample.depth!))],
        waterPlan: geometry.waterPlan,
        waterPasses: geometry.waterPasses,
        tick: geometry.tick,
        pixels,
        lighting: {
          profile: lighting.profile,
          exposure: lighting.exposure,
          ambientLuminance: Number(ambient.toFixed(6)),
          localLights: lighting.viewmodel.localLights.length,
          environment: lighting.viewmodel.environment,
          lightmapLuminance: {
            minimum: Number(Math.min(...authoredIrradiance).toFixed(6)),
            maximum: Number(Math.max(...authoredIrradiance).toFixed(6)),
          },
          launcher: {
            samples: launcher.length,
            depthRange: [Math.min(...launcher.map((sample) => sample.modelDepth)), Math.max(...launcher.map((sample) => sample.modelDepth))],
            worldDepth: launcher.map((sample) => sample.worldDepth).filter((depth) => depth !== null),
            pixels: weaponPixels,
          },
          depthIsolated: lighting.depthIsolated,
        },
      })
    }

    console.log(`[map-lighting] ${target}:sampling seconds=${windows[index]}`)
    const measured = await page.evaluate(async (minimumMilliseconds) => {
      const main = document.querySelector<HTMLElement>("main")!
      const started = performance.now()
      const firstTick = Number(main.dataset.snapshotTick)
      const frames: number[] = []
      let previous = started
      await new Promise<void>((resolve, reject) => {
        const watchdog = setTimeout(() => reject(new Error("headed animation-frame sampling stalled")), minimumMilliseconds + 10_000)
        const frame = (now: number): void => {
          if (now > previous) frames.push(now - previous)
          previous = now
          if (now - started >= minimumMilliseconds) {
            clearTimeout(watchdog)
            resolve()
          }
          else requestAnimationFrame(frame)
        }
        requestAnimationFrame(frame)
      })
      const elapsedMilliseconds = performance.now() - started
      const lastTick = Number(main.dataset.snapshotTick)
      return { elapsedMilliseconds, frames, firstTick, lastTick }
    }, windows[index]! * 1_000)
    const simulationHz = (measured.lastTick - measured.firstTick) * 1_000 / measured.elapsedMilliseconds
    if (simulationHz < 55) {
      throw new Error(`${target} fixed simulation cadence regressed: ${simulationHz.toFixed(2)} Hz`)
    }
    const frameDistribution = summarizeFrameTimes(measured.frames)
    console.log(`[map-lighting] ${target}:frames p95=${frameDistribution.p95Milliseconds} hz=${simulationHz.toFixed(2)}`)
    if (frameDistribution.p95Milliseconds > 20) {
      throw new Error(`${target} authored lighting regressed headed frame p95: ${frameDistribution.p95Milliseconds} ms`)
    }
    if (gpuValidationErrors.length > 0) {
      throw new Error(`${target} submitted destroyed or invalid authored GPU textures: ${gpuValidationErrors[0]}`)
    }
    revision += 1
    await page.evaluate((value) => {
      ;(globalThis as any).__playsrcProfile.worldLightingEvidenceRevision = value
    }, revision)
    await page.waitForFunction((value) =>
      (globalThis as any).__playsrcProfile?.worldLighting?.revision === value,
    revision, { timeout: 20_000, polling: 10 })
    const adapted = await page.evaluate(() =>
      (globalThis as any).__playsrcProfile.worldLighting.exposure) as LightingObservation["exposure"]
    if (adapted.submittedHistograms < 1 || adapted.current < 0.5 || adapted.current > 2) {
      throw new Error(`${target} has no real headed-frame HDR histogram or bounded Source exposure: ${JSON.stringify(adapted)}`)
    }
    console.log(`[map-lighting] ${target}:histograms submitted=${adapted.submittedHistograms} exposure=${adapted.current.toFixed(4)}`)
    allFrames.push(...measured.frames)
    let playerModel: Record<string, unknown> | undefined
    if (target === "ctf_2fort") {
      await consoleCommand(page, entry, "tf_bot_add red soldier normal")
      await expect(root).toHaveAttribute("data-bot-count", "1", { timeout: 30_000 })
      const bot = await page.evaluate(() => {
        const value = (globalThis as any).__playsrcProfile.bots.find((candidate: any) => candidate.class === 3)
        if (!value) throw new Error("authored Soldier bot is unavailable")
        return { identity: value.identity as number, position: value.position as SanityPosition }
      })
      const cameraSample = landmarks.samples
        .filter((sample) => {
          const range = Math.hypot(sample.position[0] - bot.position[0], sample.position[1] - bot.position[1], sample.position[2] - bot.position[2])
          return range >= 48 && range <= 300
        })
        .toSorted((left, right) => Math.hypot(left.position[0] - bot.position[0], left.position[1] - bot.position[1], left.position[2] - bot.position[2])
          - Math.hypot(right.position[0] - bot.position[0], right.position[1] - bot.position[1], right.position[2] - bot.position[2]))[0]
      if (!cameraSample) throw new Error("ctf_2fort has no authored nearby Soldier-model camera sample")
      const offset = Number(((await root.getAttribute("data-view-offset")) ?? "0,0,68").split(",")[2])
      await consoleCommand(page, entry, `setpos ${cameraSample.position[0]} ${cameraSample.position[1]} ${cameraSample.position[2] - offset}`)
      await page.waitForFunction((position) => {
        const camera = (document.querySelector<HTMLElement>("main")?.dataset.cameraPosition ?? "").split(",").map(Number)
        return camera.length === 3 && Math.hypot(camera[0]! - position[0], camera[1]! - position[1]) < 2
      }, cameraSample.position, { timeout: 15_000, polling: 10 })
      if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
      await canvas.click()
      const refreshed = await page.evaluate((identity) => {
        const bot = (globalThis as any).__playsrcProfile.bots.find((candidate: any) => candidate.identity === identity)
        if (!bot) throw new Error("authored Soldier bot disappeared")
        return bot.position as SanityPosition
      }, bot.identity)
      const aim = sanityViewAngles(cameraSample.position, [refreshed[0], refreshed[1], refreshed[2] + 52])
      revision += 1
      await page.evaluate(({ angles, revision }) => {
        const main = document.querySelector<HTMLElement>("main")!
        const canvas = document.querySelector<HTMLElement>("canvas.world-canvas")!
        const yaw = Number(canvas.dataset.displayCameraYaw ?? main.dataset.cameraYaw)
        const pitch = Number(canvas.dataset.displayCameraPitch ?? main.dataset.cameraPitch)
        const wrap = (value: number) => ((value + 180) % 360 + 360) % 360 - 180
        const mouse = new MouseEvent("mousemove", { bubbles: true })
        Object.defineProperties(mouse, {
          movementX: { value: wrap(yaw - angles.yaw) / 0.066 },
          movementY: { value: (angles.pitch - pitch) / 0.066 },
        })
        dispatchEvent(mouse)
        ;(globalThis as any).__playsrcProfile.worldLightingEvidenceRevision = revision
      }, { angles: aim, revision })
      await page.waitForFunction((value) =>
        (globalThis as any).__playsrcProfile?.worldLighting?.revision === value,
      revision, { timeout: 20_000, polling: 10 })
      const evidence = await page.evaluate(() => (globalThis as any).__playsrcProfile.worldLighting) as LightingObservation
      const identity = 0x6000_0000 + bot.identity
      const model = evidence.models.find((candidate) => candidate.identity === identity)
      const samples = evidence.worldGeometry.samples.filter((sample) => sample.identity === identity)
      if (!model || model.eyes < 1 || samples.length === 0) {
        throw new Error(`ctf_2fort Soldier model lacks authored eye lighting or visible depth: ${JSON.stringify({ model, samples })}`)
      }
      const screenshot = await canvas.screenshot()
      const pixels = visiblePixels(screenshot, samples)
      if (pixels.meanLuma <= 4) throw new Error(`ctf_2fort Soldier player model remains black: ${JSON.stringify(pixels)}`)
      await writeFile(path.join(output, "ctf_2fort-soldier-player.png"), screenshot)
      playerModel = { identity, eyes: model.eyes, localLights: model.localLights, samples: samples.length,
        depthRange: [Math.min(...samples.map((sample) => sample.modelDepth)), Math.max(...samples.map((sample) => sample.modelDepth))], pixels }
      console.log(`[map-lighting] ctf_2fort:soldier eyes=${model.eyes} lights=${model.localLights} luma=${pixels.meanLuma.toFixed(2)}`)
    }
    let ldr: Record<string, unknown> | undefined
    if ((!selectedTarget || process.env.PROFILE_MAP_SANITY_LDR === "1") && target === "pl_upward") {
      const generation = Number(await root.getAttribute("data-generation"))
      console.log(`[map-lighting] pl_upward:ldr-replacing generation=${generation}`)
      await consoleCommand(page, entry, "mat_hdr_level 0")
      console.log("[map-lighting] pl_upward:ldr-command-submitted")
      await page.waitForFunction((previous) => {
        const main = document.querySelector<HTMLElement>("main")
        return main?.dataset.phase === "Failed"
          || main?.dataset.phase === "Ready" && Number(main.dataset.generation) > previous
      }, generation, { timeout: 120_000, polling: 20 })
      await expect(root).toHaveAttribute("data-phase", "Ready")
      await expect(root).toHaveAttribute("data-viewmodel-world-depth-isolated", "true", { timeout: 30_000 })
      if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
      await canvas.click()
      revision += 1
      await page.evaluate((value) => {
        ;(globalThis as any).__playsrcProfile.worldLightingEvidenceRevision = value
      }, revision)
      await page.waitForFunction((value) =>
        (globalThis as any).__playsrcProfile?.worldLighting?.revision === value,
      revision, { timeout: 20_000, polling: 10 })
      const lighting = await page.evaluate(() => (globalThis as any).__playsrcProfile.worldLighting) as LightingObservation
      const retained = JSON.parse((await canvas.getAttribute("data-static-props")) ?? "null") as typeof staticProps
      const launcher = lighting.geometry.samples.filter((sample) => sample.material.toLowerCase().includes("rocket"))
      if (lighting.profile !== "ldr" || lighting.exposure.current !== 1 || !lighting.viewmodel ||
        !lighting.depthIsolated || retained?.total !== expectedProps || launcher.length === 0) {
        throw new Error(`pl_upward LDR lighting replacement lost authored props, exposure, or launcher depth: ${JSON.stringify({ lighting, retained })}`)
      }
      const screenshot = await canvas.screenshot()
      const pixels = visiblePixels(screenshot, launcher)
      if (pixels.meanLuma <= 4) throw new Error(`pl_upward LDR Soldier launcher remains black: ${JSON.stringify(pixels)}`)
      if (gpuValidationErrors.length > 0) {
        throw new Error(`pl_upward LDR submitted destroyed or invalid authored GPU textures: ${gpuValidationErrors[0]}`)
      }
      await writeFile(path.join(output, "pl_upward-ldr-soldier.png"), screenshot)
      ldr = { exposure: lighting.exposure, staticProps: retained, localLights: lighting.viewmodel.localLights.length,
        depthIsolated: lighting.depthIsolated, launcherPixels: pixels }
      console.log(`[map-lighting] pl_upward:ldr props=${retained.total} lights=${lighting.viewmodel.localLights.length} launcher=${pixels.meanLuma.toFixed(2)}`)
    }
    maps.push({
      target,
      loadMilliseconds,
      activeMilliseconds: Number(measured.elapsedMilliseconds.toFixed(3)),
      staticProps,
      exposure: adapted,
      simulation: { firstTick: measured.firstTick, lastTick: measured.lastTick, hz: Number(simulationHz.toFixed(2)) },
      frames: frameDistribution,
      checkpoints: observations,
      ...(playerModel ? { playerModel } : {}),
      ...(ldr ? { ldr } : {}),
    })
  }

  const coverage = new Set(maps.flatMap((map) => (map.checkpoints as { kind: string }[]).map((checkpoint) => checkpoint.kind)))
  if (!selectedTarget) for (const kind of ["spawn", "outdoor-terrain", "floor", "water", "bridge", "objective"]) {
    expect(coverage.has(kind), `authored ${kind} camera checkpoint`).toBe(true)
  }
  const report = {
    schema: "playsrc-tf2-headed-map-sanity-v2",
    headed: true,
    startupMovie: "skipped",
    viewport: { width: 1280, height: 720 },
    requestedActiveSeconds: seconds,
    activeMilliseconds: Number(maps.reduce((total, map) => total + Number(map.activeMilliseconds), 0).toFixed(3)),
    targets: TARGETS,
    frames: summarizeFrameTimes(allFrames),
    maps,
  }
  const serialized = `${JSON.stringify(report, null, 2)}\n`
  await writeFile(path.join(output, "report.json"), serialized)
  await testInfo.attach("headed-three-map-sanity", { body: Buffer.from(serialized), contentType: "application/json" })
  console.log(`PLAYSRC_MAP_SANITY ${JSON.stringify(report)}`)
})
