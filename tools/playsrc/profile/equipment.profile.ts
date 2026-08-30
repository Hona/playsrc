import { mkdir, readFile, writeFile } from "node:fs/promises"
import { execFileSync } from "node:child_process"
import path from "node:path"
import { test, expect } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { decodeScreenshot } from "./screenshot-pixels"
import { nativeEquipment } from "../../../games/tf2/browser/tests/fixtures/equipment"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"
import { summarizeFrameTimes } from "./profile-window"

test.use({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 })

test("authored backpack native equip and browser restart persistence", async ({ page }) => {
  test.skip(process.env.PLAYSRC_HITSCAN_MATRIX === "1" || process.env.PLAYSRC_EQUIPMENT_LIFECYCLE === "1" || process.env.PLAYSRC_PROJECTILE_MATRIX === "1")
  test.setTimeout(100_000)
  const local = await loadLocalConfig(), directory = path.join(local.sourceCacheDir, "profiles/equipment")
  await mkdir(directory, { recursive: true })
  const errors: string[] = []
  page.on("pageerror", error => { errors.push(error.message); console.error(error.message) })
  page.on("console", message => { if (message.type() === "error") console.error(message.text()) })
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
  const visibleItems = () => equipment.locator("[data-vgui-name^='Itemitem-']").evaluateAll(nodes => nodes.map(node => Number((node as HTMLElement).dataset.vguiName!.slice("Itemitem-".length))))
  const firstPage = await visibleItems()
  await equipment.locator("[data-vgui-name='NextPage']").click()
  const inventoryDefinitions = [...firstPage, ...await visibleItems()]
  expect(new Set(inventoryDefinitions).size).toBe(nativeEquipment.inventory.length)
  expect(inventoryDefinitions.sort((a, b) => a - b)).toEqual(nativeEquipment.inventory.map(entry => entry.item.definitionIndex).sort((a, b) => a - b))
  for (const unavailable of [19, 20, 735]) expect(inventoryDefinitions).not.toContain(unavailable)
  await equipment.locator("[data-vgui-name='PrevPage']").click()
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
  if (stock && unusual) await writeFile(path.join(directory, "equipment-samples.json"), JSON.stringify({
    stock: { seconds: stock.seconds, browser: summarizeFrameTimes(stock.frames), equipment: summarizeFrameTimes(stock.equipmentFrames) },
    unusual: { seconds: unusual.seconds, browser: summarizeFrameTimes(unusual.frames), equipment: summarizeFrameTimes(unusual.equipmentFrames) },
  }, null, 2))
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
  await expect.poll(() => page.evaluate(() => (globalThis as any).__playsrcProfile.weaponPose?.class)).toBe(6)
  await page.bringToFront()
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
  await writeFile(path.join(directory, "minigun-bones.json"), JSON.stringify({ firstBarrel, secondBarrel, firstRotation, secondRotation }, (_, value) => typeof value === "bigint" ? String(value) : value))
  expect(errors).toEqual([])
  const report = { platform: process.platform, inventoryDefinitions, tooltipPixels, purplePixels: purple, modelPixels, storageBytes: 692, mapAdmission: true, errors,
    stock: stock && { seconds: stock.seconds, browser: summarizeFrameTimes(stock.frames), equipment: summarizeFrameTimes(stock.equipmentFrames) },
    unusual: unusual && { seconds: unusual.seconds, browser: summarizeFrameTimes(unusual.frames), equipment: summarizeFrameTimes(unusual.equipmentFrames) } }
  if (stock) expect(stock.equipmentFrames.length).toBeGreaterThan(30)
  if (unusual) expect(unusual.equipmentFrames.length).toBeGreaterThan(30)
  await writeFile(path.join(directory, process.env.PLAYSRC_EQUIPMENT_UI_ONLY ? "ui-summary.json" : "native-summary.json"), JSON.stringify(report, null, 2))
})

