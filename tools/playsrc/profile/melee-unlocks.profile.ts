import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import type { Page } from "@playwright/test"
import { test, expect } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { decodeScreenshot } from "./screenshot-pixels"
import { summarizeFrameTimes } from "./profile-window"

test.use({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 })

async function command(page: Page, text: string) {
  const main = page.locator("main")
  if (await main.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  await expect(entry).toBeVisible()
  await entry.fill(text); await entry.press("Enter")
  if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
}

async function equip(page: Page, definition: number) {
  await command(page, "-attack")
  if (await page.locator("main").getAttribute("data-gameui") === "in-game") await page.keyboard.press("Escape")
  await page.locator("[data-vgui-name='CharacterSetupButton']").click()
  const layer = page.locator(".equipment-layer")
  await expect(layer).toBeVisible()
  await layer.locator("[data-vgui-name='BackpackButton']").click()
  const item = layer.locator(`[data-vgui-name='Itemitem-${definition}']`)
  for (let pageNumber = 0; pageNumber < 4 && !await item.isVisible(); pageNumber++) await layer.locator("[data-vgui-name='NextPage']").click()
  await expect(item).toBeVisible()
  expect(await item.count()).toBe(1)
  await item.click(); await item.click()
  await expect(layer.locator("[data-vgui-name='EquipmentPlayer']")).toBeVisible()
  return layer
}

type Combat = { tick: string; class: number; weapon: number; health: number; conditions: number[]; overlay: null | { identity: string; tint: number[] }; equipment: { definitionIndex: number }[]; bots: { identity: number; health: number; position: number[]; conditions: number[] }[] }
async function combat(page: Page): Promise<Combat> {
  return page.evaluate(() => (globalThis as typeof globalThis & { __playsrcProfile: { melee: Combat } }).__playsrcProfile.melee)
}

async function measure(page: Page) {
  return page.evaluate(() => new Promise<{ milliseconds: number; ticks: number; frames: number[]; samples: unknown[] }>(resolve => {
    const main = document.querySelector<HTMLElement>("main")!, start = performance.now(), firstTick = Number(main.dataset.snapshotTick)
    const frames: number[] = [], samples: unknown[] = []; let previous = start
    const frame = (now: number) => {
      frames.push(now - previous); previous = now
      if (main.dataset.performanceDetail) samples.push(JSON.parse(main.dataset.performanceDetail))
      if (now - start >= 5000) resolve({ milliseconds: now - start, ticks: Number(main.dataset.snapshotTick) - firstTick, frames, samples })
      else requestAnimationFrame(frame)
    }
    requestAnimationFrame(frame)
  }))
}

async function captureMouse(page: Page) {
  await page.bringToFront()
  await page.locator("canvas.world-canvas").click({ position: { x: 640, y: 360 } })
  await expect(page.locator("main")).toHaveAttribute("data-pointer-locked", "true")
  await page.evaluate(() => {
    const profile = (globalThis as typeof globalThis & { __playsrcProfile: { displacementCamera: { yawDegrees: number; pitchDegrees: number } } }).__playsrcProfile
    const event = new MouseEvent("mousemove", { bubbles: true })
    Object.defineProperty(event, "movementX", { value: profile.displacementCamera.yawDegrees / 0.066 })
    Object.defineProperty(event, "movementY", { value: -profile.displacementCamera.pitchDegrees / 0.066 })
    dispatchEvent(event)
  })
}

async function fire(page: Page, held: boolean) {
  await page.evaluate(held => dispatchEvent(new MouseEvent(held ? "mousedown" : "mouseup", { button: 0, bubbles: true })), held)
}

const groups = [
  [[155, "engineer", 9, 42], [171, "sniper", 2, 14], [214, "pyro", 7, 16], [232, "sniper", 2, 14], [325, "scout", 1, 6]],
  [[326, "pyro", 7, 16], [355, "scout", 1, 6], [401, "sniper", 2, 14], [416, "soldier", 3, 8], [310, "heavyweapons", 6, 11]],
] as const

for (const [group, items] of groups.entries()) test(`melee unlock group ${group + 1} actual backpack combat and lifecycle`, async ({ page }) => {
  test.setTimeout(150_000)
  const local = await loadLocalConfig(), directory = path.join(local.sourceCacheDir, "profiles/melee-unlocks", `group-${group + 1}`)
  await mkdir(directory, { recursive: true })
  const errors: string[] = [], evidence: unknown[] = []
  page.on("pageerror", error => { errors.push(error.message); console.error(error.message) })
  page.on("console", message => { if (message.type() === "error") console.error(message.text()) })
  await page.goto("/", { waitUntil: "domcontentloaded", timeout: 15_000 })
  await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu", { timeout: 20_000 })
  await page.evaluate(() => { const root = globalThis as typeof globalThis & { __playsrcProfile?: Record<string, unknown> }; root.__playsrcProfile ??= {}; root.__playsrcProfile.captureMelee = true })
  await command(page, "map pl_upward")
  await expect(page.locator("main")).toHaveAttribute("data-team-selection-visible", "true", { timeout: 45_000 })
  await page.locator(".team-selection-layer [data-vgui-name='teambutton1']").click()
  await expect(page.locator("main")).toHaveAttribute("data-phase", "Ready", { timeout: 30_000 })
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "true")
  await page.keyboard.press("Digit2")
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "false")
  await page.evaluate(() => { (globalThis as typeof globalThis & { __playsrcProfile: Record<string, unknown> }).__playsrcProfile.captureMelee = true })
  await expect.poll(async () => (await combat(page))?.tick).toBeTruthy()
  await command(page, "tf_bot_quota 0")
  await command(page, "nb_stop 1")
  await expect.poll(async () => (await combat(page)).bots.length).toBe(0)
  await command(page, "tf_bot_add 1 heavy blue easy")
  await expect.poll(async () => (await combat(page)).bots.length).toBe(1)
  const name = await page.evaluate(() => (globalThis as typeof globalThis & { __playsrcProfile: { combat: { scores: { identity: number; name: string }[] } } }).__playsrcProfile.combat.scores.find(player => player.identity !== 1)!.name)

  const selected = items.filter(([definition]) => !process.env.PROFILE_MELEE_ITEM || String(definition) === process.env.PROFILE_MELEE_ITEM)
  const labels = new Map<number, string>()
  for (const [definition, className, classId, weapon] of selected) {
    const layer = await equip(page, definition)
    labels.set(definition, (await layer.locator("[data-vgui-name='Itemslot-2']").textContent())!)
    const before = await page.screenshot({ path: path.join(directory, `${definition}-backpack.png`) })
    const decoded = decodeScreenshot(before)
    expect(decoded.pixels.some(value => value > 30)).toBe(true)
    const storage = await page.evaluate(() => localStorage.getItem("playsrc.tf2.local-equipment.v1"))
    expect(storage).toBeTruthy()
    for (let back = 0; back < 3 && await layer.isVisible(); back++) await layer.locator("[data-vgui-name='BackButton']").click()
    await expect(layer).toBeHidden()
    if (await page.locator("main").getAttribute("data-gameui") !== "in-game") await page.keyboard.press("Escape")
    await command(page, `joinclass ${className}`)
    await expect.poll(async () => (await combat(page)).class).toBe(classId)
    await command(page, "setpos -2528 -1360 17")
    await command(page, `bot_teleport "${name}" -2480 -1360 17 0 180 0`)
    await page.keyboard.press("Digit3")
    await expect.poll(async () => {
      const state = await combat(page)
      if (state.class === classId && state.weapon !== weapon) await page.keyboard.press("Digit3")
      return state.weapon
    }).toBe(weapon)
    await expect.poll(async () => (await combat(page)).equipment.some(item => item.definitionIndex === definition)).toBe(true)
    await captureMouse(page)
    const ready = await combat(page)
    await fire(page, true)
    await expect.poll(async () => (await combat(page)).bots[0]!.health).toBeLessThan(ready.bots[0]!.health)
    await fire(page, false)
    const hit = await combat(page)
    expect(hit.weapon).toBe(weapon)
    await page.screenshot({ path: path.join(directory, `${definition}-combat.png`) })
    await expect.poll(async () => BigInt((await combat(page)).tick)).toBeGreaterThan(BigInt(hit.tick) + 60n)
    await page.screenshot({ path: path.join(directory, `${definition}-held.png`) })
    const presentation = await page.locator("main").evaluate(node => ({ ...((node as HTMLElement).dataset) }))
    if (definition === 355) {
      expect((await combat(page)).bots[0]!.conditions[0]! & (1 << 30)).not.toBe(0)
      await command(page, "setpos -2600 -1360 17")
      await captureMouse(page)
      await page.screenshot({ path: path.join(directory, "fan-mark-visible.png") })
      await command(page, `bot_teleport "${name}" -2000 -1360 17 0 180 0`)
      await captureMouse(page)
      expect((await combat(page)).bots[0]!.conditions[0]! & (1 << 30)).not.toBe(0)
      await page.screenshot({ path: path.join(directory, "fan-mark-behind-wall.png") })
      evidence.push({ mechanic: "marked-target-world-depth", marked: await combat(page) })
    }
    if (definition === 325) {
      await command(page, `bot_teleport "${name}" -2000 -1360 17 0 180 0`)
      await captureMouse(page)
      await page.keyboard.press("Digit1")
      await expect.poll(async () => (await combat(page)).weapon).toBe(4)
      const baseline = process.env.PROFILE_MELEE_PERF === "1" ? await measure(page) : null
      await page.screenshot({ path: path.join(directory, "basher-before-miss.png") })
      await page.keyboard.press("Digit3")
      await expect.poll(async () => (await combat(page)).weapon).toBe(6)
      const beforeMiss = await combat(page)
      await fire(page, true)
      await expect.poll(async () => (await combat(page)).health, { intervals: [15, 30, 60] }).toBeLessThan(beforeMiss.health)
      await fire(page, false)
      const bleeding = await combat(page)
      expect(bleeding.conditions[0]! & (1 << 25)).not.toBe(0)
      expect(bleeding.overlay?.identity).toBe("materials/effects/bleed_overlay.vmt")
      await expect.poll(async () => (await combat(page)).overlay?.tint[1] ?? 1, { intervals: [15, 30, 60] }).toBeLessThan(0.82)
      await page.screenshot({ path: path.join(directory, "basher-bleeding.png") })
      await page.keyboard.press("Digit1")
      await expect.poll(async () => (await combat(page)).weapon).toBe(4)
      expect((await combat(page)).conditions[0]! & (1 << 25)).not.toBe(0)
      const active = process.env.PROFILE_MELEE_PERF === "1" ? await measure(page) : null
      for (const sample of [baseline, active]) if (sample) expect(sample.ticks).toBeGreaterThan(300)
      evidence.push({ mechanic: "basher-clean-miss-and-holstered-bleed", beforeMiss, bleeding, baseline: baseline && { ...baseline, cadence: summarizeFrameTimes(baseline.frames) }, active: active && { ...active, cadence: summarizeFrameTimes(active.frames) } })
      await command(page, "joinclass soldier")
      await expect.poll(async () => (await combat(page)).class).toBe(3)
      await command(page, "joinclass scout")
      await expect.poll(async () => (await combat(page)).class).toBe(1)
      await expect.poll(async () => (await combat(page)).health).toBe(125)
      await expect.poll(async () => (await combat(page)).conditions[0]! & (1 << 25)).toBe(0)
      await page.screenshot({ path: path.join(directory, "basher-respawn-cleared.png") })
    }
    await page.keyboard.press("Digit1")
    await expect.poll(async () => (await combat(page)).weapon).not.toBe(weapon)
    evidence.push({ definition, classId, ready, hit, storage, presentation })
    await writeFile(path.join(directory, "combat.json"), JSON.stringify({ platform: process.platform, windowsCertification: false, evidence, errors }, null, 2))
    await command(page, `bot_whack "${name}"`)
    await command(page, "nb_stop 0")
    await expect.poll(async () => (await combat(page)).bots[0]!.health, { timeout: 15_000 }).toBe(300)
    await command(page, "nb_stop 1")
    expect((await combat(page)).bots[0]!.conditions[0]! & ((1 << 25) | (1 << 30))).toBe(0)
  }
  const saved = await page.evaluate(() => localStorage.getItem("playsrc.tf2.local-equipment.v1"))
  await page.reload({ waitUntil: "domcontentloaded" })
  await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu", { timeout: 20_000 })
  await page.locator("[data-vgui-name='CharacterSetupButton']").click()
  const restored = page.locator(".equipment-layer")
  await expect(restored).toBeVisible()
  const last = selected.at(-1)!
  await restored.locator(`[data-vgui-name='Class${last[2]}']`).click()
  await expect(restored.locator("[data-vgui-name='EquipmentPlayer']")).toBeVisible()
  await expect(restored.locator("[data-vgui-name='Itemslot-2']")).toHaveText(labels.get(last[0])!)
  expect(await page.evaluate(() => localStorage.getItem("playsrc.tf2.local-equipment.v1"))).toBe(saved)
  await page.screenshot({ path: path.join(directory, "application-restart-equipment.png") })
  evidence.push({ lifecycle: "application-restart", definition: last[0], label: labels.get(last[0]), saved })
  expect(errors).toEqual([])
  await writeFile(path.join(directory, "combat.json"), JSON.stringify({ platform: process.platform, windowsCertification: false, evidence, errors }, null, 2))
})
