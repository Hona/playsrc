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

const TARGETS = ["jump_beef", "ctf_2fort", "pl_upward"] as const

type GeometrySample = Readonly<{
  x: number
  y: number
  disposition: string
  depth: number | null
  primitive: number | null
  object: number | null
  material: string | null
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

function visiblePixels(png: Buffer, hits: readonly GeometrySample[]) {
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
    if (index === TARGETS.length - 1) {
      await page.reload({ waitUntil: "load", timeout: 30_000 })
      await expect(root).toHaveAttribute("data-phase", "MainMenu", { timeout: 180_000 })
    }
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
      await writeFile(path.join(output, `${target}-${checkpoint.kind}.png`), screenshot)
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
      })
    }

    const measured = await page.evaluate(async (minimumMilliseconds) => {
      const main = document.querySelector<HTMLElement>("main")!
      const started = performance.now()
      const firstTick = Number(main.dataset.snapshotTick)
      const frames: number[] = []
      let previous = started
      await new Promise<void>((resolve) => {
        const frame = (now: number): void => {
          if (now > previous) frames.push(now - previous)
          previous = now
          if (now - started >= minimumMilliseconds) resolve()
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
    allFrames.push(...measured.frames)
    maps.push({
      target,
      loadMilliseconds,
      activeMilliseconds: Number(measured.elapsedMilliseconds.toFixed(3)),
      simulation: { firstTick: measured.firstTick, lastTick: measured.lastTick, hz: Number(simulationHz.toFixed(2)) },
      frames: summarizeFrameTimes(measured.frames),
      checkpoints: observations,
    })
  }

  const coverage = new Set(maps.flatMap((map) => (map.checkpoints as { kind: string }[]).map((checkpoint) => checkpoint.kind)))
  for (const kind of ["spawn", "outdoor-terrain", "floor", "water", "bridge", "objective"]) {
    expect(coverage.has(kind), `authored ${kind} camera checkpoint`).toBe(true)
  }
  const report = {
    schema: "playsrc-tf2-headed-map-sanity-v1",
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