test("twelve hitscan items admit their models, native firing and authored audio", async ({ page }) => {
  test.skip(process.env.PLAYSRC_HITSCAN_MATRIX !== "1" || process.env.PLAYSRC_EQUIPMENT_LIFECYCLE === "1")
  test.setTimeout(140_000)
  const subset = process.env.PLAYSRC_HITSCAN_ITEMS?.split(",").map(Number)
  const combat = process.env.PLAYSRC_HITSCAN_COMBAT === "1"
  const directory = path.join((await loadLocalConfig()).sourceCacheDir, subset ? `profiles/equipment/hitscan-${subset.join("-")}` : "profiles/equipment/hitscan")
  await mkdir(directory, { recursive: true })
  const errors: string[] = [], records: unknown[] = []
  page.on("pageerror", error => { errors.push(error.message); console.error(error.message) })
  page.on("console", message => { if (message.type() === "error") console.error(message.text()) })
  await page.addInitScript(combat => { (globalThis as any).__playsrcProfile = { captureWeaponPoses: true, captureMelee: combat, captureHitscan: combat } }, combat)
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
  if (combat) {
    await command("tf_bot_quota 0"); await command("nb_stop 1")
    // The Force-a-Nature suppresses FireBullet itself during the real pre-round
    // freeze. Let the authored waiting and pre-round clocks elapse unchanged.
    await expect.poll(() => page.evaluate(() => { const round = (globalThis as any).__playsrcProfile.round; return !round.waitingForPlayers && round.state === 4 }), { timeout: 40_000 }).toBe(true)
  }
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
    await expect(equipment).toBeHidden()
    await expect(main).toHaveAttribute("data-class-selection-visible", "true")
    await page.keyboard.press("Escape")
    await expect(main).toHaveAttribute("data-class-selection-visible", "false")
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
    let target: { identity: number; health: number; position: number[] } | undefined
    if (combat) {
      await command("tf_bot_kick all")
      await command("tf_bot_add 1 heavy blue easy")
      await expect.poll(() => page.evaluate(() => (globalThis as any).__playsrcProfile.bots.length)).toBe(1)
      const name = await page.evaluate(() => (globalThis as any).__playsrcProfile.combat.scores.find((player: any) => player.identity !== 1).name)
      await command("setpos -2528 -1360 17")
      await command(`bot_teleport "${name}" -2440 -1360 17 0 ${definition === 61 || definition === 402 ? 180 : 0} 0`)
      await expect.poll(() => page.evaluate(() => (globalThis as any).__playsrcProfile.bots[0].position)).toEqual([-2440, -1360, 17])
      target = await page.evaluate(() => { const bot = (globalThis as any).__playsrcProfile.bots[0]; return { identity: bot.identity, health: bot.health, position: bot.position } })
      await page.evaluate(() => { (globalThis as any).__playsrcProfile.meleeTimeline = [] })
    }
    await page.locator("canvas.world-canvas").click({ position: { x: 640, y: 400 } })
    if (combat) {
      const capturedTick = await page.evaluate(() => Number((globalThis as any).__playsrcProfile.melee.tick))
      await expect.poll(() => page.evaluate(() => Number((globalThis as any).__playsrcProfile.melee.tick))).toBeGreaterThan(capturedTick + 75)
      await page.evaluate(head => {
        const profile = (globalThis as any).__playsrcProfile, camera = profile.displacementCamera
        let yaw = 0, pitch = 0
        if (head) {
          const point = profile.hitscan.actors[0].attachments.head
          if (!point) throw new Error("Authored target head attachment unavailable")
          const delta = point.map((value: number, axis: number) => value - camera.position[axis])
          yaw = Math.atan2(delta[1], delta[0]) * 180 / Math.PI
          pitch = -Math.atan2(delta[2], Math.hypot(delta[0], delta[1])) * 180 / Math.PI
        }
        const input = new MouseEvent("mousemove", { bubbles: true })
        Object.defineProperty(input, "movementX", { value: (camera.yawDegrees - yaw) / 0.066 })
        Object.defineProperty(input, "movementY", { value: (pitch - camera.pitchDegrees) / 0.066 })
        dispatchEvent(input)
      }, definition === 61 || definition === 402)
      if (definition === 402) {
        await page.mouse.click(640, 400, { button: "right" })
        await expect.poll(() => page.evaluate(() => (globalThis as any).__playsrcProfile.hitscan.conditions[0] & 2)).toBe(2)
        const scopedTick = await page.evaluate(() => Number((globalThis as any).__playsrcProfile.melee.tick))
        await expect.poll(() => page.evaluate(() => Number((globalThis as any).__playsrcProfile.melee.tick)), { timeout: 7000 }).toBeGreaterThan(scopedTick + 300)
      }
      await page.evaluate(() => { (globalThis as any).__playsrcProfile.meleeTimeline = [] })
    }
    const before = await page.evaluate(() => { const p = (globalThis as any).__playsrcProfile; return { ammo: p.weaponPose?.ammo ?? p.hitscan.ammo } })
    await page.mouse.down({ button: "left" })
    if (combat) {
      try {
        await expect.poll(() => page.evaluate(() => (globalThis as any).__playsrcProfile.meleeTimeline.filter((event: any) => event.kind === 17 && event.auxiliary === 1).length), { timeout: 5000 }).toBeGreaterThan(0)
      } catch (error) {
        await page.mouse.up({ button: "left" })
        await page.screenshot({ path: path.join(directory, `${definition}-failed-combat.png`) })
        await writeFile(path.join(directory, `${definition}-failed-combat.json`), JSON.stringify(await page.evaluate(() => {
          const p = (globalThis as any).__playsrcProfile
          return { camera: p.displacementCamera, combat: p.melee, hitscan: p.hitscan, events: p.meleeTimeline, weapon: p.weaponPose }
        }), (_, value) => typeof value === "bigint" ? String(value) : value, 2))
        throw error
      }
    }
    await expect.poll(async () => {
      return page.evaluate(() => { const p = (globalThis as any).__playsrcProfile, ammo = p.weaponPose?.ammo ?? p.hitscan.ammo; return ammo.clip + ammo.reserve })
    }).toBeLessThan(before.ammo.clip + before.ammo.reserve)
    await page.mouse.up({ button: "left" })
    let damage: any[] = []
    if (combat) {
      damage = await page.evaluate(() => (globalThis as any).__playsrcProfile.meleeTimeline.filter((event: any) => event.kind === 17 && event.auxiliary === 1))
      if (definition === 1103) expect(damage.some(event => event.values[2] === 2)).toBe(true)
      if (definition === 61 || definition === 402) expect(damage.some(event => event.values[2] === 1 && event.values[3] === 1)).toBe(true)
      if (definition === 41) expect(await page.evaluate(() => (globalThis as any).__playsrcProfile.bots[0].conditions[0] & (1 << 15))).not.toBe(0)
      if (definition === 45) {
        expect(await page.evaluate(() => (globalThis as any).__playsrcProfile.bots[0].velocity[0])).toBeGreaterThan(0)
        await command("nb_stop 0")
        await expect.poll(() => page.evaluate(() => (globalThis as any).__playsrcProfile.bots[0].position[0])).toBeGreaterThan(target!.position[0])
        await command("nb_stop 1")
      }
      if (definition === 402) expect(await page.evaluate(() => (globalThis as any).__playsrcProfile.hitscan.heads)).toBe(1)
      await page.screenshot({ path: path.join(directory, `${definition}-impact.png`) })
      await writeFile(path.join(directory, `${definition}-damage.json`), JSON.stringify(await page.evaluate(() => {
        const p = (globalThis as any).__playsrcProfile
        return { camera: p.displacementCamera, combat: p.melee, hitscan: p.hitscan, events: p.meleeTimeline }
      }), (_, value) => typeof value === "bigint" ? String(value) : value, 2))
    }
    const after = await page.evaluate(() => { const p = (globalThis as any).__playsrcProfile; return { model: p.weaponPose?.model ?? null, ammo: p.weaponPose?.ammo ?? p.hitscan.ammo } })
    expect(after.ammo.clip + after.ammo.reserve).toBeLessThan(before.ammo.clip + before.ammo.reserve)
    const sounds = metadata.soundOverrides.filter(([slot]) => slot === "sound_single_shot" || slot === "sound_double_shot" || slot === "sound_burst").map(([, name]) => name)
    await expect.poll(async () => {
      const played = await main.getAttribute("data-audio-starts") ?? ""
      return sounds.some(sound => played.includes(sound))
    }).toBe(true)
    const audio = await main.getAttribute("data-audio-starts") ?? ""
    expect(sounds.some(sound => audio.includes(sound)), `${definition}: ${audio}`).toBe(true)
    await page.screenshot({ path: path.join(directory, `${definition}.png`) })
    if (combat && after.ammo.clip < before.ammo.clip && after.ammo.reserve > 0) {
      await page.keyboard.press("KeyR")
      await expect.poll(async () => (await actual())?.ammo.clip, { timeout: 7000 }).toBeGreaterThan(after.ammo.clip)
    }
    if (combat && definition === 220) {
      await page.mouse.down({ button: "right" })
      await expect.poll(() => page.evaluate(() => (globalThis as any).__playsrcProfile.meleeTimeline.some((event: any) => event.kind === 14 && event.detail === 60 && event.subject !== 0))).toBe(true)
      await page.mouse.up({ button: "right" })
      expect(await main.getAttribute("data-audio-starts")).toContain("Weapon_Hands.PushImpact")
      await page.screenshot({ path: path.join(directory, "220-shove.png") })
    }
    records.push({ definition, model: after.model, target, damage, before: { clip: before.ammo.clip, reserve: before.ammo.reserve },
      after: { clip: after.ammo.clip, reserve: after.ammo.reserve }, sounds: sounds.filter(sound => audio.includes(sound)) })
    await writeFile(path.join(directory, "matrix.json"), JSON.stringify({ platform: process.platform, requested, complete: false, records, errors }, null, 2))
    await command("-attack")
  }
  expect(errors).toEqual([])
  await writeFile(path.join(directory, "matrix.json"), JSON.stringify({ platform: process.platform, requested, complete: true, records, errors }, null, 2))
})

