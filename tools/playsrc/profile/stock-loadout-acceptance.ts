import path from "node:path"
import { writeFile } from "node:fs/promises"
import type { Page } from "@playwright/test"
import { expect } from "./application-test"
import { nativeEquipment } from "../../../games/tf2/browser/tests/fixtures/equipment"

// Class button order is authored; selectable stock items come from the native
// inventory rather than admitting weapons whose gameplay is unavailable.
const CLASSES = [
  { digit: 1, identity: 1, name: "scout" },
  { digit: 2, identity: 3, name: "soldier" },
  { digit: 3, identity: 7, name: "pyro" },
  { digit: 4, identity: 4, name: "demoman" },
  { digit: 5, identity: 6, name: "heavy" },
  { digit: 6, identity: 9, name: "engineer" },
  { digit: 7, identity: 5, name: "medic" },
  { digit: 8, identity: 2, name: "sniper" },
  { digit: 9, identity: 8, name: "spy" },
] as const

export async function acceptStockLoadouts(page: Page, directory: string, label: string) {
  const main = page.locator("main")
  const results: any[] = []
  let lastCapture = 0
  for (const playerClass of CLASSES) {
    // Blink rejects more than four rapid unlock/relock cycles until two real
    // seconds after the last successful lock. This correctness phase is outside
    // sampling; respect that policy rather than emulating/bypassing pointer lock.
    const cooldown = lastCapture + 2100 - Date.now()
    if (cooldown > 0) await page.waitForTimeout(cooldown)
    await page.keyboard.press("Comma")
    await expect(main).toHaveAttribute("data-class-selection-visible", "true")
    await page.keyboard.press(`Digit${playerClass.digit}`)
    await expect(main).toHaveAttribute("data-class-selection-visible", "false")
    await expect.poll(async () => (await main.getAttribute("data-hud-probe"))?.split(":")[1]).toBe(String(playerClass.identity))
    const weapons = nativeEquipment.classes.find(value => value.class === playerClass.identity)!.baseItems.flatMap(item => {
      const selector = nativeEquipment.inventory.find(entry => entry.item.definitionIndex === item.definitionIndex)!.classSlots.find(value => value.class === playerClass.identity && value.slot === item.slot)!
      return selector.weapon !== null && selector.selectionSlot !== null && selector.selectionSlot <= 2 ? [{ slot: selector.selectionSlot, weapon: selector.weapon }] : []
    }).sort((left, right) => left.slot - right.slot)
    for (const { slot, weapon } of weapons) {
      await page.keyboard.press(`Digit${slot + 1}`)
      await expect.poll(async () => (await main.getAttribute("data-hud-probe"))?.split(":")[2], { timeout: 5000 }).toBe(String(weapon))
      // The HUD publishes selection before the authored deploy animation has
      // brought the weapon on screen. Capture the real idle pose, not that
      // transient (possibly completely off-screen) draw frame.
      await expect(main).toHaveAttribute("data-viewmodel-activity", /IDLE/, { timeout: 5000 })
      await page.bringToFront()
      const nativeCaptureRejections: string[] = []
      for (let attempt = 0; attempt < 3; attempt++) {
        if (await main.getAttribute("data-pointer-locked") !== "true") await page.locator("canvas.world-canvas").click()
        try { await expect(main).toHaveAttribute("data-pointer-locked", "true", { timeout: 750 }); break }
        catch {
          const state = await main.evaluate(element => ({ detail: element.dataset.detail, focus: document.hasFocus(), visible: document.visibilityState, classSelection: element.dataset.classSelectionVisible, locked: Boolean(document.pointerLockElement) }))
          // Browser-policy cooldowns are recorded, never bypassed. Retry only
          // this explicit denial with another real user gesture outside sampling.
          if (!state.detail?.includes("Too many pointer lock requests") || attempt === 2) throw new Error(`Native pointer capture failed for ${playerClass.name}/${slot + 1}: ${JSON.stringify(state)}`)
          nativeCaptureRejections.push(state.detail)
          await page.waitForTimeout(2100)
        }
      }
      expect(await page.evaluate(() => document.pointerLockElement === document.querySelector("canvas.world-canvas"))).toBe(true)
      if (slot === 0) lastCapture = Date.now()
      await page.mouse.move(650, 370)
      await expect(main).toHaveAttribute("data-viewmodel-world-depth-isolated", "true")
      const revision = results.length + 1
      await page.evaluate(revision => { (globalThis as any).__playsrcProfile.worldLightingEvidenceRevision = revision }, revision)
      await page.waitForFunction(revision => (globalThis as any).__playsrcProfile.worldLighting?.revision === revision, revision)
      const geometry = await page.evaluate(() => (globalThis as any).__playsrcProfile.worldLighting.geometry)
      expect(geometry.samples.length, `${playerClass.name}/${slot + 1} visible viewmodel depth`).toBeGreaterThan(0)
      const frame = await page.locator("canvas.world-canvas").getAttribute("data-display-frame")
      await page.waitForFunction(frame => Number(document.querySelector<HTMLCanvasElement>("canvas.world-canvas")!.dataset.displayFrame) >= Number(frame) + 2, frame)
      const file = `${label}-${playerClass.name}-slot${slot + 1}.png`
      await page.screenshot({ path: path.join(directory, file) })
      results.push(await main.evaluate((element, fixture) => ({
        ...fixture, hud: element.dataset.hudProbe, modelPanel: (globalThis as any).__playsrcProfile.hudModelPanel,
        activity: element.dataset.viewmodelActivity, depth: element.dataset.viewmodelWorldDepthIsolated,
        presentation: JSON.parse(element.dataset.hudPresentationProbe ?? "{}"),
      }), { playerClass: playerClass.name, slot: slot + 1, weapon, file, geometry, nativeCaptureRejections }))
      await writeFile(path.join(directory, `${label}-stock.json`), JSON.stringify(results, null, 2))
    }
  }
  return results
}
