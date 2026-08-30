import { createHash } from "node:crypto"
import { readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { test, expect, guardStartupInput } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { startupConsoleIdle, startupNativeReader } from "./native-startup"
import { requireStartupNative } from "./static-startup-gate"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"
import { captureProcessMemory } from "./process-memory"
import { analyzeEquipmentNavigation } from "./equipment-navigation-analysis"
import { nativeEquipment } from "../../../games/tf2/browser/tests/fixtures/equipment"
import { profileArtifact } from "./profile-artifacts"

async function mediaRecord(file: string) {
  const bytes = await readFile(file)
  return { path: file, byteLength: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex") }
}

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
    const first = index
    while (index <= first) {
      if (!sampling) throw new Error(`No settled native reference for ${name} within the bounded sample`)
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
    const retention: unknown[] = []
    for (let cycle = 0; cycle < 3; cycle++) {
      await click("Class3"); await click("BackpackButton"); await page.keyboard.press("Escape")
      await expect(page.locator(selector("Class3"))).toBeVisible()
      retention.push({ cycle, heap: await cdp.send("Runtime.getHeapUsage"), dom: await cdp.send("Memory.getDOMCounters") })
    }
    requireStartupNative(await reader.read())
    await page.screenshot({ path: path.join(directory, "equipment.page.png") })
    expect(errors).toEqual([])
    await profileArtifact(() => writeFile(path.join(directory, "equipment-measurement.json"), JSON.stringify({ cpu, startedEpoch, endedEpoch, evidence, references, heapBefore, heapAfter, residentBefore, residentAfter, retention }, null, 2)))
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
      await profileArtifact(() => writeFile(path.join(directory, "equipment-failure.json"), JSON.stringify({ failure, evidence }, null, 2)))
    }
    await profileArtifact(async () => {
      const pageMedia = await mediaRecord(path.join(directory, "equipment.page.png")).catch(error => ({ unavailable: String(error) }))
      await writeFile(path.join(directory, "equipment-native.json"), JSON.stringify({ captures, pageMedia, references, hits, errors, failure }, null, 2))
      await analyzeEquipmentNavigation(directory)
    })
    await reader.close()
  }
})

