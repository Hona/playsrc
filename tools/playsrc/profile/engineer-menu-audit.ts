import { writeFile } from "node:fs/promises"
import { spawnSync } from "node:child_process"
import path from "node:path"
import { expect as playwrightExpect, type Page, type Locator } from "@playwright/test"
import { tf2UiResources } from "@playsrc/game-tf2-browser/ui-resources"

const expect = playwrightExpect.configure({ timeout: 3000 })

export async function auditEngineerMenus(page: Page, root: Locator, directory: string, label: string, command: (value: string) => Promise<void>) {
  await command("joinclass engineer")
  await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[1]).toBe("9")
  const menus: unknown[] = []
  const retain = () => writeFile(path.join(directory, `${label}-engineer-ui.json`), JSON.stringify({ menus, performanceSample: false }))
  for (const [key, menu] of [["Digit4", "build"], ["Digit5", "destroy"]] as const) {
    await page.keyboard.press(key)
    await expect(root).toHaveAttribute("data-engineer-menu", menu)
    const file = path.join(directory, `${label}-engineer-${menu}.png`)
    await page.screenshot({ path: file })
    const native = path.join(directory, `${label}-engineer-${menu}.desktop.png`)
    if (process.platform === "darwin" && spawnSync("screencapture", ["-x", native], { timeout: 5000 }).status !== 0) throw new Error("Engineer desktop capture failed")
    menus.push({ menu, file, native, controls: await page.locator(".engineer-layer").evaluate(element => ({
      panels: element.querySelectorAll("[data-vgui-panel]").length, rasters: element.querySelectorAll("canvas[data-vgui-raster]").length,
      visibleText: (element as HTMLElement).innerText,
    })) })
  }
  await retain()
  if (process.env.PROFILE_ENGINEER_BINDING_AUDIT !== "1") return
  await page.keyboard.press("Escape")
  await page.locator("[data-vgui-name='SettingsButton']").click()
  await expect(page.locator("main")).toHaveAttribute("data-options-visible", "true")
  const list = page.locator("[data-vgui-name='listpanel_keybindlist']")
  const row = tf2UiResources.keyboardActions.findIndex(action => action.binding === "lastinv")
  expect(row).toBeGreaterThanOrEqual(0)
  const target = list.locator(`[data-vgui-item='${row + 1}']`)
  await list.hover()
  for (let attempt = 0; attempt < 32; attempt++) {
    const bounds = await target.boundingBox(), viewport = await list.boundingBox()
    if (bounds && viewport && bounds.y >= viewport.y && bounds.y + bounds.height <= viewport.y + viewport.height) break
    await page.mouse.wheel(0, 240)
    await page.waitForTimeout(40)
  }
  // VGUI owns scrolling. Click visible pixels without implicit scrollIntoView.
  const bounds = await target.boundingBox()
  if (!bounds) throw new Error("Last weapon binding row has no visible bounds")
  await page.mouse.click(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2)
  await expect(target).toHaveAttribute("aria-selected", "true", { timeout: 3000 })
  await page.locator("[data-vgui-name='ChangeKeyButton']").click()
  await page.keyboard.press("F8")
  await page.screenshot({ path: path.join(directory, `${label}-options-rebound.png`) })
  await expect(target.locator("[data-vgui-column='Key']")).toHaveText("F8", { timeout: 3000 })
  await page.locator("[data-vgui-name='OptionsDialog'] [data-vgui-name='OKButton']").click()
  await expect(page.locator("main")).toHaveAttribute("data-options-visible", "false")
  await page.locator("[data-vgui-name='ResumeButton']").click()
  await expect(root).toHaveAttribute("data-gameui", "in-game")
  await page.keyboard.press("F8")
  await expect(root).toHaveAttribute("data-engineer-menu", "none")
  for (const [key, menu] of [["Digit4", "build"], ["Digit5", "destroy"]] as const) {
    await page.keyboard.press("Digit3")
    await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[2]).toBe("42")
    await page.keyboard.press(key)
    await expect(root).toHaveAttribute("data-engineer-menu", menu)
    const cancel = page.locator(`.engineer-layer [data-vgui-name='${menu === "build" ? "HudMenuEngyBuild" : "HudMenuEngyDestroy"}'] [data-vgui-name='CancelLabel']`)
    await expect(cancel).toContainText(/'F8'/i)
    const file = path.join(directory, `${label}-engineer-${menu}-rebound.png`)
    await page.screenshot({ path: file })
    const text = await cancel.innerText()
    await page.keyboard.press("F8")
    await expect(root).toHaveAttribute("data-engineer-menu", "none")
    await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[2]).toBe("42")
    menus.push({ menu, file, binding: text, canceledTo: 42 })
    await retain()
  }
}
