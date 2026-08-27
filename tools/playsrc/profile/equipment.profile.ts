import { mkdir, readFile, writeFile } from "node:fs/promises"
import { execFileSync } from "node:child_process"
import path from "node:path"
import { test, expect } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { decodeScreenshot } from "./screenshot-pixels"
import { summarizeFrameTimes } from "./profile-window"
import { nativeEquipment } from "../../../games/tf2/browser/tests/fixtures/equipment"

test.use({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 })

test("authored backpack native equip and browser restart persistence", async ({ page }) => {
  test.skip(process.env.PLAYSRC_HITSCAN_MATRIX === "1")
  test.setTimeout(100_000)
  const local = await loadLocalConfig(), directory = path.join(local.sourceCacheDir, "profiles/equipment")
  await mkdir(directory, { recursive: true })
  const errors: string[] = []
  page.on("pageerror", error => errors.push(error.message))
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = { captureEquipment: false, equipmentFrames: [] } })
  const sample = async () => page.evaluate(async () => {
    const profile = (globalThis as any).__playsrcProfile
    profile.equipmentFrames = []; profile.captureEquipment = true
    const started = performance.now(), frames: number[] = []
    let previous = started
    await new Promise<void>(resolve => requestAnimationFrame(function frame(now) {
      frames.push(now - previous); previous = now
      now - started >= 5000 ? resolve() : requestAnimationFrame(frame)
    }))
    profile.captureEquipment = false
    return { seconds: (performance.now() - started) / 1000, frames, equipmentFrames: profile.equipmentFrames as number[] }
  })
  await page.goto("/", { waitUntil: "domcontentloaded", timeout: 15_000 })
  await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu", { timeout: 20_000 })
  await page.locator("[data-vgui-name='CharacterSetupButton']").click()
  const equipment = page.locator(".equipment-layer")
  await expect(equipment.locator("[data-vgui-name='Class3']")).toBeVisible({ timeout: 20_000 })
  await equipment.locator("[data-vgui-name='Class3']").click()
  await expect(equipment).toHaveAttribute("data-preview-model", "models/player/soldier.mdl", { timeout: 15_000 })
  const stock = process.env.PLAYSRC_EQUIPMENT_UI_ONLY ? undefined : await sample()
  await equipment.locator("[data-vgui-name='BackpackButton']").click()
  await expect(equipment.locator("[data-vgui-name='Itemitem-378']")).toBeVisible()
  const capture = await page.screenshot({ path: path.join(directory, "backpack.png") })
  const pixels = decodeScreenshot(capture)
  const hat = await equipment.locator("[data-vgui-name='Itemitem-378']").boundingBox()
  expect(hat).not.toBeNull()
  let purple = 0
  for (let y = Math.floor(hat!.y); y < hat!.y + hat!.height; y++) for (let x = Math.floor(hat!.x); x < hat!.x + hat!.width; x++) {
    const at = (y * pixels.width + x) * pixels.channels
    const r = pixels.pixels[at]!, g = pixels.pixels[at + 1]!, b = pixels.pixels[at + 2]!
    if (r > 60 && b > 60 && r > g * 1.3 && b > g * 1.3) purple++
  }
  expect(purple, "Unusual quality must produce actual purple border pixels").toBeGreaterThan(20)
  const desktop = (name: string) => {
    if (process.platform === "darwin") execFileSync("screencapture", ["-x", path.join(directory, name)], { timeout: 5000 })
  }
  desktop("backpack-desktop.png")
  await equipment.locator("[data-vgui-name='Itemitem-378']").hover()
  await expect(equipment.locator("[data-vgui-name='ItemTooltipName']")).toHaveText("Unusual Team Captain")
  await expect(equipment.locator("[data-vgui-name='ItemTooltip']")).toContainText("Burning Flames")
  await expect(equipment.locator("[data-vgui-name='ItemTooltip']")).toBeVisible()
  // Browser capture preparation can end native hover. Capture visible pixels first.
  desktop("tooltip-desktop.png")
  await expect(equipment.locator("[data-vgui-name='ItemTooltip']")).toBeVisible()
  let tooltipPixels: { purpleAdded: number; lightTextAdded: number } | undefined
  if (process.platform === "darwin") {
    const before = decodeScreenshot(await readFile(path.join(directory, "backpack-desktop.png")))
    const after = decodeScreenshot(await readFile(path.join(directory, "tooltip-desktop.png")))
    expect([after.width, after.height, after.channels]).toEqual([before.width, before.height, before.channels])
    let purpleAdded = 0, lightTextAdded = 0
    for (let at = 0; at < after.pixels.length; at += after.channels) {
      const r = after.pixels[at]!, g = after.pixels[at + 1]!, b = after.pixels[at + 2]!
      if (Math.abs(r - before.pixels[at]!) + Math.abs(g - before.pixels[at + 1]!) + Math.abs(b - before.pixels[at + 2]!) < 40) continue
      if (r > 60 && b > 60 && r > g * 1.3 && b > g * 1.3) purpleAdded++
      if (r > 175 && g > 175 && b > 140) lightTextAdded++
    }
    tooltipPixels = { purpleAdded, lightTextAdded }
    expect(purpleAdded).toBeGreaterThan(1000)
    expect(lightTextAdded).toBeGreaterThan(50)
  }
  await writeFile(path.join(directory, "tooltip-dom.json"), JSON.stringify(await equipment.evaluate(root => [...root.querySelectorAll<HTMLElement>("[data-vgui-name^='ItemTooltip'],[data-vgui-name^='ItemDescription']")].map(node => ({ name: node.dataset.vguiName, text: node.textContent, style: node.getAttribute("style"), bounds: node.getBoundingClientRect().toJSON() }))), null, 2))
  await equipment.locator("[data-vgui-name='Itemitem-378']").click()
  await equipment.locator("[data-vgui-name='Itemitem-378']").click()
  await expect(equipment.locator("[data-vgui-name='EquipmentPlayer']")).toBeVisible()
  await writeFile(path.join(directory, "equipped-dom.json"), JSON.stringify(await equipment.evaluate(root => [...root.querySelectorAll<HTMLElement>("[data-vgui-name='ItemName'],[data-vgui-name='EquipmentPlayer']")].map(node => ({ name: node.dataset.vguiName, text: node.textContent, style: node.getAttribute("style"), bounds: node.getBoundingClientRect().toJSON() }))), null, 2))
  await expect(equipment).toHaveAttribute("data-preview-model", "models/player/soldier.mdl", { timeout: 15_000 })
  const saved = await page.evaluate(() => localStorage.getItem("playsrc.tf2.local-equipment.v1"))
  expect(saved?.length).toBe(924)
  const equippedCapture = decodeScreenshot(await page.screenshot({ path: path.join(directory, "equipped.png") }))
  const modelBounds = (await equipment.locator("[data-vgui-name='EquipmentPlayer']").boundingBox())!
  let modelPixels = 0
  for (let y = Math.ceil(modelBounds.y); y < modelBounds.y + modelBounds.height; y++) for (let x = Math.ceil(modelBounds.x); x < modelBounds.x + modelBounds.width; x++) {
    const at = (y * equippedCapture.width + x) * equippedCapture.channels
    const r = equippedCapture.pixels[at]!, g = equippedCapture.pixels[at + 1]!, b = equippedCapture.pixels[at + 2]!
    if (r > 60 && r > g * 1.4 && r > b * 1.4) modelPixels++
  }
  expect(modelPixels, "the standalone loadout must show actual RED Soldier pixels").toBeGreaterThan(1000)
  const unusual = process.env.PLAYSRC_EQUIPMENT_UI_ONLY ? undefined : await sample()
  await page.reload({ waitUntil: "domcontentloaded" })
  await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu", { timeout: 20_000 })
  await page.locator("[data-vgui-name='CharacterSetupButton']").click()
  await equipment.locator("[data-vgui-name='Class3']").click()
  expect(await page.evaluate(() => localStorage.getItem("playsrc.tf2.local-equipment.v1"))).toBe(saved)
  await expect(equipment.locator("[data-vgui-name='Itemslot-7'] [data-vgui-name='ItemIcon']")).toBeVisible()
  // A real map command must close the equipment surface before team admission.
  await page.keyboard.press("Backquote")
  const consoleEntry = page.locator("[aria-label='Console command']")
  await consoleEntry.fill("map pl_upward")
  await consoleEntry.press("Enter")
  await expect(equipment).toBeHidden()
  await expect(page.locator("main")).toHaveAttribute("data-team-selection-visible", "true", { timeout: 45_000 })
  if (await page.locator("main").getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  await page.locator(".team-selection-layer [data-vgui-name='teambutton1']").click()
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "true")
  await page.locator(".class-selection-layer [data-vgui-name='heavyweapons']").click()
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "false")
  await expect(page.locator("main")).toHaveAttribute("data-phase", "Ready")
  await page.evaluate(() => { (globalThis as any).__playsrcProfile.captureWeaponPoses = true })
  await page.locator("canvas.world-canvas").click({ position: { x: 640, y: 400 } })
  await expect(page.locator("main")).toHaveAttribute("data-pointer-locked", "true")
  await page.mouse.down({ button: "right" })
  await expect(page.locator("main")).toHaveAttribute("data-viewmodel-activities", /ACT_PRIMARY_ATTACK_STAND_PREFIRE/)
  await expect(page.locator("main")).toHaveAttribute("data-viewmodel-activity", "ACT_PRIMARY_VM_SECONDARYATTACK")
  const firstBarrel = await page.evaluate(() => (globalThis as any).__playsrcProfile.weaponPose as { model: string; tick: string; bones: number[] })
  await page.screenshot({ path: path.join(directory, "minigun-spinning-before.png") })
  await expect.poll(async () => page.evaluate(() => Number((globalThis as any).__playsrcProfile.weaponPose.tick))).toBeGreaterThan(Number(firstBarrel.tick) + 5)
  const secondBarrel = await page.evaluate(() => (globalThis as any).__playsrcProfile.weaponPose as typeof firstBarrel)
  await page.screenshot({ path: path.join(directory, "minigun-spinning-after.png") })
  const relativeBarrel = (bones: number[]) => Array.from({ length: 9 }, (_, index) => {
    const row = Math.floor(index / 3), column = index % 3
    return [0, 1, 2].reduce((sum, axis) => sum + bones[axis * 4 + row]! * bones[12 + axis * 4 + column]!, 0)
  })
  expect(firstBarrel.model).toBe("models/weapons/c_models/c_minigun/c_minigun.mdl")
  const firstRotation = relativeBarrel(firstBarrel.bones), secondRotation = relativeBarrel(secondBarrel.bones)
  expect(firstRotation.some((value, index) => Math.abs(value - secondRotation[index]!) > 0.001)).toBe(true)
  await page.mouse.down({ button: "left" })
  await expect(page.locator("main")).toHaveAttribute("data-viewmodel-activity", "ACT_PRIMARY_VM_PRIMARYATTACK")
  await page.mouse.up({ button: "left" }); await page.mouse.up({ button: "right" })
  await expect(page.locator("main")).toHaveAttribute("data-viewmodel-activity", "ACT_PRIMARY_ATTACK_STAND_POSTFIRE")
  await page.evaluate(() => { (globalThis as any).__playsrcProfile.captureWeaponPoses = false })
  await writeFile(path.join(directory, "minigun-bones.json"), JSON.stringify({ firstBarrel, secondBarrel, firstRotation, secondRotation }))
  expect(errors).toEqual([])
  const report = { platform: process.platform, tooltipPixels, purplePixels: purple, modelPixels, storageBytes: 692, mapAdmission: true, errors,
    stock: stock && { seconds: stock.seconds, browser: summarizeFrameTimes(stock.frames), equipment: summarizeFrameTimes(stock.equipmentFrames) },
    unusual: unusual && { seconds: unusual.seconds, browser: summarizeFrameTimes(unusual.frames), equipment: summarizeFrameTimes(unusual.equipmentFrames) } }
  if (stock) expect(stock.equipmentFrames.length).toBeGreaterThan(30)
  if (unusual) expect(unusual.equipmentFrames.length).toBeGreaterThan(30)
  await writeFile(path.join(directory, process.env.PLAYSRC_EQUIPMENT_UI_ONLY ? "ui-summary.json" : "native-summary.json"), JSON.stringify(report, null, 2))
})