test.describe("equipment transaction faults", () => {
  test.use({ allowRecoverableApplicationFailure: true })
  test("equipment pending Back, retry, resource failure and reopen", async ({ page, context }) => {
    test.setTimeout(100_000)
    if (process.platform !== "win32") throw new Error("Equipment transaction acceptance requires native Windows")
    const directory = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!, { sourceCacheDir } = await loadLocalConfig()
    const reader = await startupNativeReader(page, sourceCacheDir), records: unknown[] = [], media: string[] = []
    guardStartupInput(page, async () => { requireStartupNative(await reader.read()) })
    await page.addInitScript(() => {
      const inputs: unknown[] = (globalThis as any).__equipmentTransactionInputs = []
      for (const kind of ["pointerup", "keydown"]) window.addEventListener(kind, event => {
        if (event.isTrusted) inputs.push({ kind, code: (event as KeyboardEvent).code, inputEpoch: performance.timeOrigin + event.timeStamp })
      }, true)
    })
    const equipment = page.locator(".equipment-layer"), main = page.locator("main")
    const control = (name: string) => equipment.locator(`[data-vgui-name='${name}']`)
    const gate = Promise.withResolvers<void>(), intercepted = Promise.withResolvers<string>()
    let fault: "none" | "delay" | "reject" = "none"
    await context.route(url => url.hostname === "127.0.0.1" && /^\/objects\/sha256\/[a-f0-9]{64}$/.test(url.pathname), async route => {
      const selected = fault
      if (selected === "none") { await route.continue(); return }
      fault = "none"
      records.push({ kind: "local-resource-fault", fault: selected, url: route.request().url(), at: Date.now() })
      if (selected === "reject") { await route.abort("failed"); return }
      intercepted.resolve(route.request().url())
      await gate.promise
      await route.abort("failed")
    })
    const capture = async (name: string) => {
      requireStartupNative(await reader.read())
      await page.screenshot({ path: path.join(directory, `${name}.page.png`) })
      const native = await reader.read(path.join(directory, `${name}.desktop.png`), "window")
      requireStartupNative(native)
      media.push(path.join(directory, `${name}.desktop.png`), path.join(directory, `${name}.page.png`))
      requireStartupNative(await reader.read())
      records.push({ kind: "capture", name, native, inputs: await page.evaluate(() => (globalThis as any).__equipmentTransactionInputs) })
    }
    try {
      await page.goto("/", { waitUntil: "domcontentloaded" })
      await expect(main).toHaveAttribute("data-phase", "MainMenu")
      await page.locator("[data-vgui-name='CharacterSetupButton']").click()
      await control("Class3").click()
      await expect(equipment).toHaveAttribute("data-preview-model", "models/player/soldier.mdl", { timeout: 20_000 })
      await page.keyboard.press("Backquote")
      const entry = page.locator("[aria-label='Console command']")
      await entry.fill("echo equipment input ownership")
      await page.keyboard.press("ArrowLeft")
      await expect(entry).toHaveValue("echo equipment input ownership")
      await page.keyboard.press("Escape")
      await expect(main).toHaveAttribute("data-console-visible", "true")
      await expect(entry).toHaveValue("echo equipment input ownership")
      await expect(control("Itemslot-0")).toBeVisible()
      await page.keyboard.press("Backquote")
      await expect(main).toHaveAttribute("data-console-visible", "false")
      await page.waitForTimeout(2100)
      if (await startupConsoleIdle(sourceCacheDir) < 2000) throw new Error("Transaction test requires genuine native idle")
      await capture("loadout")
      await control("Itemslot-0").hover()
      await expect(control("ItemTooltip")).toBeVisible()
      await capture("tooltip")
      await control("Itemslot-0").click()
      const before = await page.evaluate(() => localStorage.getItem("playsrc.tf2.local-equipment.v1"))
      fault = "delay"
      await control("Itemitem-127").click()
      await Promise.race([intercepted.promise, new Promise((_, reject) => setTimeout(() => reject(new Error("Cold equipment resource was not intercepted")), 5000))])
      const back = await control("BackButton").boundingBox()
      const point = { x: back!.x + back!.width * 0.95, y: back!.y + back!.height * 0.5 }
      expect(await page.evaluate(point => !!document.elementFromPoint(point.x, point.y)?.closest("[data-vgui-name='BackButton']"), point)).toBe(true)
      await page.mouse.click(point.x, point.y)
      await expect(control("Itemslot-0")).toBeVisible({ timeout: 1500 })
      await capture("back-cancelled-pending")
      await control("Itemslot-0").click(); await control("Itemitem-127").click()
      await page.keyboard.press("Escape")
      await expect(control("Itemslot-0")).toBeVisible({ timeout: 1500 })
      expect(await page.evaluate(() => localStorage.getItem("playsrc.tf2.local-equipment.v1"))).toBe(before)
      await capture("cancelled-pending")
      await control("Itemslot-0").click(); await control("Itemitem-127").click()
      gate.resolve()
      await expect(control("Itemslot-0")).toBeVisible({ timeout: 20_000 })
      await expect.poll(() => page.evaluate(() => localStorage.getItem("playsrc.tf2.local-equipment.v1"))).not.toBe(before)
      const saved = await page.evaluate(() => localStorage.getItem("playsrc.tf2.local-equipment.v1"))
      await expect(equipment).toHaveAttribute("data-preview-model", "models/player/soldier.mdl")
      await capture("equipped-retry")
      await control("BackButton").click(); await control("BackButton").click()
      await expect(equipment).toBeHidden()
      await page.locator("[data-vgui-name='CharacterSetupButton']").click()
      await page.keyboard.press("ArrowLeft"); await page.keyboard.press("ArrowRight")
      await page.keyboard.down("Enter"); await page.keyboard.down("Enter")
      await expect(control("Class3")).toBeVisible()
      await expect(control("Itemslot-0")).toHaveCount(0)
      await capture("held-enter")
      await page.keyboard.up("Enter")
      await expect(control("Itemslot-0")).toBeVisible({ timeout: 20_000 })
      expect(await page.evaluate(() => localStorage.getItem("playsrc.tf2.local-equipment.v1"))).toBe(saved)
      await control("Itemslot-0").click()
      fault = "reject"
      await control("Itemitem-228").click()
      await expect(main).toHaveAttribute("data-phase", "Failed", { timeout: 15_000 })
      await expect(equipment).toBeHidden()
      expect(await page.evaluate(() => localStorage.getItem("playsrc.tf2.local-equipment.v1"))).toBe(saved)
      await capture("resource-failure")
      await page.reload({ waitUntil: "domcontentloaded" })
      await expect(main).toHaveAttribute("data-phase", "MainMenu")
      expect(await page.evaluate(() => localStorage.getItem("playsrc.tf2.local-equipment.v1"))).toBe(saved)
    } catch (error) {
      records.push({ kind: "failure", error: String(error) })
      await capture("failed-transaction").catch(error => records.push({ kind: "capture-unavailable", error: String(error) }))
      throw error
    } finally {
      gate.resolve()
      await reader.close()
      await profileArtifact(async () => writeFile(path.join(directory, "equipment-transactions.json"), JSON.stringify({ kind: "local-resource-fault-injection-not-a-performance-sample", records,
        media: await Promise.all(media.map(mediaRecord)) }, null, 2)))
    }
  })
})

