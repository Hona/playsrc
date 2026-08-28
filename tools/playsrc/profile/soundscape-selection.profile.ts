import { test, expect } from "./application-test"
import { chooseTf2Team } from "./team-selection-evidence"
import { writeFile } from "node:fs/promises"

test("configured Granary mixed PCM, soundscape transitions, pause and map ownership", async ({ page }, testInfo) => {
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = {} })
  const main = page.locator("main")
  const errors: string[] = []
  page.on("pageerror", error => errors.push(error.message))
  const command = async (value: string) => {
    if (await main.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    const entry = page.locator("[aria-label='Console command']")
    await entry.fill(value); await entry.press("Enter")
    await page.keyboard.press("Backquote")
  }
  const selection = () => page.evaluate(() => (globalThis as any).__playsrcProfile.soundscape)
  const failure = async (error: unknown) => {
    await writeFile(testInfo.outputPath("room-failure.json"), JSON.stringify(await page.evaluate(() => {
      const profile = (globalThis as any).__playsrcProfile
      return { player: profile.player, selection: profile.soundscape, audio: profile.audio?.stats() }
    })))
    await page.screenshot({ path: testInfo.outputPath("room-failure.png") })
    throw error
  }
  await page.goto("/")
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 60_000 })
  await command("map cp_granary")
  await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 60_000 })
  await chooseTf2Team(page, "red")
  await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 30_000 })
  await page.locator("canvas.world-canvas").click({ force: true })
  // Walkable red yard/spawn positions from configured cp_granary BSP.
  // Stay clear of the gantry directly above the capture point, which correctly
  // rejects an automatic-room scan with side sky but a solid overhead ray.
  // 6080d8b69cd526aa3e76252d61138ccd4d371953ea0ef2787e5a56bd3399f08d.
  await command("setpos -1000 -5200 -450")
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.soundscape?.entity === 49, undefined, { timeout: 5000 }).catch(failure)
  const outside = await selection()
  await page.waitForFunction(() => {
    const audio = (globalThis as any).__playsrcProfile.audio?.stats()
    return audio?.contextState === "running" && audio.soundscape === 19 && audio.room >= 60 && audio.roomObservation?.[0] === 1 && audio.activeVoices >= 4
  }, undefined, { timeout: 5000 }).catch(failure)
  await page.screenshot({ path: testInfo.outputPath("outside-selection.png") })
  // Record first-view admission separately from the steady acoustic crossing.
  // Do not hide cold renderer stalls inside a claimed zero-underrun lifecycle.
  const coldBefore = await page.evaluate(() => (globalThis as any).__playsrcProfile.audio.stats())
  await command("setpos -1328 -6528 -281")
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.audio.stats().soundscape === 20, undefined, { timeout: 5000 }).catch(failure)
  await page.screenshot({ path: testInfo.outputPath("inside-selection.png") })
  const coldAfter = await page.evaluate(() => (globalThis as any).__playsrcProfile.audio.stats())
  await writeFile(testInfo.outputPath("cold-view-admission.json"), JSON.stringify({ coldBefore, coldAfter }))
  await command("setpos -1000 -5200 -450")
  await page.waitForFunction(() => {
    const audio = (globalThis as any).__playsrcProfile.audio.stats()
    return audio.soundscape === 19 && audio.roomObservation?.[0] === 1
  }, undefined, { timeout: 5000 }).catch(failure)
  const recording = page.evaluate(async () => {
    const profile = (globalThis as any).__playsrcProfile, audio = profile.audio
    profile.audioOwner = audio
    const before = audio.stats(), root = document.querySelector<HTMLElement>("main")!, tick = Number(root.dataset.snapshotTick)
    profile.audioCaptureStart = before.renderedFrames
    const start = performance.now(), frames: number[] = []; let previous = start, complete = false
    const sample = (now: number) => { if (complete) return; frames.push(now - previous); previous = now; requestAnimationFrame(sample) }
    requestAnimationFrame(sample)
    const captured = await audio.capture(441000)
    complete = true
    const after = audio.stats(), raw = new Uint8Array(captured.pcm)
    let binary = ""
    for (let at = 0; at < raw.length; at += 8192) binary += String.fromCharCode(...raw.subarray(at, at + 8192))
    return { before, after, frames, seconds: (performance.now() - start) / 1000, ticks: Number(root.dataset.snapshotTick) - tick,
      captured: { frames: captured.frames, differingSamples: captured.differingSamples, uncoveredSamples: captured.uncoveredSamples, underruns: captured.underruns, sampleRate: captured.sampleRate }, pcm: btoa(binary) }
  })
  await page.waitForFunction(() => {
    const profile = (globalThis as any).__playsrcProfile
    return profile.audio.stats().renderedFrames - profile.audioCaptureStart >= 348390
  })
  await command("setpos -1328 -6528 -281")
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.soundscape?.entity === 47, undefined, { timeout: 5000 }).catch(failure)
  const inside = await selection()
  expect(inside.soundscape).not.toBe(outside.soundscape)
  expect(inside.positions).toHaveLength(8)
  await page.waitForFunction(() => {
    const audio = (globalThis as any).__playsrcProfile.audio.stats()
    return audio.soundscape === 20 && audio.roomObservation?.[0] === 0
  }, undefined, { timeout: 5000 }).catch(failure)
  const sample = await recording
  await writeFile(testInfo.outputPath("granary-capture.json"), JSON.stringify({ ...sample, pcm: undefined }))
  expect(sample.captured.differingSamples).toBe(0)
  expect(sample.captured.uncoveredSamples).toBe(0)
  expect(sample.captured.underruns).toBe(0)
  expect(sample.after.mp3Frames).toBeGreaterThan(0)
  expect(sample.after.room).not.toBe(sample.before.room)
  expect(sample.ticks / sample.seconds).toBeGreaterThan(63)
  const pcm = Buffer.from(sample.pcm, "base64")
  let nonzero = 0, peak = 0, stereoDifferences = 0
  for (let at = 0; at < pcm.length; at += 4) {
    const left = pcm.readInt16LE(at), right = pcm.readInt16LE(at + 2)
    nonzero += Number(left !== 0 || right !== 0); peak = Math.max(peak, Math.abs(left), Math.abs(right)); stereoDifferences += Number(left !== right)
  }
  expect(nonzero).toBeGreaterThan(44100)
  expect(stereoDifferences).toBeGreaterThan(0)
  const header = Buffer.alloc(44)
  header.write("RIFF"); header.writeUInt32LE(pcm.length + 36, 4); header.write("WAVEfmt ", 8); header.writeUInt32LE(16, 16)
  header.writeUInt16LE(1, 20); header.writeUInt16LE(2, 22); header.writeUInt32LE(44100, 24); header.writeUInt32LE(176400, 28)
  header.writeUInt16LE(4, 32); header.writeUInt16LE(16, 34); header.write("data", 36); header.writeUInt32LE(pcm.length, 40)
  const audioPath = testInfo.outputPath("granary-mixed.wav")
  await writeFile(audioPath, Buffer.concat([header, pcm]))
  await testInfo.attach("mixed-owned-audio", { path: audioPath, contentType: "audio/wav" })
  await page.keyboard.press("Escape")
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.audio.stats().contextState === "suspended")
  const paused = await page.evaluate(() => (globalThis as any).__playsrcProfile.audio.stats())
  await page.evaluate(() => new Promise(resolve => setTimeout(resolve, 250)))
  expect(await page.evaluate(() => (globalThis as any).__playsrcProfile.audio.stats().renderedFrames)).toBe(paused.renderedFrames)
  await page.keyboard.press("Escape")
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.audio.stats().contextState === "running")
  const priorGeneration = await main.getAttribute("data-generation")
  await command("map cp_granary")
  await expect(main).not.toHaveAttribute("data-generation", priorGeneration!)
  await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 45_000 })
  const replacement = await page.evaluate(() => ({ sameOwner: (globalThis as any).__playsrcProfile.audioOwner === (globalThis as any).__playsrcProfile.audio, stats: (globalThis as any).__playsrcProfile.audio.stats() }))
  expect(replacement.sameOwner).toBe(true)
  expect(replacement.stats.epoch).toBeGreaterThan(sample.after.epoch)
  const filename = testInfo.outputPath("soundscape-selection.json")
  await writeFile(filename, JSON.stringify({ outside, inside, coldBefore, coldAfter, sample: { ...sample, pcm: undefined }, nonzero, peak, stereoDifferences, paused, replacement, errors, graphPcmMatchesPaint: true }))
  await testInfo.attach("soundscape-selection", { path: filename, contentType: "application/json" })
  expect(errors).toEqual([])
})
