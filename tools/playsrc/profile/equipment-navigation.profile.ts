import { createHash } from "node:crypto"
import { readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { test, expect, guardStartupInput } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { startupConsoleIdle, startupNativeReader } from "./native-startup"
import { requireStartupNative } from "./static-startup-gate"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"
import { captureProcessMemory } from "./process-memory"
import { decodeScreenshot } from "./screenshot-pixels"
import { nativeSelectionRect } from "./selection-transition-analysis"
import { selectionVisibleLatency } from "./selection-visible-latency"
import { nativeEquipment } from "../../../games/tf2/browser/tests/fixtures/equipment"

test.use({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 })

test("equipment trusted input to native visible pages", async ({ page, context }) => {
  test.setTimeout(100_000)
  if (process.platform !== "win32") throw new Error("Matched equipment navigation requires the configured native Windows browser")
  const directory = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!, { sourceCacheDir } = await loadLocalConfig()
  if (!directory) throw new Error("Use the checked equipment-navigation runner")
  const reader = await startupNativeReader(page, sourceCacheDir)
  const cdp = await context.newCDPSession(page), browser = await context.browser()!.newBrowserCDPSession()
  const captures: any[] = [], references: any[] = [], hits: unknown[] = [], errors: string[] = []
  let sampling = false, complete = false, nextCapture: Promise<number> | undefined, loop: Promise<void> | undefined
  let failure: string | undefined
  let cpuActive = false
  const cpu = process.env.PLAYSRC_EQUIPMENT_CPU === "1"
  guardStartupInput(page, async () => { requireStartupNative(await reader.read()) })
  page.on("pageerror", error => errors.push(error.message))
  await page.addInitScript(installBrowserFrameProfiler)
  await page.addInitScript(() => {
    const profile = (globalThis as any).__playsrcProfile = { inputs: [], captureEquipment: false, equipmentFrames: [] }
    const input = (event: Event) => {
      if (!profile.captureEquipment || !event.isTrusted) return
      profile.inputs.push({ type: event.type, key: (event as KeyboardEvent).code, trusted: event.isTrusted,
        inputEpoch: performance.timeOrigin + event.timeStamp, processing: performance.now() })
    }
    window.addEventListener("pointerup", input, true)
    window.addEventListener("keydown", input, true)
  })
  const capture = async () => {
    const file = `equipment-${captures.length}.desktop.png`, startedEpoch = Date.now()
    const admission = await reader.read(path.join(directory, file), "window"), admissionAfter = await reader.read()
    requireStartupNative(admission); requireStartupNative(admissionAfter)
    if (!admission.pixels) throw new Error("Native equipment capture is absent")
    const bytes = await readFile(path.join(directory, file))
    captures.push({ file, startedEpoch, endedEpoch: Date.now(), admission, admissionAfter, nativeRecords: reader.records.slice(-2),
      byteLength: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex"), privacy: "private-desktop-never-upload" })
    return captures.length - 1
  }
  const selector = (name: string) => `.equipment-layer [data-vgui-name='${name}']`
  const click = async (name: string, fraction = 0.5) => {
    const hit = await page.locator(name === "CharacterSetupButton" ? `[data-vgui-name='${name}']` : selector(name)).evaluate((node, fraction) => {
      const bounds = node.getBoundingClientRect(), x = bounds.x + bounds.width * fraction, y = bounds.y + bounds.height * fraction
      const target = document.elementFromPoint(x, y)
      return { x, y, name: (node as HTMLElement).dataset.vguiName, hit: (target as HTMLElement)?.dataset.vguiName, inside: !!target && node.contains(target) }
    }, fraction)
    hits.push(hit); expect(hit.inside, `${name} must own the actual hit pixel`).toBe(true)
    await page.mouse.click(hit.x, hit.y)
  }
  const reference = async (name: string) => {
    await expect(page.locator(selector(name))).toBeVisible({ timeout: 2500 })
    const facts = await page.locator(selector(name)).evaluate(node => ({ bounds: node.getBoundingClientRect().toJSON(), screenX, screenY, outerWidth, outerHeight,
      innerWidth, innerHeight, epoch: performance.timeOrigin + performance.now(), input: (globalThis as any).__playsrcProfile.inputs.at(-1) }))
    let index = await nextCapture!
    while (captures[index].startedEpoch < facts.epoch) {
      if (!sampling) throw new Error(`No native reference for ${name} within the bounded sample`)
      index = await nextCapture!
    }
    references.push({ name, index, facts })
  }
  try {
    await page.goto("/", { waitUntil: "domcontentloaded" })
    await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu")
    requireStartupNative(await reader.read())
    await click("CharacterSetupButton")
    await expect(page.locator(selector("Class3"))).toBeVisible()
    await click("Class3")
    await expect(page.locator(".equipment-layer")).toHaveAttribute("data-preview-model", "models/player/soldier.mdl", { timeout: 20_000 })
    await click("BackButton")
    await expect(page.locator(selector("Class3"))).toBeVisible()
    await page.waitForTimeout(2100)
    if (await startupConsoleIdle(sourceCacheDir) < 2000) throw new Error("Equipment input requires two seconds of genuine native idle")
    await capture()
    const heapBefore = await cdp.send("Runtime.getHeapUsage")
    const residentBefore = await captureProcessMemory((await browser.send("SystemInfo.getProcessInfo")).processInfo)
    if (cpu) { await cdp.send("Profiler.enable"); await cdp.send("Profiler.start"); cpuActive = true }
    await page.evaluate(() => { (globalThis as any).__playsrcProfile.captureEquipment = true; (globalThis as any).__playsrcFrameProfiler.active = true })
    const startedEpoch = Date.now()
    sampling = true
    loop = (async () => {
      while (sampling && Date.now() - startedEpoch < 9000 && (!complete || Date.now() - startedEpoch < 5000)) { nextCapture = capture(); await nextCapture }
      sampling = false
    })()
    void loop.catch(() => {})
    await click("Class3"); await reference("Itemslot-0")
    await click("Itemslot-0"); await reference("UnequipButton")
    await page.keyboard.press("Escape"); await reference("Itemslot-0")
    await click("BackpackButton"); await reference("Itemitem-18")
    await page.keyboard.press("PageDown")
    await reference(`Itemitem-${nativeEquipment.inventory[50]!.item.definitionIndex}`)
    await page.keyboard.press("Escape"); await reference("Class3")
    complete = true
    await loop
    const endedEpoch = Date.now()
    if (cpu) { await writeFile(path.join(directory, "equipment.cpuprofile"), JSON.stringify((await cdp.send("Profiler.stop")).profile)); cpuActive = false }
    const evidence = await page.evaluate(() => {
      const p = (globalThis as any).__playsrcProfile, f = (globalThis as any).__playsrcFrameProfiler
      p.captureEquipment = false; f.active = false
      return { inputs: p.inputs, equipmentFrames: p.equipmentFrames, worker: f.worker, counters: f.counters, modelPreparation: f.modelPreparation,
        longTasks: f.longTasks, longAnimationFrames: f.longAnimationFrames, completedFrames: f.completedFrames }
    })
    const heapAfter = await cdp.send("Runtime.getHeapUsage"), residentAfter = await captureProcessMemory((await browser.send("SystemInfo.getProcessInfo")).processInfo)
    await writeFile(path.join(directory, "equipment-measurement.json"), JSON.stringify({ cpu, startedEpoch, endedEpoch, evidence, references, heapBefore, heapAfter, residentBefore, residentAfter }, null, 2))
    const images = await Promise.all(captures.map(async capture => decodeScreenshot(await readFile(path.join(directory, capture.file)))))
    const latencies = references.map(reference => {
      const image = images[reference.index]!
      const rects = captures.map((capture, index) => nativeSelectionRect(images[index]!, capture.admission.pixels.bounds, capture.nativeRecords[0].facts.bounds, reference.facts))
      const expected = rects[reference.index]!
      let distinct = 0
      for (let y = 0; y < expected.height; y++) for (let x = 0; x < expected.width; x++) {
        const at = ((expected.y + y) * image.width + expected.x + x) * image.channels
        if (image.pixels[at]! > 100) distinct++
      }
      expect(distinct, `${reference.name} must contain actual authored pixels`).toBeGreaterThan(16)
      return { name: reference.name, input: reference.facts.input, ...selectionVisibleLatency(reference.facts.input.inputEpoch, endedEpoch, captures.map((capture, index) => {
        const sampled = images[index]!, rect = rects[index]!
        let different = 0
        for (let y = 0; y < expected.height; y++) for (let x = 0; x < expected.width; x++) {
          const a = ((expected.y + y) * image.width + expected.x + x) * image.channels, b = ((rect.y + y) * sampled.width + rect.x + x) * sampled.channels
          if ([0, 1, 2].some(channel => Math.abs(image.pixels[a + channel]! - sampled.pixels[b + channel]!) > 2)) different++
        }
        return { ...capture.admission.pixels, matches: different < expected.width * expected.height * 0.01 }
      })) }
    })
    await writeFile(path.join(directory, "equipment-latency.json"), JSON.stringify(latencies, null, 2))
    await page.screenshot({ path: path.join(directory, "equipment.page.png") })
    expect(errors).toEqual([])
  } catch (error) { failure = String(error); throw error }
  finally {
    sampling = false; await loop?.catch(() => {})
    if (cpuActive) await writeFile(path.join(directory, "equipment.cpuprofile"), JSON.stringify((await cdp.send("Profiler.stop")).profile))
    if (failure) {
      const evidence = await page.evaluate(() => {
        const p = (globalThis as any).__playsrcProfile, f = (globalThis as any).__playsrcFrameProfiler
        if (p) p.captureEquipment = false
        if (f) f.active = false
        return { inputs: p?.inputs, equipmentFrames: p?.equipmentFrames, worker: f?.worker, counters: f?.counters, modelPreparation: f?.modelPreparation,
          longTasks: f?.longTasks, longAnimationFrames: f?.longAnimationFrames, endedEpoch: Date.now() }
      }).catch(error => ({ unavailable: String(error) }))
      await writeFile(path.join(directory, "equipment-failure.json"), JSON.stringify({ failure, evidence }, null, 2))
    }
    await writeFile(path.join(directory, "equipment-native.json"), JSON.stringify({ captures, references, hits, errors, failure }, null, 2))
    await reader.close()
  }
})