test("equipment map replacement and disconnect cancel uncommitted selection", async ({ page, context }) => {
  test.setTimeout(120_000)
  const directory = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!, { sourceCacheDir } = await loadLocalConfig()
  const reader = await startupNativeReader(page, sourceCacheDir), records: unknown[] = [], media: string[] = []
  guardStartupInput(page, async () => { requireStartupNative(await reader.read()) })
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = { captureWeaponPoses: true } })
  let gate = Promise.withResolvers<void>(), entered = Promise.withResolvers<void>(), armed = false
  await context.route(url => url.hostname === "127.0.0.1" && /^\/objects\/sha256\/[a-f0-9]{64}$/.test(url.pathname), async route => {
    if (!armed) { await route.continue(); return }
    armed = false
    const held = gate
    records.push({ kind: "held-resource", url: route.request().url(), at: Date.now() })
    entered.resolve(); await held.promise
    if (!route.request().failure()) await route.continue()
  })
  const equipment = page.locator(".equipment-layer"), main = page.locator("main")
  const control = (name: string) => equipment.locator(`[data-vgui-name='${name}']`)
  const command = async (text: string) => {
    if (await main.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    await page.locator("[aria-label='Console command']").fill(text); await page.keyboard.press("Enter")
  }
  const holdEquip = async (slot = 0, definition = 127) => {
    await expect(equipment).toHaveAttribute("data-preview-model", "models/player/soldier.mdl", { timeout: 20_000 })
    await control(`Itemslot-${slot}`).click()
    gate = Promise.withResolvers<void>(); entered = Promise.withResolvers<void>(); armed = true
    await control(`Itemitem-${definition}`).click()
    await Promise.race([entered.promise, new Promise((_, reject) => setTimeout(() => reject(new Error("Equipment lifecycle resource gate was not reached")), 5000))])
  }
  const capture = async (name: string) => {
    const native = await reader.read(path.join(directory, `${name}.desktop.png`), "window")
    requireStartupNative(native); requireStartupNative(await reader.read())
    await page.screenshot({ path: path.join(directory, `${name}.page.png`) })
    media.push(path.join(directory, `${name}.desktop.png`), path.join(directory, `${name}.page.png`))
    records.push({ name, native })
  }
  try {
    await page.goto("/", { waitUntil: "domcontentloaded" })
    await expect(main).toHaveAttribute("data-phase", "MainMenu")
    await page.waitForTimeout(2100)
    if (await startupConsoleIdle(sourceCacheDir) < 2000) throw new Error("Equipment lifecycle requires genuine native idle")
    await page.locator("[data-vgui-name='CharacterSetupButton']").click(); await control("Class3").click()
    const saved = await page.evaluate(() => localStorage.getItem("playsrc.tf2.local-equipment.v1"))
    await holdEquip()
    await command("map pl_upward")
    await expect(equipment).toBeHidden({ timeout: 1500 })
    gate.resolve()
    await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 55_000 })
    if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await page.locator(".team-selection-layer [data-vgui-name='teambutton1']").click()
    await page.locator(".class-selection-layer [data-vgui-name='soldier']").click()
    await expect(main).toHaveAttribute("data-phase", "Ready")
    await expect.poll(() => page.evaluate(() => (globalThis as any).__playsrcProfile.weaponPose?.definition)).toBe(18)
    expect(await page.evaluate(() => localStorage.getItem("playsrc.tf2.local-equipment.v1"))).toBe(saved)
    await capture("map-cancelled-equip")
    await page.keyboard.press("Comma")
    await page.locator(".class-selection-layer [data-vgui-name='EditLoadoutButton']").click()
    await holdEquip(7, 378)
    await command("disconnect")
    await expect(equipment).toBeHidden({ timeout: 1500 })
    gate.resolve()
    await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 15_000 })
    expect(await page.evaluate(() => localStorage.getItem("playsrc.tf2.local-equipment.v1"))).toBe(saved)
    await capture("disconnect-cancelled-equip")
  } finally {
    gate.resolve(); await reader.close()
    await profileArtifact(async () => writeFile(path.join(directory, "equipment-lifecycle.json"), JSON.stringify({ records, media: await Promise.all(media.map(mediaRecord)) }, null, 2)))
  }
})
