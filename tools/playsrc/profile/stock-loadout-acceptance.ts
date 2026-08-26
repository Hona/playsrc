import path from "node:path"
import { writeFile } from "node:fs/promises"
import type { Page } from "@playwright/test"
import { expect } from "./application-test"

// Browser protocol identities for the configured stock primary/secondary/melee
// loadouts. Spy's secondary is the sapper, not a combat firearm.
const CLASSES = [
  { digit: 1, identity: 1, name: "scout", weapons: [4, 5, 6] },
  { digit: 2, identity: 3, name: "soldier", weapons: [1, 7, 8] },
  { digit: 3, identity: 7, name: "pyro", weapons: [15, 7, 16] },
  { digit: 4, identity: 4, name: "demoman", weapons: [18, 3, 17] },
  { digit: 5, identity: 6, name: "heavy", weapons: [9, 10, 11] },
  { digit: 6, identity: 9, name: "engineer", weapons: [40, 41, 42] },
  { digit: 7, identity: 5, name: "medic", weapons: [19, 20, 21] },
  { digit: 8, identity: 2, name: "sniper", weapons: [12, 13, 14] },
  { digit: 9, identity: 8, name: "spy", weapons: [50, 52, 51] },
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
    for (const [slot, weapon] of playerClass.weapons.entries()) {
      await page.keyboard.press(`Digit${slot + 1}`)
      await expect.poll(async () => (await main.getAttribute("data-hud-probe"))?.split(":")[2], { timeout: 5000 }).toBe(String(weapon))
      await page.bringToFront()
      await page.locator("canvas.world-canvas").click()
      try { await expect(main).toHaveAttribute("data-pointer-locked", "true", { timeout: 5000 }) }
      catch { throw new Error(`Native pointer capture failed for ${playerClass.name}/${slot + 1}: ${JSON.stringify(await main.evaluate(element => ({ detail: element.dataset.detail, focus: document.hasFocus(), visible: document.visibilityState, classSelection: element.dataset.classSelectionVisible, locked: Boolean(document.pointerLockElement) })))}`) }
      if (slot === 0) lastCapture = Date.now()
      await page.mouse.move(650, 370)
      await page.mouse.click(650, 370)
      await expect(main).toHaveAttribute("data-viewmodel-world-depth-isolated", "true")
      const file = `${label}-${playerClass.name}-slot${slot + 1}.png`
      await page.screenshot({ path: path.join(directory, file) })
      results.push(await main.evaluate((element, fixture) => ({
        ...fixture, hud: element.dataset.hudProbe, modelPanel: (globalThis as any).__playsrcProfile.hudModelPanel,
        activity: element.dataset.viewmodelActivity, depth: element.dataset.viewmodelWorldDepthIsolated,
        presentation: JSON.parse(element.dataset.hudPresentationProbe ?? "{}"),
      }), { playerClass: playerClass.name, slot: slot + 1, weapon, file }))
      await writeFile(path.join(directory, `${label}-stock.json`), JSON.stringify(results, null, 2))
    }
  }
  return results
}
