import { createHash } from "node:crypto"
import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, test } from "./application-test"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { loadLocalConfig } from "../src/config"
import { settleTf2Gameplay } from "./team-selection-evidence"

const WATER_MATERIAL = "materials/maps/pl_upward/water/water_hydro_cheap_dx80_7168_-2048_128.vmt"
const SAMPLE_MILLISECONDS = profileSampleSeconds() * 1_000

test("profiles exact authored LightmappedGeneric water animation through the headed gameplay renderer", async ({ page }, testInfo) => {
  await page.addInitScript(() => {
    const state: Record<string, unknown> = {}
    ;(window as typeof window & { __playsrcProfile?: Record<string, unknown> }).__playsrcProfile = state
  })
  await page.goto("/", { waitUntil: "load", timeout: 30_000 })
  await page.waitForFunction(() => ["MainMenu", "Failed"].includes(document.querySelector<HTMLElement>("main")?.dataset.phase ?? ""), undefined, { timeout: 180_000, polling: 25 })
  expect(await page.locator("main").getAttribute("data-phase")).toBe("MainMenu")
  const configuration = await (await page.request.get("/playsrc-config.json")).json() as { defaultTarget: string }
  expect(configuration.defaultTarget).toBe("pl_upward")

  await page.keyboard.press("Backquote")
  const command = page.locator("[aria-label='Console command']")
  await expect(command).toBeVisible()
  await command.fill("map pl_upward")
  const loadStarted = await page.evaluate(() => performance.now())
  await page.keyboard.press("Enter")
  await settleTf2Gameplay(page)
  expect(await page.locator("main").getAttribute("data-phase")).toBe("Ready")
  const loadFinished = await page.evaluate(() => performance.now())
  const load = JSON.parse((await page.locator("main").getAttribute("data-load-performance")) ?? "null") as {
    totalMilliseconds: number
    mapBytes: number
    presentationBytes: number
    client: { textureDecoderRequests: number; textureMetadataInspections: number }
  }
  if (!Number.isSafeInteger(load.client.textureMetadataInspections)
    || load.client.textureMetadataInspections < 1
    || load.client.textureDecoderRequests <= load.client.textureMetadataInspections) {
    throw new Error(`immutable VTF decoder did not eliminate repeated metadata inspection: ${JSON.stringify(load.client)}`)
  }

  const source = await page.evaluate((identity) => {
    const profile = (globalThis as typeof globalThis & { __playsrcProfile?: any }).__playsrcProfile
    const state = profile?.materialAnimation
    const material = state?.materials?.find((value: any) => value.identity === identity)
    const volume = state?.volumes?.find((value: any) => value.surfaceMaterial === material?.mapMaterial)
    const surface = state?.surfaces?.find((value: any) => value.material === material?.mapMaterial)
    return { material, volume, surface, generation: state?.generation }
  }, WATER_MATERIAL)
  if (!source.material || !source.volume || source.material.shader !== "lightmapped-generic"
    || source.material.proxies.length !== 1 || source.material.proxies[0]?.name !== "AnimatedTexture"
    || source.material.environmentMap?.tint.some((value: number) => value !== Math.fround(0.2))) {
    throw new Error(`configured upward animated world material is incomplete: ${JSON.stringify(source)}`)
  }
  const bump = source.material.textures.find((texture: any) => texture.role === 7)
  if (!bump || bump.frameCount !== 30 || bump.mipCount !== 9 || !bump.frameProxyMutated || bump.colorRead !== "linear") {
    throw new Error(`configured upward authored bump chain differs: ${JSON.stringify(bump)}`)
  }

  await page.keyboard.press("Backquote")
  const origin = await page.evaluate(() => {
    const value = document.querySelector<HTMLElement>("main")?.dataset.cameraPosition?.split(",").map(Number)
    if (!value || value.length !== 3 || !value.every(Number.isFinite)) throw new Error("authoritative player camera is unavailable")
    return value
  })
  let geometry: { water: any[]; stationary: any[]; generation: number; target: string; camera: any } | undefined
  let revision = 0
  for (const pitch of [15, 30, 0, 45]) {
    for (const yaw of [315, 0, 45, 90, 135, 180, 225, 270]) {
      revision += 1
      await page.evaluate(({ origin, yaw, pitch, revision }) => {
        const profile = (globalThis as typeof globalThis & { __playsrcProfile: any }).__playsrcProfile
        profile.displacementCameraOverride = { position: origin, yawDegrees: yaw, pitchDegrees: pitch }
        profile.geometryEvidenceRevision = revision
      }, { origin, yaw, pitch, revision })
      await page.waitForFunction(({ revision, generation }) => {
        const root = document.querySelector<HTMLElement>("main")
        const evidence = (globalThis as typeof globalThis & { __playsrcProfile?: any }).__playsrcProfile?.geometryEvidence
        return root?.dataset.phase === "Failed" || (evidence?.generation === generation && evidence.revision === revision)
      }, { revision, generation: source.generation }, { timeout: 15_000, polling: 20 })
      expect(await page.locator("main").getAttribute("data-phase")).toBe("Ready")
      const evidence = await page.evaluate((identity) => {
        const value = (globalThis as typeof globalThis & { __playsrcProfile?: any }).__playsrcProfile.geometryEvidence
        const main = value.geometry.samples as any[]
        const sky = value.skyGeometry?.samples as any[] | undefined
        const water = (sky ?? []).filter((sample, index) =>
          sample.material?.toLowerCase() === identity && main[index]?.disposition === "background",
        )
        const stationary = main.filter((sample) => sample.disposition === "main-world"
          && sample.material?.toLowerCase() !== identity)
        return { water, stationary, generation: value.generation, target: value.target, camera: value.camera }
      }, WATER_MATERIAL)
      if (evidence.water.length > 0 && evidence.stationary.length > 0) {
        geometry = evidence
        break
      }
    }
    if (geometry) break
  }
  if (!geometry) {
    throw new Error("headed 3D-sky depth evidence found no visible authored water pixel behind an unobstructed main-world ray")
  }
  const candidate = { identity: WATER_MATERIAL, camera: geometry.camera, bounds: source.volume.bounds, surfaceZ: source.volume.surfaceZ }
  await page.waitForTimeout(250)

  await page.evaluate(() => {
    const root = document.querySelector<HTMLElement>("main")!
    const canvas = document.querySelector<HTMLCanvasElement>("canvas.world-canvas")!
    const records: { at: number; tick: number; frame: number; detail: Record<string, number> }[] = []
    const profile = (globalThis as typeof globalThis & { __playsrcProfile: any }).__playsrcProfile
    profile.materialAnimationSamples = records
    profile.materialAnimationStarted = performance.now()
    profile.materialAnimationFirstTick = Number(root.dataset.snapshotTick)
    profile.materialAnimationFirstDisplay = Number(canvas.dataset.displayFrame)
    const capture = () => {
      const value = root.dataset.worldMaterialFrames ?? ""
      const frame = Number(value.slice(value.lastIndexOf(":") + 1))
      const detail = root.dataset.performanceDetail ? JSON.parse(root.dataset.performanceDetail) : null
      const tick = Number(root.dataset.snapshotTick)
      if (Number.isSafeInteger(frame) && detail && Number(detail.tick) === tick && records.at(-1)?.tick !== tick) {
        records.push({ at: performance.now(), tick, frame, detail })
      }
    }
    const observer = new MutationObserver(capture)
    observer.observe(root, { attributes: true, attributeFilter: ["data-world-material-frames", "data-performance-detail"] })
    profile.materialAnimationObserver = observer
  })
  const before = await page.locator("canvas.world-canvas").screenshot({ animations: "disabled" })
  await page.waitForTimeout(425)
  const after = await page.locator("canvas.world-canvas").screenshot({ animations: "disabled" })
  const motion = await page.evaluate(async ({ left, right, water, stationary }) => {
    const decode = async (encoded: string) => {
      const bytes = Uint8Array.from(atob(encoded), (value) => value.charCodeAt(0))
      const image = await createImageBitmap(new Blob([bytes], { type: "image/png" }))
      const canvas = new OffscreenCanvas(image.width, image.height)
      const context = canvas.getContext("2d", { willReadFrequently: true })
      if (!context) throw new Error("headed material screenshot decoder is unavailable")
      context.drawImage(image, 0, 0)
      return { width: image.width, height: image.height, pixels: context.getImageData(0, 0, image.width, image.height).data }
    }
    const first = await decode(left), second = await decode(right)
    if (first.width !== second.width || first.height !== second.height) throw new Error("headed screenshots changed size")
    const measure = (samples: any[]) => samples.map((sample) => {
      const centerX = Math.round((sample.x + 1) * 0.5 * first.width)
      const centerY = Math.round((1 - sample.y) * 0.5 * first.height)
      let changed = 0, maximumDelta = 0, total = 0
      const firstPixel: number[] = [], secondPixel: number[] = []
      for (let y = Math.max(0, centerY - 8); y < Math.min(first.height, centerY + 8); y++) {
        for (let x = Math.max(0, centerX - 8); x < Math.min(first.width, centerX + 8); x++) {
          const at = (y * first.width + x) * 4
          const delta = Math.max(...[0, 1, 2].map((channel) => Math.abs(first.pixels[at + channel]! - second.pixels[at + channel]!)))
          if (x === centerX && y === centerY) {
            firstPixel.push(...first.pixels.slice(at, at + 4))
            secondPixel.push(...second.pixels.slice(at, at + 4))
          }
          maximumDelta = Math.max(maximumDelta, delta)
          changed += Number(delta > 2)
          total++
        }
      }
      return { x: centerX, y: centerY, depth: sample.depth, primitive: sample.primitive,
        material: sample.material, changed, total, fraction: changed / total, maximumDelta, firstPixel, secondPixel }
    })
    return { width: first.width, height: first.height, water: measure(water), stationary: measure(stationary) }
  }, {
    left: before.toString("base64"),
    right: after.toString("base64"),
    water: geometry.water,
    stationary: geometry.stationary,
  })
  const animatedPixels = motion.water.filter((sample) => sample.changed > 0 && sample.maximumDelta > 2)
  if (animatedPixels.length === 0) throw new Error(`authored upward bump frames did not change real headed water pixels: ${JSON.stringify(motion)}`)
  const stableWorld = motion.stationary.filter((sample) => sample.fraction < 0.1)
  if (stableWorld.length === 0) throw new Error(`stationary world pixels moved with the authored water: ${JSON.stringify(motion)}`)

  const remaining = await page.evaluate((sampleMilliseconds) => {
    const started = Number((globalThis as typeof globalThis & { __playsrcProfile?: any }).__playsrcProfile.materialAnimationStarted)
    return Math.max(0, sampleMilliseconds - (performance.now() - started))
  }, SAMPLE_MILLISECONDS)
  if (remaining > 0) await page.waitForTimeout(remaining)
  const timeline = await page.evaluate(() => {
    const root = document.querySelector<HTMLElement>("main")!
    const canvas = document.querySelector<HTMLCanvasElement>("canvas.world-canvas")!
    const profile = (globalThis as typeof globalThis & { __playsrcProfile?: any }).__playsrcProfile
    profile.materialAnimationObserver.disconnect()
    const finished = performance.now()
    return {
      milliseconds: finished - profile.materialAnimationStarted,
      tickDelta: Number(root.dataset.snapshotTick) - Number(profile.materialAnimationFirstTick),
      displayDelta: Number(canvas.dataset.displayFrame) - Number(profile.materialAnimationFirstDisplay),
      records: profile.materialAnimationSamples as { at: number; tick: number; frame: number; detail: Record<string, number> }[],
      finalPhase: root.dataset.phase,
    }
  })
  expect(timeline.finalPhase).toBe("Ready")
  if (timeline.milliseconds < SAMPLE_MILLISECONDS || timeline.milliseconds > SAMPLE_MILLISECONDS + 2_000) {
    throw new Error(`headed material animation sampling escaped its five-second bound: ${timeline.milliseconds}`)
  }
  const uniqueFrames = [...new Set(timeline.records.map((record) => record.frame))].sort((a, b) => a - b)
  if (uniqueFrames.length !== 30 || uniqueFrames.some((frame, index) => frame !== index)) {
    throw new Error(`authored 30-frame bump chain was not fully selected: ${JSON.stringify(uniqueFrames)}`)
  }
  const mismatched = timeline.records.filter((record) => {
    const expected = Math.trunc(Math.fround(Math.fround(record.tick * 0.015) * 30)) % 30
    return record.frame !== expected
  })
  if (mismatched.length > 0) throw new Error(`Rust AnimatedTexture proxy timeline diverged from exact Source frame selection: ${JSON.stringify(mismatched.slice(0, 5))}`)
  const ticksPerSecond = timeline.tickDelta * 1_000 / timeline.milliseconds
  if (ticksPerSecond < 55 || ticksPerSecond > 75) {
    throw new Error(`authored material animation changed the fixed TF2 simulation cadence: ${ticksPerSecond}`)
  }
  const samples = timeline.records.map((record) => Number(record.detail.total)).filter(Number.isFinite).sort((a, b) => a - b)
  if (samples.length < 30) throw new Error(`headed animation produced too few actual frame samples: ${samples.length}`)
  const frameTimes = summarizeFrameTimes(samples)
  if (frameTimes.p95Milliseconds > 1_000 / 120) {
    throw new Error(`authored material animation exceeded its visible 120-Hz frame-work budget: p95=${frameTimes.p95Milliseconds}`)
  }

  await page.evaluate(() => {
    delete (globalThis as typeof globalThis & { __playsrcProfile?: any }).__playsrcProfile.displacementCameraOverride
  })
  await page.keyboard.press("Backquote")
  await expect(command).toBeVisible()
  await command.fill("map jump_beef")
  await page.keyboard.press("Enter")
  await page.waitForFunction(() => {
    const root = document.querySelector<HTMLElement>("main")
    return root?.dataset.phase === "Failed"
      || (root?.dataset.phase === "Ready" && root.dataset.detail === "Playing jump_beef")
  }, undefined, { timeout: 600_000, polling: 25 })
  expect(await page.locator("main").getAttribute("data-phase")).toBe("Ready")
  await page.keyboard.press("Backquote")
  await page.evaluate(() => {
    ;(globalThis as typeof globalThis & { __playsrcProfile?: any }).__playsrcProfile.displacementCameraOverride = {
      position: [-4800, 3000, -2100], yawDegrees: 0, pitchDegrees: 20,
    }
  })
  await page.waitForFunction(() => {
    const root = document.querySelector<HTMLElement>("main")
    return root?.dataset.phase === "Failed" || (root?.dataset.waterPlan?.startsWith("above:")
      && root.dataset.waterPasses === "reflection,refraction,main" && root.dataset.waterNormalFrame !== undefined)
  }, undefined, { timeout: 30_000, polling: 25 })
  expect(await page.locator("main").getAttribute("data-phase")).toBe("Ready")
  const preserved = await page.evaluate(() => {
    const root = document.querySelector<HTMLElement>("main")!
    const profile = (globalThis as typeof globalThis & { __playsrcProfile?: any }).__playsrcProfile
    return {
      generation: Number(root.dataset.generation),
      target: profile.materialAnimation.target,
      worldMaterials: profile.materialAnimation.materials.length,
      worldMaterialFrames: root.dataset.worldMaterialFrames ?? null,
      waterPlan: root.dataset.waterPlan,
      waterPasses: root.dataset.waterPasses,
      normalFrame: Number(root.dataset.waterNormalFrame),
      load: JSON.parse(root.dataset.loadPerformance ?? "null"),
    }
  })
  if (preserved.target !== "jump_beef" || preserved.worldMaterials !== 0 || preserved.worldMaterialFrames !== ""
    || preserved.waterPasses !== "reflection,refraction,main" || !Number.isSafeInteger(preserved.normalFrame)) {
    throw new Error(`map replacement retained stale world animation or changed existing Water rendering: ${JSON.stringify(preserved)}`)
  }
  const preservedWater = await page.locator("canvas.world-canvas").screenshot({ animations: "disabled" })

  const report = {
    schema: "playsrc-tf2-material-animation-profile-v1",
    headed: true,
    target: "pl_upward",
    generation: source.generation,
    material: source.material,
    camera: candidate.camera,
    sourceVolume: candidate,
    loading: {
      consoleToReadyMilliseconds: loadFinished - loadStarted,
      ...load,
      metadataReuses: load.client.textureDecoderRequests - load.client.textureMetadataInspections,
    },
    pixels: motion,
    sampling: {
      milliseconds: timeline.milliseconds,
      ticks: timeline.tickDelta,
      ticksPerSecond,
      displayFrames: timeline.displayDelta,
      observedFrames: uniqueFrames,
      sourceTimelineSamples: timeline.records.length,
      ...frameTimes,
    },
    lifecycle: preserved,
    captureHashes: {
      before: createHash("sha256").update(before).digest("hex"),
      after: createHash("sha256").update(after).digest("hex"),
      existingWater: createHash("sha256").update(preservedWater).digest("hex"),
    },
  }
  const local = await loadLocalConfig()
  const output = path.join(local.sourceCacheDir, "evidence", "tf2-material-animation")
  await mkdir(output, { recursive: true })
  await Promise.all([
    writeFile(path.join(output, "pl-upward-frame-before.png"), before),
    writeFile(path.join(output, "pl-upward-frame-after.png"), after),
    writeFile(path.join(output, "jump-beef-preserved-water.png"), preservedWater),
    writeFile(path.join(output, "report.json"), `${JSON.stringify(report, null, 2)}\n`),
  ])
  await testInfo.attach("headed-material-animation", { body: Buffer.from(JSON.stringify(report, null, 2)), contentType: "application/json" })
  console.log(`PLAYSRC_MATERIAL_ANIMATION ${JSON.stringify({
    target: report.target,
    loadMilliseconds: Number(report.loading.consoleToReadyMilliseconds.toFixed(3)),
    metadataInspections: report.loading.client.textureMetadataInspections,
    metadataReuses: report.loading.metadataReuses,
    changedWaterPixels: animatedPixels[0]!.changed,
    stationaryPixels: stableWorld[0]!.total - stableWorld[0]!.changed,
    sampleSeconds: SAMPLE_MILLISECONDS / 1_000,
    ticksPerSecond: Number(report.sampling.ticksPerSecond.toFixed(3)),
    authoredFrames: report.sampling.observedFrames.length,
    ...frameTimes,
    replacementTarget: report.lifecycle.target,
    replacementPasses: report.lifecycle.waterPasses,
  })}`)
})