test("twelve hitscan items admit their models, native firing and authored audio", async ({ page }) => {
  test.skip(process.env.PLAYSRC_HITSCAN_MATRIX !== "1")
  test.setTimeout(140_000)
  const subset = process.env.PLAYSRC_HITSCAN_ITEMS?.split(",").map(Number)
  const directory = path.join((await loadLocalConfig()).sourceCacheDir, subset ? "profiles/equipment/hitscan-targeted" : "profiles/equipment/hitscan")
  await mkdir(directory, { recursive: true })
  const errors: string[] = [], records: unknown[] = []
  page.on("pageerror", error => errors.push(error.message))
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = { captureWeaponPoses: true } })
  const main = page.locator("main"), equipment = page.locator(".equipment-layer")
  const command = async (text: string) => {
    if (await main.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    await page.locator("[aria-label='Console command']").fill(text)
    await page.keyboard.press("Enter")
    if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  }
  const actual = () => page.evaluate(() => (globalThis as any).__playsrcProfile.weaponPose as { model: string; definition: number; class: number; ammo: { clip: number; reserve: number } } | null)
  await page.goto("/", { waitUntil: "domcontentloaded" })
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 20_000 })
  await command("map pl_upward")
  await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 45_000 })
  await page.locator(".team-selection-layer [data-vgui-name='teambutton1']").click()
  await page.locator(".class-selection-layer [data-vgui-name='soldier']").click()
  await expect(main).toHaveAttribute("data-phase", "Ready")
  const cases = [
    [45, 1, "scout", 0], [1103, 1, "scout", 0], [425, 6, "heavyweapons", 1], [1153, 9, "engineer", 0],
    [415, 3, "soldier", 1], [424, 6, "heavyweapons", 0], [312, 6, "heavyweapons", 0], [41, 6, "heavyweapons", 0],
    [61, 8, "spy", 1], [460, 8, "spy", 1], [220, 1, "scout", 0], [402, 2, "sniper", 0],
  ] as const
  const selectedCases = cases.filter(([definition]) => !subset || subset.includes(definition))
  expect(selectedCases.length).toBeGreaterThan(0)
  const requested = selectedCases.map(([definition]) => definition)
  for (const [definition, playerClass, className, slot] of selectedCases) {
    await command(`joinclass ${className}`)
    await expect.poll(async () => (await actual())?.class).toBe(playerClass)
    await page.keyboard.press("Comma")
    await page.locator(".class-selection-layer [data-vgui-name='EditLoadoutButton']").click()
    await equipment.locator(`[data-vgui-name='Itemslot-${slot}']`).click()
    await equipment.locator(`[data-vgui-name='Itemitem-${definition}']`).click()
    await expect(equipment.locator("[data-vgui-name='EquipmentPlayer']")).toBeVisible({ timeout: 20_000 })
    await equipment.locator("[data-vgui-name='BackButton']").click()
    await equipment.locator("[data-vgui-name='BackButton']").click()
    await expect(equipment).toBeHidden()
    // Switching class is a real respawn path even when the current map's room
    // does not admit an immediate loadout refresh.
    if ((await actual())?.definition !== definition) {
      const other = playerClass === 3 ? "scout" : "soldier"
      await command(`joinclass ${other}`)
      await expect.poll(async () => (await actual())?.class).toBe(playerClass === 3 ? 1 : 3)
      await command(`joinclass ${className}`)
      await expect.poll(async () => (await actual())?.class).toBe(playerClass)
    }
    const metadata = nativeEquipment.inventory.find(item => item.item.definitionIndex === definition)!
    const selection = metadata.classSlots.find(value => value.class === playerClass)!.selectionSlot
    await page.keyboard.press(`Digit${selection + 1}`)
    await expect.poll(async () => (await actual())?.definition).toBe(definition)
    await expect.poll(async () => (await actual())?.model).toBe(metadata.modelPlayer)
    const before = (await actual())!
    await page.locator("canvas.world-canvas").click({ position: { x: 640, y: 400 } })
    await page.mouse.down({ button: "left" })
    await expect.poll(async () => {
      const pose = await actual()
      return pose ? pose.ammo.clip + pose.ammo.reserve : Infinity
    }).toBeLessThan(before.ammo.clip + before.ammo.reserve)
    await page.mouse.up({ button: "left" })
    const after = (await actual())!
    expect(after.ammo.clip + after.ammo.reserve).toBeLessThan(before.ammo.clip + before.ammo.reserve)
    const sounds = metadata.soundOverrides.filter(([slot]) => slot === "sound_single_shot" || slot === "sound_double_shot" || slot === "sound_burst").map(([, name]) => name)
    await expect.poll(async () => {
      const played = await main.getAttribute("data-audio-starts") ?? ""
      return sounds.some(sound => played.includes(sound))
    }).toBe(true)
    const audio = await main.getAttribute("data-audio-starts") ?? ""
    expect(sounds.some(sound => audio.includes(sound)), `${definition}: ${audio}`).toBe(true)
    await page.screenshot({ path: path.join(directory, `${definition}.png`) })
    records.push({ definition, model: after.model, before: { clip: before.ammo.clip, reserve: before.ammo.reserve },
      after: { clip: after.ammo.clip, reserve: after.ammo.reserve }, sounds: sounds.filter(sound => audio.includes(sound)) })
    await writeFile(path.join(directory, "matrix.json"), JSON.stringify({ platform: process.platform, requested, complete: false, records, errors }, null, 2))
    await command("-attack")
  }
  expect(errors).toEqual([])
  await writeFile(path.join(directory, "matrix.json"), JSON.stringify({ platform: process.platform, requested, complete: true, records, errors }, null, 2))
})
