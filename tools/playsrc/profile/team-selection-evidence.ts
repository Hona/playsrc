import { mkdir } from "node:fs/promises"
import path from "node:path"
import type { Page } from "@playwright/test"
import { expect } from "./application-test"
import { decodeScreenshot } from "./screenshot-pixels"
import { loadLocalConfig } from "../src/config"

export type Tf2TeamSelectionEvidence = Readonly<{
  localTeam: number
  redCount: number
  blueCount: number
  buttons: readonly Readonly<{ name: string; label: string; visible: boolean }>[]
  pixels: Readonly<{ width: number; height: number; nonBlack: number; redDominant: number; blueDominant: number }>
  models: string
}>

export async function captureTf2TeamSelection(page: Page): Promise<Tf2TeamSelectionEvidence> {
  await page.waitForFunction(() => {
    const main = document.querySelector<HTMLElement>("main")
    return main?.dataset.teamSelectionVisible === "true"
      && (main.dataset.teamSelectionModels ?? "").includes("MenuBG:")
      && (main.dataset.teamSelectionModels ?? "").includes("reddoor:")
      && (main.dataset.teamSelectionModels ?? "").includes("bluedoor:")
  }, undefined, { timeout: 60_000 })
  const state = await page.evaluate(() => {
    const main = document.querySelector<HTMLElement>("main")!
    const buttons = ["teambutton0", "teambutton1", "teambutton2", "teambutton3"].map((name) => {
      const element = document.querySelector<HTMLElement>(`.team-selection-layer [data-vgui-name='${name}']`)
      const bounds = element?.getBoundingClientRect()
      return {
        name,
        label: element?.getAttribute("aria-label") ?? "",
        visible: Boolean(element && bounds && bounds.width > 0 && bounds.height > 0
          && getComputedStyle(element).visibility !== "hidden" && getComputedStyle(element).display !== "none"),
      }
    })
    return {
      localTeam: Number(main.dataset.teamSelectionLocal),
      redCount: Number(main.dataset.teamSelectionRedCount),
      blueCount: Number(main.dataset.teamSelectionBlueCount),
      buttons,
      models: main.dataset.teamSelectionModels ?? "",
    }
  })
  const local = await loadLocalConfig()
  const output = path.join(local.sourceCacheDir, "profiles", "team-selection")
  await mkdir(output, { recursive: true })
  const screenshot = await page.screenshot({ path: path.join(output, "initial-unassigned-team-menu.png") })
  const decoded = decodeScreenshot(screenshot)
  let nonBlack = 0
  let redDominant = 0
  let blueDominant = 0
  for (let offset = 0; offset < decoded.pixels.length; offset += decoded.channels) {
    const red = decoded.pixels[offset]!
    const green = decoded.pixels[offset + 1]!
    const blue = decoded.pixels[offset + 2]!
    if (red > 8 || green > 8 || blue > 8) nonBlack += 1
    if (red > blue * 1.15 && red > green * 1.05) redDominant += 1
    if (blue > red * 1.1 && blue > green * 0.9) blueDominant += 1
  }
  const evidence: Tf2TeamSelectionEvidence = Object.freeze({
    ...state,
    pixels: Object.freeze({ width: decoded.width, height: decoded.height, nonBlack, redDominant, blueDominant }),
  })
  console.log(`TF2_TEAM_SELECTION ${JSON.stringify(evidence)}`)
  expect(evidence.localTeam).toBe(0)
  expect(evidence.redCount).toBe(0)
  expect(evidence.blueCount).toBe(0)
  expect(evidence.buttons.map((button) => button.label)).toEqual(["BLU", "RED", "Auto-assign", "Spectate"])
  expect(evidence.buttons.every((button) => button.visible)).toBe(true)
  expect(evidence.pixels.nonBlack).toBeGreaterThan(20_000)
  expect(evidence.pixels.redDominant).toBeGreaterThan(250)
  expect(evidence.pixels.blueDominant).toBeGreaterThan(250)
  return Object.freeze(evidence)
}

export async function chooseTf2Team(page: Page, team: "red" | "blue"): Promise<void> {
  const button = page.locator(`.team-selection-layer [data-vgui-name='${team === "red" ? "teambutton1" : "teambutton0"}']`)
  await expect(button).toBeVisible()
  await button.click()
  await expect(page.locator("main")).toHaveAttribute("data-team-selection-visible", "false")
  await expect(page.locator("main")).toHaveAttribute("data-team-selection-local", team === "red" ? "2" : "3")
  await page.waitForFunction((selected) => {
    const main = document.querySelector<HTMLElement>("main")
    if (!main || main.dataset.phase === "Failed") return true
    if (main.dataset.classSelectionVisible === "true") return true
    if (main.dataset.phase !== "Ready") return false
    try {
      return String(JSON.parse(main.dataset.hudPresentationProbe ?? "{}").classModel?.scalars?.team) === String(selected)
    } catch {
      return false
    }
  }, team === "red" ? 2 : 3, { timeout: 60_000 })
  if (await page.locator("main").getAttribute("data-class-selection-visible") === "true") {
    await page.keyboard.press("Digit2")
    await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "false")
  }
}
