import { createHash } from "node:crypto"
import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
import { expect, test } from "./application-test"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"
import { chooseTf2Team } from "./team-selection-evidence"

type CameraEvidence = { position: number[]; yawDegrees: number; pitchDegrees: number; verticalFovDegrees: number }
type CompletedFrame = {
  frame: number; completedAt: number; generation: number; viewportRevision: number; preparedRevision: number
  viewRevision: number; mouseRevision: number; snapRevision: number; tick: string
  main: CameraEvidence; sky: CameraEvidence | null; controller: { origin: number[]; scale: number; area: number } | null
  selection: number; disposition: string; mainVisibilityIdentity: string; skyVisibilityIdentity: string | null
  mainSurfaces: number; skySurfaces: number; skyProps: number
  visibilityMilliseconds: number; renderMilliseconds: number; totalMilliseconds: number
}

test("headed grounded Upward movement presents coherent visible 3D-sky and main-world cameras", async ({ page }, testInfo) => {
  const seconds = profileSampleSeconds()
  await page.addInitScript(() => {
    let locked: Element | null = null
    Object.defineProperty(document, "pointerLockElement", { configurable: true, get: () => locked })
    Object.defineProperty(Element.prototype, "requestPointerLock", { configurable: true, value(this: Element) {
      locked = this; queueMicrotask(() => document.dispatchEvent(new Event("pointerlockchange"))); return Promise.resolve()
    } })
    Object.defineProperty(document, "exitPointerLock", { configurable: true, value() {
      locked = null; queueMicrotask(() => document.dispatchEvent(new Event("pointerlockchange"))); return Promise.resolve()
    } })
    ;(globalThis as any).__playsrcProfile = { skyCoherenceFrames: [] }
  })
  await page.goto("/", { waitUntil: "load", timeout: 30_000 })
  const root = page.locator("main")
  await expect(root).toHaveAttribute("data-phase", "MainMenu", { timeout: 60_000 })
  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  await entry.fill("map pl_upward")
  await entry.press("Enter")
  await page.waitForFunction(() => {
    const main = document.querySelector<HTMLElement>("main")
    return main?.dataset.teamSelectionVisible === "true" || main?.dataset.phase === "Ready" || main?.dataset.phase === "Failed"
  }, undefined, { timeout: 65_000 })
  if (await root.getAttribute("data-team-selection-visible") === "true") {
    if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await chooseTf2Team(page, "blue")
  }
  await expect(root).toHaveAttribute("data-phase", "Ready", { timeout: 15_000 })
  if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  expect(await root.getAttribute("data-movement-mode")).not.toBe("1")
  const canvas = page.locator("canvas.world-canvas")
  await canvas.click()
  await page.waitForFunction(() => document.pointerLockElement?.classList.contains("world-canvas"), undefined, { timeout: 5_000 })
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.skyCoherenceFrames.some((frame: CompletedFrame) => frame.sky !== null), undefined, { timeout: 15_000 })
  const before = await canvas.screenshot()
  await page.keyboard.down("w")
  const measurement = await page.evaluate(async (duration) => {
    const profile = (globalThis as any).__playsrcProfile as { skyCoherenceFrames: CompletedFrame[] }
    const main = document.querySelector<HTMLElement>("main")!
    const initial = profile.skyCoherenceFrames.length
    const started = performance.now()
    const tick = Number(main.dataset.snapshotTick)
    let motions = 0
    while (performance.now() - started < duration * 1_000) {
      await new Promise<void>((resolve) => setTimeout(resolve, 54))
      const motion = new MouseEvent("mousemove", { bubbles: true })
      Object.defineProperties(motion, {
        movementX: { value: motions % 2 === 0 ? 14 : -11 },
        movementY: { value: motions % 4 < 2 ? 3 : -3 },
      })
      window.dispatchEvent(motion)
      motions += 1
    }
    return {
      seconds: (performance.now() - started) / 1_000,
      ticks: Number(main.dataset.snapshotTick) - tick,
      movementMode: main.dataset.movementMode,
      motions,
      frames: profile.skyCoherenceFrames.slice(initial),
    }
  }, seconds)
  await page.keyboard.up("w")
  const after = await canvas.screenshot()
  const image = decodeScreenshot(after)
  const pixels = { samples: 0, distinct: new Set<number>(), upper: 0, lower: 0 }
  for (let y = 20; y < image.height - 20; y += 8) {
    for (let x = 20; x < image.width - 20; x += 8) {
      const offset = (y * image.width + x) * image.channels
      const red = image.pixels[offset]!, green = image.pixels[offset + 1]!, blue = image.pixels[offset + 2]!
      pixels.samples += 1
      pixels.distinct.add((red >> 3) << 10 | (green >> 3) << 5 | blue >> 3)
      if (y < image.height / 2 && red + green + blue > 50) pixels.upper += 1
      if (y >= image.height / 2 && red + green + blue > 50) pixels.lower += 1
    }
  }

  const frames = measurement.frames
  const authored = frames.filter((frame) => frame.sky !== null)
  const intervals = frames.slice(1).map((frame, index) => frame.completedAt - frames[index]!.completedAt)
  for (const frame of frames) {
    expect(frame.mainSurfaces).toBeGreaterThan(0)
    expect(frame.mainVisibilityIdentity).toMatch(/^[a-f0-9]{64}$/u)
    if (!frame.sky) {
      expect(frame.disposition).not.toBe("authored")
      continue
    }
    expect(frame.selection).toBe(2)
    expect(frame.disposition).toBe("authored")
    expect(frame.skyVisibilityIdentity).toMatch(/^[a-f0-9]{64}$/u)
    expect(frame.skySurfaces).toBeGreaterThan(0)
    expect(frame.sky.yawDegrees).toBe(frame.main.yawDegrees)
    expect(frame.sky.pitchDegrees).toBe(frame.main.pitchDegrees)
    expect(frame.sky.verticalFovDegrees).toBe(frame.main.verticalFovDegrees)
    expect(frame.controller?.area).toBeLessThan(255)
    for (let axis = 0; axis < 3; axis += 1) {
      expect(frame.sky.position[axis]).toBeCloseTo(frame.controller!.origin[axis]! + frame.main.position[axis]! / Math.max(1, frame.controller!.scale), 9)
    }
  }
  const displacement = frames.length > 1
    ? Math.hypot(...frames.at(-1)!.main.position.map((value, axis) => value - frames[0]!.main.position[axis]!)) : 0
  const report = {
    schema: "playsrc-headed-grounded-sky-coherence-v1", target: "pl_upward", headed: true,
    seconds: measurement.seconds, presentedFrames: frames.length, presentedFps: frames.length / measurement.seconds,
    frames: summarizeFrameTimes(intervals), asyncVisibility: summarizeFrameTimes(frames.map((frame) => frame.visibilityMilliseconds)),
    render: summarizeFrameTimes(frames.map((frame) => frame.renderMilliseconds)),
    simulationTicksPerSecond: measurement.ticks / measurement.seconds, displacement, motions: measurement.motions,
    authoredFrames: authored.length, orientations: new Set(authored.map((frame) => frame.main.yawDegrees)).size,
    controller: authored[0]?.controller ?? null,
    visibilityIdentities: new Set(authored.map((frame) => frame.skyVisibilityIdentity)).size,
    pixels: { samples: pixels.samples, distinct: pixels.distinct.size, upper: pixels.upper, lower: pixels.lower },
    screenshots: { before: createHash("sha256").update(before).digest("hex"), after: createHash("sha256").update(after).digest("hex") },
    first: frames[0], last: frames.at(-1),
  }
  const local = await loadLocalConfig()
  const directory = path.join(local.sourceCacheDir, "evidence", "tf2-sky-camera-coherence")
  await mkdir(directory, { recursive: true })
  await Promise.all([
    writeFile(path.join(directory, "report.json"), `${JSON.stringify(report, null, 2)}\n`),
    writeFile(path.join(directory, "before.png"), before),
    writeFile(path.join(directory, "after.png"), after),
  ])
  await testInfo.attach("headed-grounded-authored-sky-coherence", { body: JSON.stringify(report, null, 2), contentType: "application/json" })
  await testInfo.attach("headed-grounded-authored-sky-pixels", { body: after, contentType: "image/png" })
  console.log(`PLAYSRC_SKY_COHERENCE ${JSON.stringify(report)}`)
  expect(measurement.movementMode).not.toBe("1")
  expect(report.authoredFrames).toBeGreaterThan(20)
  expect(report.orientations).toBeGreaterThan(5)
  expect(report.displacement).toBeGreaterThan(8)
  expect(report.simulationTicksPerSecond).toBeGreaterThan(55)
  expect(report.pixels.distinct).toBeGreaterThan(40)
  expect(report.pixels.upper).toBeGreaterThan(100)
  expect(report.pixels.lower).toBeGreaterThan(100)
  expect(report.screenshots.before).not.toBe(report.screenshots.after)
})
