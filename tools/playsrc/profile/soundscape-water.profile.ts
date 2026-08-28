import { test, expect } from "./application-test"
import { chooseTf2Team } from "./team-selection-evidence"
import { writeFile } from "node:fs/promises"

test("Lakeside trigger entries and exits change real mixed audio including underwater processing", async ({ page }, testInfo) => {
  await page.addInitScript(() => {
    const profile = ((globalThis as any).__playsrcProfile = { longTasks: [] as unknown[], coldPhases: [] as unknown[] })
    new PerformanceObserver(list => { for (const task of list.getEntries()) if (profile.longTasks.length < 128) profile.longTasks.push(task.toJSON()) }).observe({ type: "longtask", buffered: true })
  })
  const main = page.locator("main"), errors: string[] = []
  page.on("pageerror", error => errors.push(error.message))
  const command = async (value: string) => {
    await page.evaluate(value => { const profile = (globalThis as any).__playsrcProfile; profile.coldPhases.push({ command: value, at: performance.now(), audio: profile.audio?.stats() }) }, value)
    if (await main.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    const entry = page.locator("[aria-label='Console command']")
    await entry.fill(value); await entry.press("Enter"); await page.keyboard.press("Backquote")
  }
  await page.goto("/")
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 60_000 })
  await command("map koth_lakeside_final")
  await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 60_000 })
  await chooseTf2Team(page, "red")
  await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 30_000 })
  const cpu = process.env.PROFILE_AUDIO_CPU === "1" ? await page.context().newCDPSession(page) : undefined
  if (cpu) {
    await cpu.send("Profiler.enable")
    await cpu.send("Profiler.setSamplingInterval", { interval: 1000 })
    await cpu.send("Profiler.start")
  }
  await page.locator("canvas.world-canvas").click({ force: true })
  // Trigger *24 covers the shallow lake shore. The deep z=1 water volume
  // is inside the temple, not this shore (whose water surface is z=-248).
  await command("setpos 20 1100 -240")
  await page.waitForFunction(() => {
    const profile = (globalThis as any).__playsrcProfile, audio = profile.audio?.stats()
    return profile.soundscape?.entity === 1 && audio && !audio.underwater && audio.contextState === "running" && audio.room >= 60 && audio.activeVoices > 0
  }, undefined, { timeout: 5000 }).catch(async error => {
    await writeFile(testInfo.outputPath("lake-admission-failure.json"), JSON.stringify(await page.evaluate(() => {
      const profile = (globalThis as any).__playsrcProfile
      return { player: profile.player, selection: profile.soundscape, audio: profile.audio?.stats(), dataset: { ...document.querySelector<HTMLElement>("main")!.dataset } }
    })))
    await page.screenshot({ path: testInfo.outputPath("lake-admission-failure.png") })
    throw error
  })
  const lake = await page.evaluate(() => ({ selection: (globalThis as any).__playsrcProfile.soundscape, audio: (globalThis as any).__playsrcProfile.audio.stats(), player: (globalThis as any).__playsrcProfile.player }))
  await page.screenshot({ path: testInfo.outputPath("lake-shore.png") })
  await command("setpos 0 -1200 64")
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.soundscape?.entity === 4, undefined, { timeout: 5000 })
  const temple = await page.evaluate(() => ({ selection: (globalThis as any).__playsrcProfile.soundscape, audio: (globalThis as any).__playsrcProfile.audio.stats() }))
  await command("setpos 128 -1472 -100")
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.audio.stats().underwater, undefined, { timeout: 5000 }).catch(async error => {
    await writeFile(testInfo.outputPath("temple-water-failure.json"), JSON.stringify(await page.evaluate(() => {
      const profile = (globalThis as any).__playsrcProfile
      return { player: profile.player, selection: profile.soundscape, audio: profile.audio?.stats(), dataset: { ...document.querySelector<HTMLElement>("main")!.dataset } }
    })))
    await page.screenshot({ path: testInfo.outputPath("temple-water-failure.png") })
    throw error
  })
  const water = await page.evaluate(() => ({ selection: (globalThis as any).__playsrcProfile.soundscape, audio: (globalThis as any).__playsrcProfile.audio.stats(), player: (globalThis as any).__playsrcProfile.player }))
  expect(water.audio.soundscape).toBe(temple.audio.soundscape)
  await page.screenshot({ path: testInfo.outputPath("temple-underwater.png") })
  const recording = page.evaluate(async () => {
    const profile = (globalThis as any).__playsrcProfile, audio = profile.audio, before = audio.stats()
    profile.waterCaptureStart = before.renderedFrames
    const captured = await audio.capture(308700), after = audio.stats(), bytes = new Uint8Array(captured.pcm)
    let binary = ""; for (let at = 0; at < bytes.length; at += 8192) binary += String.fromCharCode(...bytes.subarray(at, at + 8192))
    return { before, after, captured: { sampleFormat: captured.sampleFormat, differingSamples: captured.differingSamples, uncoveredSamples: captured.uncoveredSamples, underruns: captured.underruns, frames: captured.frames }, pcm: btoa(binary) }
  })
  await page.waitForFunction(() => {
    const profile = (globalThis as any).__playsrcProfile
    return profile.audio.stats().renderedFrames - profile.waterCaptureStart >= 220500
  }, undefined, { timeout: 8000 })
  await command("setpos 0 208 64")
  await page.waitForFunction(() => {
    const profile = (globalThis as any).__playsrcProfile
    return profile.soundscape?.entity === 5 && !profile.audio.stats().underwater
  }, undefined, { timeout: 5000 })
  const outside = await page.evaluate(() => ({ selection: (globalThis as any).__playsrcProfile.soundscape, audio: (globalThis as any).__playsrcProfile.audio.stats(), player: (globalThis as any).__playsrcProfile.player }))
  await page.screenshot({ path: testInfo.outputPath("lake-exit.png") })
  const captured = await recording
  if (cpu) {
    await writeFile(testInfo.outputPath("cold-admission.cpuprofile"), JSON.stringify((await cpu.send("Profiler.stop")).profile))
    await cpu.detach()
  }
  await writeFile(testInfo.outputPath("lakeside-capture.json"), JSON.stringify({ ...captured, pcm: undefined }))
  await writeFile(testInfo.outputPath("lakeside-raw.pcm"), Buffer.from(captured.pcm, "base64"))
  expect(captured.captured.differingSamples).toBe(0)
  expect(captured.captured.uncoveredSamples).toBe(0)
  expect(captured.captured.underruns).toBe(0)
  expect(outside.audio.soundscape).not.toBe(temple.audio.soundscape)
  const pcm = Buffer.from(captured.pcm, "base64")
  let nonzero = 0, waterStereoDifferences = 0
  for (let frame = 0; frame < 176400; frame++) {
    const left = pcm.readFloatLE(frame * 8), right = pcm.readFloatLE(frame * 8 + 4)
    nonzero += Number(left !== 0 || right !== 0); waterStereoDifferences += Number(left !== right)
  }
  expect(nonzero).toBeGreaterThan(44100)
  expect(waterStereoDifferences).toBe(0)
  const header = Buffer.alloc(44)
  header.write("RIFF"); header.writeUInt32LE(pcm.length + 36, 4); header.write("WAVEfmt ", 8); header.writeUInt32LE(16, 16)
  header.writeUInt16LE(3, 20); header.writeUInt16LE(2, 22); header.writeUInt32LE(44100, 24); header.writeUInt32LE(352800, 28)
  header.writeUInt16LE(8, 32); header.writeUInt16LE(32, 34); header.write("data", 36); header.writeUInt32LE(pcm.length, 40)
  const audioPath = testInfo.outputPath("lakeside-mixed.wav")
  await writeFile(audioPath, Buffer.concat([header, pcm]))
  await testInfo.attach("owned-underwater-audio", { path: audioPath, contentType: "audio/wav" })
  await command("setpos 0 0 64")
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.soundscape?.entity === 0, undefined, { timeout: 5000 })
  const exited = await page.evaluate(() => ({ selection: (globalThis as any).__playsrcProfile.soundscape, audio: (globalThis as any).__playsrcProfile.audio.stats() }))
  expect(exited.audio.soundscape).toBe(outside.audio.soundscape)
  const diagnostics = await page.evaluate(() => { const profile = (globalThis as any).__playsrcProfile; return { longTasks: profile.longTasks, coldPhases: profile.coldPhases } })
  await writeFile(testInfo.outputPath("lakeside-audio.json"), JSON.stringify({ lake, temple, water, outside, exited, captured: { ...captured, pcm: undefined }, nonzero, waterStereoDifferences, diagnostics, errors }))
  expect(exited.audio.underrunFrames).toBe(0)
  expect(exited.audio.extraPaintCalls).toBeGreaterThan(0)
  expect(errors).toEqual([])
})