test("remember active and last weapon settings survive real death and browser persistence", async ({ page }) => {
  test.skip(process.env.PLAYSRC_EQUIPMENT_LIFECYCLE !== "1")
  test.setTimeout(140_000)
  const directory = path.join((await loadLocalConfig()).sourceCacheDir, "profiles/equipment/weapon-lifecycle")
  await mkdir(directory, { recursive: true })
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = { captureMelee: true } })
  const main = page.locator("main"), records: unknown[] = []
  const state = () => page.evaluate(() => (globalThis as any).__playsrcProfile.melee as { tick: string; weapon: number; health: number; lifecycle: number })
  const command = async (text: string) => {
    if (await main.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    await page.locator("[aria-label='Console command']").fill(text); await page.keyboard.press("Enter")
    if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  }
  await page.goto("/", { waitUntil: "domcontentloaded" })
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 20_000 })
  await command("map pl_upward")
  await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 45_000 })
  await page.locator(".team-selection-layer [data-vgui-name='teambutton1']").click()
  await page.locator(".class-selection-layer [data-vgui-name='scout']").click()
  await expect(main).toHaveAttribute("data-phase", "Ready")
  await command("tf_bot_quota 0")
  await expect.poll(() => page.evaluate(() => { const round = (globalThis as any).__playsrcProfile.round; return !round.waitingForPlayers && round.state === 4 }), { timeout: 40_000 }).toBe(true)
  for (const remember of [false, true]) {
    await command("nb_stop 1")
    await command("tf_bot_kick all")
    await command("tf_bot_add 1 heavy blue easy")
    await expect.poll(() => page.evaluate(() => (globalThis as any).__playsrcProfile.bots.length)).toBe(1)
    const botName = await page.evaluate(() => (globalThis as any).__playsrcProfile.combat.scores.find((player: any) => player.identity !== 1).name)
    await command(`tf_remember_activeweapon ${Number(remember)}`)
    await command(`tf_remember_lastswitched ${Number(remember)}`)
    await page.keyboard.press("Digit3")
    await expect.poll(async () => (await state()).weapon).toBe(6)
    await page.keyboard.press("Digit2")
    await expect.poll(async () => (await state()).weapon).toBe(5)
    await page.screenshot({ path: path.join(directory, `${remember}-before-death.png`) })
    await command("setpos -2528 -1360 17")
    await command(`bot_teleport "${botName}" -2480 -1360 17 0 180 0`)
    await command("nb_stop 0")
    await expect.poll(async () => (await state()).lifecycle, { timeout: 20_000, intervals: [50, 100] }).toBe(2)
    const dead = await state()
    await command("nb_stop 1")
    await expect.poll(async () => (await state()).lifecycle, { timeout: 30_000 }).toBe(1)
    expect((await state()).weapon).toBe(remember ? 5 : 4)
    await page.screenshot({ path: path.join(directory, `${remember}-respawn.png`) })
    await page.keyboard.press("KeyQ")
    await expect.poll(async () => (await state()).weapon).toBe(remember ? 6 : 5)
    records.push({ remember, dead, afterLastInv: await state() })
  }
  await page.reload({ waitUntil: "domcontentloaded" })
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 20_000 })
  await page.keyboard.press("Backquote")
  for (const name of ["tf_remember_activeweapon", "tf_remember_lastswitched"]) {
    await page.locator("[aria-label='Console command']").fill(name); await page.keyboard.press("Enter")
    await expect(page.locator("[aria-label='Console output']")).toContainText(`"${name}" = "1"`)
  }
  await page.screenshot({ path: path.join(directory, "restored-preferences.png") })
  await writeFile(path.join(directory, "lifecycle.json"), JSON.stringify({ records, persisted: true }, null, 2))
})

