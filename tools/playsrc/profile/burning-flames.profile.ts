import { execFileSync } from "node:child_process"
import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { test, expect } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { chooseTf2Team } from "./team-selection-evidence"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"
import { captureProcessMemory } from "./process-memory"
import { decodeScreenshot } from "./screenshot-pixels"
import { summarizeFrameTimes } from "./profile-window"
import { cosmeticDepthEvidence } from "./cosmetic-depth-evidence"

test("Burning Flames Team Captain: real backpack equip, preview, two actors and depth", async ({ page, browser }) => {
  test.setTimeout(170_000)
  page.setDefaultTimeout(10_000)
  const directory = path.join((await loadLocalConfig()).sourceCacheDir, "evidence/burning-flames", `complete-${Date.now()}`)
  await mkdir(directory, { recursive: true })
  const errors: string[] = []
  let faviconMisses = 0
  page.on("pageerror", error => errors.push(error.message))
  page.on("console", message => { if (message.type() === "error") {
    if (message.location().url.endsWith("/favicon.ico") && message.text().includes("404")) faviconMisses++
    else errors.push(`${message.text()} ${message.location().url}`)
  } })
  await page.addInitScript(installBrowserFrameProfiler)
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = { captureCosmetics: true } })
  const main = page.locator("main"), canvas = page.locator("canvas.world-canvas"), equipment = page.locator(".equipment-layer")
  const command = async (text: string) => {
    if (await main.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    await page.locator("[aria-label='Console command']").fill(text); await page.keyboard.press("Enter")
    if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  }
  const shot = async (name: string) => { const bytes = await canvas.screenshot(); await writeFile(path.join(directory, `${name}.png`), bytes); return decodeScreenshot(bytes) }
  const desktop = (name: string) => { if (process.platform === "darwin") execFileSync("screencapture", ["-x", path.join(directory, `${name}-desktop.png`)], { timeout: 5000 }) }
  const records: Record<string, unknown> = { directory }
  const closeEquipment = async () => {
    for (let i = 0; i < 3 && await equipment.locator("[data-vgui-name='BackButton']").isVisible(); i++) await equipment.locator("[data-vgui-name='BackButton']").click()
    await expect(equipment).toBeHidden()
  }
  try {
    await page.goto("/")
    await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 30_000 })
    await page.locator("[data-vgui-name='CharacterSetupButton']").click()
    await equipment.locator("[data-vgui-name='Class3']").click()
    await equipment.locator("[data-vgui-name='BackpackButton']").click()
    const hat = equipment.locator("[data-vgui-name='Itemitem-378']")
    await expect(hat).toHaveCount(1)
    const bounds = (await hat.boundingBox())!
    const backpack = decodeScreenshot(await page.screenshot({ path: path.join(directory, "backpack.png") }))
    const ratio = backpack.width / page.viewportSize()!.width
    let purple = 0
    for (let y = Math.floor(bounds.y * ratio); y < (bounds.y + bounds.height) * ratio; y++) for (let x = Math.floor(bounds.x * ratio); x < (bounds.x + bounds.width) * ratio; x++) {
      const offset = (y * backpack.width + x) * backpack.channels
      const [r, g, b] = backpack.pixels.subarray(offset, offset + 3)
      if (r! > 60 && b! > 60 && r! > g! * 1.3 && b! > g! * 1.3) purple++
    }
    expect(purple).toBeGreaterThan(20); records.purplePixels = purple
    desktop("backpack")
    await hat.click(); await equipment.locator("[data-vgui-name='Itemitem-378']").click()
    await expect(equipment.locator("[data-vgui-name='EquipmentPlayer']")).toBeVisible()
    await closeEquipment()
    await command("map pl_upward")
    await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 60_000 })
    await chooseTf2Team(page, "red")
    await page.keyboard.press("Digit2")
    await expect(main).toHaveAttribute("data-class-selection-visible", "false")
    await page.keyboard.press("Comma")
    await page.locator(".class-selection-layer [data-vgui-name='EditLoadoutButton']").click()
    await page.waitForFunction(() => (globalThis as any).__playsrcProfile.cosmeticPreview?.particles >= 12)
    records.preview = await page.evaluate(() => (globalThis as any).__playsrcProfile.cosmeticPreview)
    await page.screenshot({ path: path.join(directory, "preview.png") }); desktop("preview")
    await closeEquipment()
    await command("nb_stop 1")
    await command("tf_bot_add 2 soldier blue easy")
    await page.waitForFunction(() => (globalThis as any).__playsrcProfile.bots?.filter((bot: any) => bot.class === 3).length >= 2)
    const actors = await page.evaluate(() => { const p = (globalThis as any).__playsrcProfile; return p.bots.filter((bot: any) => bot.class === 3).slice(0, 2).map((bot: any) => ({ identity: bot.identity, name: p.combat.scores.find((player: any) => player.identity === bot.identity).name })) })
    // Open area 4 in the configured NAV: [-1200,325,508] to [-775,750,510].
    await command("setpos -1060 537.5 512")
    await page.waitForFunction(() => (globalThis as any).__playsrcProfile.displacementCamera?.position[0] === -1060)
    await page.evaluate(() => { const p = (globalThis as any).__playsrcProfile; p.displacementCameraOverride = { position: p.displacementCamera.position, yawDegrees: 0, pitchDegrees: 0 } })
    for (const [index, actor] of actors.entries()) {
      const y = 495.5 + index * 84
      await command(`bot_teleport "${actor.name}" -920 ${y} 512 0 180 0`)
      await page.waitForFunction(({ identity, y }) => { const bot = (globalThis as any).__playsrcProfile.bots?.find((bot: any) => bot.identity === identity); return bot?.position[0] === -920 && bot?.position[1] === y }, { identity: actor.identity, y })
    }
    records.fixture = await page.evaluate(() => (globalThis as any).__playsrcProfile.bots)
    await shot("fixture-world")
    await page.waitForFunction(() => (globalThis as any).__playsrcProfile.cosmetics?.actors.length === 2)
    const cdp = await browser.newBrowserCDPSession()
    const processes = (await cdp.send("SystemInfo.getProcessInfo")).processInfo.map((p: any) => ({ id: p.id, type: p.type }))
    const sample = async (label: string) => {
      if (process.env.PROFILE_COSMETIC_VISUAL_ONLY === "1") { records[label] = { measured: false }; await page.waitForTimeout(200); return }
      const memoryBefore = await captureProcessMemory(processes)
      const cpuBefore = await cdp.send("SystemInfo.getProcessInfo")
      await page.evaluate(() => { const p = (globalThis as any).__playsrcFrameProfiler; p.completedFrames = []; p.gpuTimestamps = []; p.worker = []; p.simulation = []; p.active = true; p.sampleStart = performance.now() })
      await page.waitForTimeout(5000)
      const data = await page.evaluate(() => { const p = (globalThis as any).__playsrcFrameProfiler; p.active = false; return { elapsed: performance.now() - p.sampleStart, frames: p.completedFrames, gpu: p.gpuTimestamps, worker: p.worker, simulation: p.simulation, counters: p.counters, losses: p.losses, quality: (globalThis as any).__playsrcProfile.videoQuality, cosmetics: (globalThis as any).__playsrcProfile.cosmetics } })
      const memoryAfter = await captureProcessMemory(processes), cpuAfter = await cdp.send("SystemInfo.getProcessInfo")
      const intervals = data.frames.slice(1).map((f: any, i: number) => f.at - data.frames[i].at)
      records[label] = { ...data, memoryBefore, memoryAfter, cpuBefore, cpuAfter, frameTimes: summarizeFrameTimes(intervals) }
      expect(data.frames.length).toBeGreaterThan(30); expect(data.losses).toEqual([])
    }
    await sample("baseline")
    const before = await shot("unequipped-world")
    await page.evaluate(items => { (globalThis as any).__playsrcProfile.cosmeticBotEquip = { revision: 1, items } }, actors.map((actor: any) => ({ actor: actor.identity, definition: 378 })))
    await page.waitForFunction(() => { const p = (globalThis as any).__playsrcProfile; return p.cosmeticBotEquipResult?.complete && p.cosmetics?.models.length === 2 && p.cosmetics.particles.length >= 24 && new Set(p.cosmetics.particles.map((i: any) => i.effectIdentity)).size === 2 })
    const first = await shot("two-actors-a")
    await sample("equipped")
    const second = await shot("two-actors-b"); desktop("two-actors")
    const cosmetics = await page.evaluate(() => (globalThis as any).__playsrcProfile.cosmetics)
    records.actors = actors; records.cosmetics = cosmetics
    // The camera and stationary actors are unchanged; require changing fire pixels on each side.
    const counts = [0, 0], changed = [0, 0]
    for (let y = 0; y < second.height * 0.75; y++) for (let x = 0; x < second.width; x++) {
      const at = (y * second.width + x) * second.channels, side = x < second.width / 2 ? 0 : 1
      const [r, g, b] = second.pixels.subarray(at, at + 3)
      if (r! > 80 && r! > g! * 1.1 && g! > b! * 1.15 && r! > before.pixels[at]! + 15) counts[side]!++
      if (r! > 80 && g! > b! * 1.15 && Math.abs(r! - first.pixels[at]!) > 15) changed[side]!++
    }
    records.firePixels = counts; records.changedPixels = changed
    expect(counts.every(value => value > 10)).toBe(true); expect(changed.every(value => value > 10)).toBe(true)
    await page.evaluate(() => { (globalThis as any).__playsrcProfile.cosmeticDepthRevision = 1 })
    await page.waitForFunction(() => (globalThis as any).__playsrcProfile.cosmeticDepthCapture?.revision === 1)
    const depth = await page.evaluate(() => { const c = (globalThis as any).__playsrcProfile.cosmeticDepthCapture; const { before, depth, ...size } = c.buffers;
      const encode = (bytes: Uint8Array) => { let text = ""; for (let i = 0; i < bytes.length; i += 32768) text += String.fromCharCode(...bytes.subarray(i, i + 32768)); return btoa(text) };
      return { ...c, buffers: { ...size, before: encode(before), depth: encode(depth) } } })
    await writeFile(path.join(directory, "depth-before.rgba"), Buffer.from(depth.buffers.before, "base64"))
    await writeFile(path.join(directory, "depth.rgba"), Buffer.from(depth.buffers.depth, "base64"))
    await writeFile(path.join(directory, "depth-after.png"), Buffer.from(depth.pixels.split(",")[1], "base64"))
    records.depth = { ...depth, buffers: { ...depth.buffers, before: "depth-before.rgba", depth: "depth.rgba" }, pixels: "depth-after.png" }
    const depthProof = cosmeticDepthEvidence({ ...depth.buffers, camera: depth.camera, particles: depth.particles,
      before: Buffer.from(depth.buffers.before, "base64"), depth: Buffer.from(depth.buffers.depth, "base64"), after: Buffer.from(depth.pixels.split(",")[1], "base64") })
    records.depthProof = depthProof
    expect(depthProof.occluded).toBeGreaterThan(10)
    expect(depthProof.unchanged / depthProof.occluded).toBeGreaterThan(0.9)
    expect(depthProof.visibleChanged).toBeGreaterThan(20)
    await page.evaluate(actor => { (globalThis as any).__playsrcProfile.cosmeticBotEquip = { revision: 2, items: [{ actor, definition: null }] } }, actors[0].identity)
    await page.waitForFunction(() => { const p = (globalThis as any).__playsrcProfile; return p.cosmeticBotEquipResult?.revision === 2 && p.cosmeticBotEquipResult.complete && p.cosmetics.models.length === 1 && new Set(p.cosmetics.particles.map((i: any) => i.effectIdentity)).size === 1 })
    await shot("one-actor-after-unequip")
    records.afterUnequip = await page.evaluate(() => (globalThis as any).__playsrcProfile.cosmetics)
    const previousEffect = (records.afterUnequip as any).particles[0].effectIdentity
    await command(`bot_whack "${actors[1].name}"`)
    await page.waitForFunction(identity => { const p = (globalThis as any).__playsrcProfile; return p.bots.find((bot: any) => bot.identity === identity)?.lifecycle !== 1 && p.cosmetics.models.length === 0 && p.cosmetics.particles.length === 0 }, actors[1].identity)
    await shot("death-cleanup")
    await page.waitForFunction(identity => { const p = (globalThis as any).__playsrcProfile; return p.bots.find((bot: any) => bot.identity === identity)?.lifecycle === 1 && p.cosmetics.particles.length > 0 }, actors[1].identity, { timeout: 20_000 })
    expect(await page.evaluate(() => (globalThis as any).__playsrcProfile.cosmetics.particles[0].effectIdentity)).not.toBe(previousEffect)
    await command(`bot_teleport "${actors[1].name}" -920 579.5 512 0 180 0`)
    await page.waitForFunction(() => (globalThis as any).__playsrcProfile.cosmetics.particles.filter((p: any) => Math.abs(p.position[0] + 920) < 50 && Math.abs(p.position[1] - 579.5) < 50).length >= 12)
    await shot("respawn-effect")
    records.afterRespawn = await page.evaluate(() => (globalThis as any).__playsrcProfile.cosmetics)
    await page.evaluate(() => { delete (globalThis as any).__playsrcProfile.displacementCameraOverride })
    await command("map pl_upward")
    await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 45_000 })
    await chooseTf2Team(page, "blue")
    await page.keyboard.press("Digit2")
    await page.waitForFunction(() => { const p = (globalThis as any).__playsrcProfile; return p.cosmetics.models.length === 0 && p.cosmetics.particles.length === 0 && p.cosmetics.local.some((i: any) => i.definitionIndex === 378) })
    await page.keyboard.press("Comma"); await page.keyboard.press("Digit1")
    await page.waitForFunction(() => !(globalThis as any).__playsrcProfile.cosmetics.local.some((i: any) => i.definitionIndex === 378))
    await page.keyboard.press("Comma"); await page.keyboard.press("Digit2")
    await page.waitForFunction(() => (globalThis as any).__playsrcProfile.cosmetics.local.some((i: any) => i.definitionIndex === 378))
    await shot("blue-respawn-and-class-restoration")
    await cdp.detach()
    expect(errors).toEqual([])
  } finally {
    records.errors = errors
    records.faviconMisses = faviconMisses
    records.terminal = await page.evaluate(() => ({ phase: document.querySelector<HTMLElement>("main")?.dataset.phase,
      detail: document.querySelector<HTMLElement>("main")?.dataset.detail, cosmetics: (globalThis as any).__playsrcProfile.cosmetics,
      bots: (globalThis as any).__playsrcProfile.bots, console: document.querySelector<HTMLElement>("[aria-label='Console output']")?.innerText })).catch(() => null)
    await writeFile(path.join(directory, "report.json"), JSON.stringify(records, (_key, value) => typeof value === "bigint" ? value.toString() : value, 2))
    await page.screenshot({ path: path.join(directory, "terminal.png") }).catch(() => {})
    desktop("terminal")
    console.log("[burning-flames]", directory)
  }
})
