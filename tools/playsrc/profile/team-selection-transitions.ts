import type { Page } from "@playwright/test"
import { expect } from "./application-test"
import { chooseTf2Team } from "./team-selection-evidence"

export async function exerciseTf2TeamTransitions(page: Page): Promise<Readonly<{
  transitions: readonly number[]
  spectatorHudHidden: boolean
  cancelRestoresGameplay: boolean
}>> {
  const root = page.locator("main")
  const transitions: number[] = [Number(await root.getAttribute("data-team-selection-local"))]

  await page.keyboard.press("Period")
  await expect(root).toHaveAttribute("data-team-selection-visible", "true")
  await expect(root).toHaveAttribute("data-team-selection-local", "2")
  await expect(root).toHaveAttribute("data-team-selection-red-count", "1")
  await expect(root).toHaveAttribute("data-team-selection-blue-count", "0")
  const cancel = page.locator(".team-selection-layer [data-vgui-name='CancelButton']")
  await expect(cancel).toBeVisible()
  await page.keyboard.press("Escape")
  await expect(root).toHaveAttribute("data-team-selection-visible", "false")
  expect(await root.getAttribute("data-gameui")).toBe("in-game")

  await page.keyboard.press("Period")
  await chooseTf2Team(page, "blue")
  await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[2]).not.toBe("undefined")
  transitions.push(Number(await root.getAttribute("data-team-selection-local")))

  await page.keyboard.press("Period")
  await expect(root).toHaveAttribute("data-team-selection-local", "3")
  await expect(root).toHaveAttribute("data-team-selection-red-count", "0")
  await expect(root).toHaveAttribute("data-team-selection-blue-count", "1")
  const spectate = page.locator(".team-selection-layer [data-vgui-name='teambutton3']")
  await expect(spectate).toBeVisible()
  await spectate.click()
  await expect(root).toHaveAttribute("data-team-selection-local", "1")
  await expect(root).toHaveAttribute("data-team-selection-visible", "false")
  await page.waitForFunction(() => {
    const main = document.querySelector<HTMLElement>("main")
    const hud = document.querySelector<HTMLElement>(".hud-layer")
    return main?.dataset.teamSelectionLocal === "1" && hud !== null
      && [...hud.querySelectorAll<HTMLElement>("[data-vgui-name='HudPlayerStatus']")]
        .every((panel) => getComputedStyle(panel).display === "none" || getComputedStyle(panel).visibility === "hidden")
  }, undefined, { timeout: 30_000 })
  transitions.push(Number(await root.getAttribute("data-team-selection-local")))

  await page.keyboard.press("Period")
  await expect(root).toHaveAttribute("data-team-selection-visible", "true")
  await chooseTf2Team(page, "red")
  transitions.push(Number(await root.getAttribute("data-team-selection-local")))
  expect(transitions).toEqual([2, 3, 1, 2])

  return Object.freeze({
    transitions: Object.freeze(transitions),
    spectatorHudHidden: true,
    cancelRestoresGameplay: true,
  })
}
