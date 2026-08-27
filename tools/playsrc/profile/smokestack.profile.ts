import { test, expect } from "./application-test"
import { chooseTf2Team } from "./team-selection-evidence"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"
import { decodeScreenshot } from "./screenshot-pixels"
import { loadLocalConfig } from "../src/config"
import { readFile, writeFile } from "node:fs/promises"
import { spawnSync } from "node:child_process"
import path from "node:path"

const json = (value: unknown) => JSON.stringify(value, (_, value) => typeof value === "bigint" ? value.toString() : value)
test("Lakeside authored smokestacks, TurnOff drain, reentry and replacement", async ({ page }, testInfo) => {
  const config = await loadLocalConfig()
  const facts = JSON.parse(await readFile(path.join(config.sourceCacheDir, "evidence/map-runtime/koth_lakeside_final.facts.json"), "utf8"))
  expect(facts.smokestacks).toHaveLength(46)
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = {} })
  await page.addInitScript(installBrowserFrameProfiler)
  const main = page.locator("main")
  const errors: string[] = []
  page.on("pageerror", error => errors.push(error.message))
  const command = async (text: string) => {
    if (await main.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    const entry = page.locator("[aria-label='Console command']")
    await entry.fill(text); await entry.press("Enter")
  }
  const closeConsole = async () => { if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote") }
  let revision = 0
  const capture = async (name: string) => {
    const current = ++revision
    const captureDepth = name === "smoke-active" || name === "smoke-occluded-active"
    await page.evaluate(({ revision, depth }) => {
      const profile = (globalThis as any).__playsrcProfile
      profile.particleEvidenceRevision = revision; profile.hudPixelEvidenceRevision = revision; profile.geometryEvidenceRevision = revision
      if (depth) profile.cosmeticDepthRevision = revision
    }, { revision: current, depth: captureDepth })
    await page.waitForFunction(({ revision, depth }) => {
      const p = (globalThis as any).__playsrcProfile
      return p.particleEvidence?.revision === revision && p.hudPixelEvidence?.revision === revision && p.geometryEvidence?.revision === revision && (!depth || p.cosmeticDepthCapture?.revision === revision)
    }, { revision: current, depth: captureDepth })
    const evidence = await page.evaluate(depth => {
      const p = (globalThis as any).__playsrcProfile
      return { particles: p.particleEvidence, geometry: p.geometryEvidence, assets: p.memoryAssets, pixels: Array.from(p.hudPixelEvidence.before.bytes),
        depth: depth ? { bytes: Array.from(p.cosmeticDepthCapture.buffers.depth), width: p.cosmeticDepthCapture.buffers.width, height: p.cosmeticDepthCapture.buffers.height } : null }
    }, captureDepth)
    const bytes = Buffer.from(evidence.pixels as number[])
    await writeFile(testInfo.outputPath(`${name}.png`), bytes)
    await writeFile(testInfo.outputPath(`${name}.json`), json({ ...evidence, pixels: undefined, depth: evidence.depth ? { width: evidence.depth.width, height: evidence.depth.height } : null }))
    if (evidence.depth) await writeFile(testInfo.outputPath(`${name}-depth.rgba`), Buffer.from(evidence.depth.bytes as number[]))
    await testInfo.attach(name, { path: testInfo.outputPath(`${name}.png`), contentType: "image/png" })
    return { ...evidence, image: decodeScreenshot(bytes) }
  }
  const sample = async (name: string) => {
    const result = await page.evaluate(async () => {
      const p = (globalThis as any).__playsrcFrameProfiler, profile = (globalThis as any).__playsrcProfile
      const root = document.querySelector<HTMLElement>("main")!
      const start = performance.now(), tick = Number(root.dataset.snapshotTick)
      const counters = { ...p.counters }
      p.completedFrames.length = 0; p.gpuTimestamps.length = 0; p.worker.length = 0; p.active = true
      const raf: number[] = []; let previous = start
      await new Promise<void>(resolve => { const frame = (now: number) => { raf.push(now - previous); previous = now; if (now - start >= 5000) resolve(); else requestAnimationFrame(frame) }; requestAnimationFrame(frame) })
      p.active = false
      return { seconds: (performance.now() - start) / 1000, ticks: Number(root.dataset.snapshotTick) - tick, raf,
        countersBefore: counters, countersAfter: p.counters, frames: p.completedFrames, gpu: p.gpuTimestamps, worker: p.worker,
        bots: profile.bots, assets: profile.memoryAssets, heap: (performance as any).memory?.usedJSHeapSize }
    })
    await writeFile(testInfo.outputPath(`${name}.json`), json(result))
    expect.soft(result.ticks / result.seconds).toBeGreaterThan(63)
    expect(result.bots).toHaveLength(15)
    expect(result.countersAfter.validationErrors).toBe(0)
    return result
  }
  await page.goto("/"); await page.bringToFront()
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 60_000 })
  const loading = performance.now()
  await command("map koth_lakeside_final")
  await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 60_000 })
  await closeConsole(); await chooseTf2Team(page, "red")
  await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 30_000 })
  const loadingMilliseconds = performance.now() - loading
  await command("tf_bot_quota 15"); await closeConsole()
  await expect(main).toHaveAttribute("data-bot-count", "15")
  await page.locator("canvas.world-canvas").click()
  await expect(main).toHaveAttribute("data-pointer-locked", "true")
  const beforeInput = await page.evaluate(() => (globalThis as any).__playsrcProfile.player.position)
  await page.keyboard.down("w"); await page.waitForTimeout(400); await page.keyboard.up("w")
  const afterInput = await page.evaluate(() => (globalThis as any).__playsrcProfile.player.position)
  expect(Math.hypot(afterInput[0] - beforeInput[0], afterInput[1] - beforeInput[1])).toBeGreaterThan(8)
  const stack = facts.smokestacks[0], origin = stack.position
  const camera = { position: [origin[0], origin[1] - 80, origin[2] + 20], yawDegrees: 90, pitchDegrees: 5 }
  await page.evaluate(camera => { (globalThis as any).__playsrcProfile.displacementCameraOverride = camera }, camera)
  await page.waitForTimeout(900)
  const active = await capture("smoke-active")
  const smoke = active.particles.items.filter((item: any) => item.effectIdentity >= 0x5000_0000 && item.effectIdentity < 0x6000_0000)
  expect(smoke.some((item: any) => item.effectIdentity === 0x5000_0000 + stack.identity)).toBe(true)
  expect(smoke.every((item: any) => item.material === "particle/smokesprites_0001" && !item.sky && item.sequence === 0 && item.animationRate === 0)).toBe(true)
  for (const item of smoke) {
    const authored = facts.smokestacks.find((source: any) => source.identity === item.effectIdentity - 0x5000_0000)
    const [r, g, b] = authored.color.split(" ").map(Number)
    expect(item.color).toBe((r << 16) | (g << 8) | b)
    expect(item.lifetimeSeconds).toBeCloseTo(Number(authored.jetLength) / Number(authored.speed), 6)
  }
  expect(active.geometry.geometry.samples.some((s: any) => s.disposition === "main-world" && s.depth > 0)).toBe(true)
  if (process.platform === "darwin") {
    const desktop = testInfo.outputPath("smoke-active-desktop.png")
    expect(spawnSync("/usr/sbin/screencapture", ["-x", desktop]).status).toBe(0)
    await testInfo.attach("native-desktop", { path: desktop, contentType: "image/png" })
  }
  const activeSample = await sample("smoke-active-performance")
  await command("ent_fire env_smokestack TurnOff"); await closeConsole()
  const lifetime = Math.max(...facts.smokestacks.map((s: any) => Number(s.jetLength) / Number(s.speed)))
  await page.waitForTimeout((lifetime + 0.25) * 1000)
  const stopped = await capture("smoke-stopped")
  expect(stopped.particles.items.filter((item: any) => item.effectIdentity >= 0x5000_0000 && item.effectIdentity < 0x6000_0000)).toHaveLength(0)
  const { image: before } = active, { image: after } = stopped
  let changed = 0, warm = 0
  // Central world-only region, excluding all HUD and the viewmodel.
  const roi = { x0: Math.floor(before.width * 0.35), x1: Math.ceil(before.width * 0.65), y0: Math.floor(before.height * 0.25), y1: Math.ceil(before.height * 0.6) }
  for (let y = roi.y0; y < roi.y1; y++) for (let x = roi.x0; x < roi.x1; x++) {
    const offset = (y * before.width + x) * before.channels
    if ([0, 1, 2].some(channel => Math.abs(before.pixels[offset + channel]! - after.pixels[offset + channel]!) > 5)) {
      changed++; if (before.pixels[offset]! > before.pixels[offset + 2]! + 10) warm++
    }
  }
  await writeFile(testInfo.outputPath("smoke-pixel-difference.json"), json({ changed, warm, roi, lifetime, beforeInput, afterInput, loadingMilliseconds }))
  expect(changed).toBeGreaterThan(20); expect(warm).toBeGreaterThan(20)
  const depth = active.depth!
  expect(depth.width).toBe(before.width); expect(depth.height).toBe(before.height)
  const visibleCenterDepth = Number(depth.bytes[(Math.floor(before.height / 2) * before.width + Math.floor(before.width / 2)) * 4 + 3]) * 192 / 255
  expect(visibleCenterDepth).toBeGreaterThan(0)
  const stoppedSample = await sample("smoke-stopped-performance")
  await command("ent_fire env_smokestack TurnOn"); await closeConsole(); await page.waitForTimeout(800)
  const restarted = await capture("smoke-restarted")
  expect(restarted.particles.items.some((item: any) => item.effectIdentity === 0x5000_0000 + stack.identity)).toBe(true)
  await page.evaluate(camera => { (globalThis as any).__playsrcProfile.displacementCameraOverride = { ...camera, yawDegrees: -90 } }, camera)
  await page.waitForTimeout(300)
  await page.evaluate(camera => { (globalThis as any).__playsrcProfile.displacementCameraOverride = camera }, camera)
  await page.waitForTimeout(300)
  expect((await capture("smoke-reentry")).particles.items.some((item: any) => item.effectIdentity === 0x5000_0000 + stack.identity)).toBe(true)
  const probe = facts.smokeOcclusion
  expect(probe).toBeTruthy()
  await page.evaluate(camera => { (globalThis as any).__playsrcProfile.displacementCameraOverride = camera }, probe)
  await page.waitForTimeout(900)
  const occluded = await capture("smoke-occluded-active")
  const occludedItems = occluded.particles.items.filter((item: any) => item.effectIdentity === 0x5000_0000 + probe.identity)
  expect(occludedItems.length).toBeGreaterThan(0)
  const occludedDepth = occluded.depth!
  const centerOffset = (Math.floor(occludedDepth.height / 2) * occludedDepth.width + Math.floor(occludedDepth.width / 2)) * 4
  const wallDepth = Number(occludedDepth.bytes[centerOffset + 3]) * 192 / 255
  const yaw = probe.yawDegrees * Math.PI / 180
  const particleDepths = occludedItems.map((item: any) => (item.position[0] - probe.position[0]) * Math.cos(yaw) + (item.position[1] - probe.position[1]) * Math.sin(yaw))
  expect(wallDepth).toBeGreaterThan(0)
  expect(wallDepth).toBeLessThan(Math.min(...particleDepths) - 2)
  await command("ent_fire env_smokestack TurnOff"); await closeConsole(); await page.waitForTimeout((lifetime + 0.25) * 1000)
  const occludedStopped = await capture("smoke-occluded-stopped")
  let occludedChanged = 0
  for (let y = before.height / 2 - 40; y < before.height / 2 + 40; y++) for (let x = before.width / 2 - 40; x < before.width / 2 + 40; x++) {
    const offset = (y * before.width + x) * before.channels
    if ([0, 1, 2].some(channel => Math.abs(occluded.image.pixels[offset + channel]! - occludedStopped.image.pixels[offset + channel]!) > 5)) occludedChanged++
  }
  await writeFile(testInfo.outputPath("smoke-depth-occlusion.json"), json({ probe, wallDepth, particleDepths, occludedChanged, pixels: 6400, visibleCenterDepth }))
  expect(occludedChanged).toBe(0)
  await page.evaluate(camera => { (globalThis as any).__playsrcProfile.displacementCameraOverride = camera }, camera)
  await command("map koth_lakeside_final")
  await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 60_000 })
  await closeConsole(); await chooseTf2Team(page, "red")
  await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 30_000 })
  await page.waitForTimeout(800)
  const replacement = await capture("smoke-map-replacement")
  expect(replacement.geometry.generation).toBeGreaterThan(active.geometry.generation)
  expect(replacement.particles.items.some((item: any) => item.effectIdentity === 0x5000_0000 + stack.identity)).toBe(true)
  // The old map's emission was turned off. The replacement must use its own
  // authored InitialState and owner generation, not carry that stopped client.
  await writeFile(testInfo.outputPath("smoke-summary.json"), json({ errors, activeSample, stoppedSample, authored: facts.smokestacks }))
  expect(errors).toEqual([])
})