test("projectile unlocks equip through the authored loadout and fire their native models", async ({ page }) => {
  test.skip(process.env.PLAYSRC_PROJECTILE_MATRIX !== "1")
  test.setTimeout(140_000)
  const perf = process.env.PLAYSRC_PROJECTILE_PERF === "1"
  const directory = path.join((await loadLocalConfig()).sourceCacheDir, perf ? "profiles/equipment/projectile-performance" : "profiles/equipment/projectiles")
  await mkdir(directory, { recursive: true })
  const errors: string[] = [], records: unknown[] = []
  page.on("pageerror", error => { errors.push(error.message); console.error(error.message) })
  page.on("console", message => {
    if (message.type() !== "error") return
    const url = message.location().url
    // Chromium probes this optional browser icon even though the page declares none.
    if (url && new URL(url).pathname === "/favicon.ico") return
    const detail = `${message.text()} (${url})`
    errors.push(detail); console.error(detail)
  })
  if (perf) await page.addInitScript(installBrowserFrameProfiler)
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = { captureWeaponPoses: true, captureProjectileQueries: true, captureProjectileGameplay: true } })
  const main = page.locator("main"), equipment = page.locator(".equipment-layer")
  const command = async (text: string) => {
    if (await main.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    await page.locator("[aria-label='Console command']").fill(text); await page.keyboard.press("Enter")
    if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  }
  const actual = () => page.evaluate(() => (globalThis as any).__playsrcProfile.weaponPose as { model: string; definition: number; class: number; ammo: { clip: number; reserve: number } } | null)
  const aim = async (pitch: number) => {
    await page.evaluate(pitch => {
      const canvas = document.querySelector<HTMLElement>("canvas.world-canvas")!
      const yaw = Number(canvas.dataset.displayCameraYaw), currentPitch = Number(canvas.dataset.displayCameraPitch)
      const event = new MouseEvent("mousemove", { bubbles: true })
      Object.defineProperties(event, { movementX: { value: (((yaw + 180) % 360 + 360) % 360 - 180) / 0.066 }, movementY: { value: (pitch - currentPitch) / 0.066 } })
      dispatchEvent(event)
    }, pitch)
    await expect.poll(async () => Math.abs(Number(await page.locator("canvas.world-canvas").getAttribute("data-display-camera-pitch")) - pitch)).toBeLessThan(0.1)
  }
  await page.goto("/", { waitUntil: "domcontentloaded" })
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 20_000 })
  await command("map pl_upward")
  await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 45_000 })
  await page.locator(".team-selection-layer [data-vgui-name='teambutton1']").click()
  await page.locator(".class-selection-layer [data-vgui-name='soldier']").click()
  await expect(main).toHaveAttribute("data-phase", "Ready")
  await command("tf_remember_activeweapon 1")
  if (perf) {
    await command("tf_bot_quota_mode fill"); await command("tf_bot_quota 24")
    await expect.poll(async () => page.evaluate(() => (globalThis as any).__playsrcProfile.projectileState?.bots)).toBe(23)
  }
  const subset = process.env.PLAYSRC_PROJECTILE_ITEMS?.split(",").map(Number)
  const requested = (perf ? [18, 1104] : [127, 228, 237, 414, 513, 1104, 39, 351, 740, 595]).filter(value => !subset || subset.includes(value))
  expect(requested.length).toBeGreaterThan(0)
  for (const definition of requested) {
    const metadata = nativeEquipment.inventory.find(item => item.item.definitionIndex === definition)!
    const pyro = [39, 351, 740, 595].includes(definition), playerClass = pyro ? 7 : 3, className = pyro ? "pyro" : "soldier", slot = pyro ? 1 : 0
    await command(`joinclass ${className}`)
    await expect.poll(async () => (await actual())?.class).toBe(playerClass)
    await page.keyboard.press("Comma")
    await page.locator(".class-selection-layer [data-vgui-name='EditLoadoutButton']").click()
    await equipment.locator(`[data-vgui-name='Itemslot-${slot}']`).click()
    await equipment.locator(`[data-vgui-name='Itemitem-${definition}']`).click()
    await expect(equipment.locator("[data-vgui-name='EquipmentPlayer']")).toBeVisible()
    await page.screenshot({ path: path.join(directory, `${definition}-equipped.png`) })
    await equipment.locator("[data-vgui-name='BackButton']").click()
    await expect(equipment).toBeHidden()
    await expect(main).toHaveAttribute("data-class-selection-visible", "true")
    await page.keyboard.press("Escape")
    await expect(main).toHaveAttribute("data-class-selection-visible", "false")
    await command("joinclass scout"); await expect.poll(async () => (await actual())?.class).toBe(1)
    await command(`joinclass ${className}`); await expect.poll(async () => (await actual())?.class).toBe(playerClass)
    const selection = metadata.classSlots.find(value => value.class === playerClass)!.selectionSlot
    await page.keyboard.press(`Digit${selection + 1}`)
    await expect.poll(async () => (await actual())?.definition).toBe(definition)
    await expect.poll(async () => (await actual())?.model).toBe(metadata.modelPlayer)
    if (perf) {
      await page.evaluate(() => { const profile = (globalThis as any).__playsrcProfile; profile.captureProjectileHistory = true; profile.projectileHistory = []; profile.projectileHistoryKey = "" })
      // Configured Upward NAV area95: standing hull ground and900unit overhead
      // clearance across a192unit square are admitted against its exact BSP.
      await command("setpos 2025 800 257.03125")
      await page.locator("canvas.world-canvas").click({ position: { x: 640, y: 400 } })
      await expect.poll(async () => page.evaluate(() => (globalThis as any).__playsrcProfile.projectileState.grounded)).toBe(true)
      await aim(89)
      await expect.poll(async () => page.evaluate(() => {
        const pose = (globalThis as any).__playsrcProfile.weaponPose
        return pose && BigInt(pose.tick) >= pose.ammo.nextPrimaryTick
      })).toBe(true)
      await page.keyboard.down("Space"); await page.keyboard.down("Control")
    }
    const before = (await actual())!, fires = Number(await main.getAttribute("data-fire-events"))
    if (!perf) await page.locator("canvas.world-canvas").click({ position: { x: 640, y: 400 } })
    await page.mouse.down({ button: "left" })
    await expect.poll(async () => Number(await main.getAttribute("data-fire-events"))).toBeGreaterThan(fires)
    await page.mouse.up({ button: "left" })
    if (definition === 595) {
      expect((await actual())?.ammo).toMatchObject({ clip: before.ammo.clip, reserve: before.ammo.reserve })
    } else {
      await expect.poll(async () => { const pose = await actual(); return pose ? pose.ammo.clip + pose.ammo.reserve : Infinity }).toBeLessThan(before.ammo.clip + before.ammo.reserve)
    }
    let performanceSample: unknown
    if (perf) {
      await page.keyboard.up("Space"); await page.keyboard.up("Control")
      await expect.poll(async () => page.evaluate(() => ((globalThis as any).__playsrcProfile.projectileState.conditions[2] & (1 << 17)) !== 0)).toBe(true)
      await aim(-60)
      await page.mouse.down({ button: "left" })
      if (definition === 1104) {
        try { await expect.poll(async () => page.evaluate(() => (globalThis as any).__playsrcProfile.projectileState.ropeItems), { timeout: 8000 }).toBeGreaterThan(0) }
        catch (error) {
          await writeFile(path.join(directory, "airstrike-jump-timeline.json"), JSON.stringify(await page.evaluate(() => (globalThis as any).__playsrcProfile.projectileHistory), null, 2))
          throw error
        }
        await page.screenshot({ path: path.join(directory, "airborne-rope.png") })
      }
      const sampled = await page.evaluate(async () => {
        const root = globalThis as any, state = root.__playsrcFrameProfiler, profile = root.__playsrcProfile
        const startFrame = state.completedFrames.length, startWorker = state.worker.length
        const before = { ...state.counters }, uploads = { ...profile.modelParticleUploads }, tick = Number(profile.projectileState.tick)
        const heap = (performance as any).memory?.usedJSHeapSize ?? null
        const started = performance.now(), intervals: number[] = []
        let previous = started, nextInput = 1000
        state.active = true
        await new Promise<void>(resolve => requestAnimationFrame(function frame(now) {
          intervals.push(now - previous); previous = now
          if (now - started >= nextInput && nextInput < 5000) {
            const event = new MouseEvent("mousemove", { bubbles: true }); Object.defineProperty(event, "movementX", { value: 1 }); dispatchEvent(event); nextInput += 1000
          }
          now - started >= 5000 ? resolve() : requestAnimationFrame(frame)
        }))
        const ended = performance.now(); state.active = false
        const frames = state.completedFrames.slice(startFrame)
        return { seconds: (ended - started) / 1000, tickRate: (Number(profile.projectileState.tick) - tick) / ((ended - started) / 1000), heapBefore: heap,
          intervals, frames: frames.map((frame: any) => ({ at: frame.at, mouseRevision: frame.mouseRevision, ...frame.detail })), input: state.input.filter((input: any) => input.at >= started && input.at <= ended),
          worker: state.worker.slice(startWorker).map((record: any) => ({ kind: record.kind, sentBytes: record.bytes, receivedBytes: record.receivedBytes, sharedBytes: record.sharedBytes })),
          counters: Object.fromEntries(Object.entries(state.counters).map(([name, value]) => [name, Number(value) - Number(before[name] ?? 0)])),
          uploads: Object.fromEntries(Object.entries(profile.modelParticleUploads).map(([name, value]) => [name, Number(value) - Number(uploads[name] ?? 0)])) }
      })
      await page.mouse.up({ button: "left" })
      const memory = await page.evaluate(() => ({ heapAfter: (performance as any).memory?.usedJSHeapSize ?? null, assets: (globalThis as any).__playsrcProfile.memoryAssets }))
      expect(sampled.tickRate).toBeGreaterThan(60)
      expect(sampled.frames.length).toBeGreaterThan(30)
      expect(sampled.frames.every((frame: any) => frame.bots === 23)).toBe(true)
      performanceSample = { ...sampled, ...memory, browser: summarizeFrameTimes(sampled.intervals), completed: summarizeFrameTimes(sampled.frames.map((frame: any) => frame.total)) }
    }
    await page.screenshot({ path: path.join(directory, `${definition}-fired.png`) })
    const after = (await actual())!
    if (definition === 595) {
      await expect.poll(async () => page.evaluate(() => (globalThis as any).__playsrcProfile.projectileQueries?.maxPossiblePixels ?? 0)).toBeGreaterThan(1)
      await page.mouse.down({ button: "right" })
      await expect.poll(() => main.getAttribute("data-audio-starts")).toContain("Weapon_ManMelter.altfire_lp")
      await page.screenshot({ path: path.join(directory, "595-vacuum.png") })
      await page.mouse.up({ button: "right" })
    }
    records.push({ definition, model: after.model, before: { clip: before.ammo.clip, reserve: before.ammo.reserve },
      after: { clip: after.ammo.clip, reserve: after.ammo.reserve }, audio: await main.getAttribute("data-audio-starts"),
      visibilityQueries: await page.evaluate(() => (globalThis as any).__playsrcProfile.projectileQueries), performance: performanceSample })
    await writeFile(path.join(directory, "matrix.json"), JSON.stringify({ requested, complete: false, records, errors }, null, 2))
    await command("-attack")
  }
  expect(errors).toEqual([])
  await writeFile(path.join(directory, "matrix.json"), JSON.stringify({ requested, complete: true, records, errors }, null, 2))
})
