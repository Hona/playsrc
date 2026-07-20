import { mkdir, rm, writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, test, type Page } from "@playwright/test"
import { loadLocalConfig } from "../src/config"

const TARGET = "jump_beef"
const VIEWPORTS = Object.freeze([
  Object.freeze({ width: 1280, height: 720 }),
  Object.freeze({ width: 1024, height: 768 }),
  Object.freeze({ width: 1600, height: 900 }),
  Object.freeze({ width: 1280, height: 720 }),
])
const PANELS = Object.freeze([
  "HudViewport",
  "HudPlayerStatus",
  "HudPlayerClass",
  "PlayerStatusClassImage",
  "HudPlayerHealth",
  "PlayerStatusHealthImage",
  "HudWeaponAmmo",
  "HudWeaponSelection",
  "HudCrosshair",
])

type Rect = Readonly<{ x: number; y: number; width: number; height: number }>
type Capture = Readonly<{
  viewport: Readonly<{ width: number; height: number }>
  host: Rect
  panels: Readonly<Record<string, Readonly<{ id: string; parent: string | null; local: Rect; rect: Rect }>>>
}>

const scaled = (value: number, height: number): number => Math.trunc(value * height / 480)
const rect = (x: number, y: number, width: number, height: number): Rect => Object.freeze({ x, y, width, height })

function expected(width: number, height: number): Readonly<Record<string, Rect>> {
  return Object.freeze({
    HudViewport: rect(0, 0, width, height),
    HudPlayerStatus: rect(0, 0, width, height),
    HudPlayerClass: rect(0, 0, width, height),
    PlayerStatusClassImage: rect(scaled(25, height), height - scaled(88, height), scaled(75, height), scaled(75, height)),
    HudPlayerHealth: rect(0, height - scaled(120, height), scaled(250, height), scaled(120, height)),
    PlayerStatusHealthImage: rect(scaled(75, height), scaled(35, height), scaled(51, height), scaled(51, height)),
    HudWeaponAmmo: rect(width - scaled(95, height), height - scaled(55, height), scaled(94, height), scaled(45, height)),
    HudWeaponSelection: rect(0, 0, width, height),
    HudCrosshair: rect(0, 0, scaled(640, height), height),
  })
}

async function settle(page: Page): Promise<void> {
  await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))))
}

test("profile TF2 HUD layout and viewport resize", async ({ page }) => {
  const local = await loadLocalConfig()
  const outputDirectory = path.join(local.sourceCacheDir, "profiles", "hud", TARGET)
  await rm(outputDirectory, { recursive: true, force: true })
  await mkdir(outputDirectory, { recursive: true })

  await page.goto("/", { waitUntil: "domcontentloaded", timeout: 30_000 })
  await page.waitForFunction(() => {
    const phase = document.querySelector("main")?.getAttribute("data-phase")
    return phase === "MainMenu" || phase === "Failed"
  }, undefined, { timeout: 180_000, polling: 50 })
  expect(await page.locator("main").getAttribute("data-phase")).toBe("MainMenu")

  await page.keyboard.press("Backquote")
  const consoleEntry = page.locator("[aria-label='Console command']")
  await expect(consoleEntry).toBeVisible()
  await consoleEntry.fill(`map ${TARGET}`)
  await page.keyboard.press("Enter")
  await page.waitForFunction(() => {
    const main = document.querySelector<HTMLElement>("main")
    return main?.dataset.phase === "Ready" && main.dataset.gameui === "in-game"
  }, undefined, { timeout: 600_000, polling: 50 })
  await page.keyboard.press("Backquote")

  const captures: Capture[] = []
  for (let index = 0; index < VIEWPORTS.length; index += 1) {
    const viewport = VIEWPORTS[index]!
    await page.setViewportSize(viewport)
    await settle(page)
    const capture = await page.evaluate(({ names, viewport }) => {
      const readRect = (element: Element) => {
        const value = element.getBoundingClientRect()
        return { x: value.x, y: value.y, width: value.width, height: value.height }
      }
      const host = document.querySelector("[data-vgui-runtime='tf2-hud']")
      if (!host) throw new Error("TF2 HUD runtime host is unavailable")
      const panels: Record<string, { id: string; parent: string | null; local: ReturnType<typeof readRect>; rect: ReturnType<typeof readRect> }> = {}
      for (const name of names) {
        const element = host.querySelector<HTMLElement>(`[data-vgui-name="${name}"]`)
        if (!element) throw new Error(`TF2 HUD panel ${name} is unavailable`)
        panels[name] = {
          id: element.id,
          parent: element.parentElement?.dataset.vguiName ?? null,
          local: {
            x: Number.parseFloat(element.style.left),
            y: Number.parseFloat(element.style.top),
            width: Number.parseFloat(element.style.width),
            height: Number.parseFloat(element.style.height),
          },
          rect: readRect(element),
        }
      }
      return { viewport, host: readRect(host), panels }
    }, { names: PANELS, viewport })
    captures.push(capture)
    await page.screenshot({ path: path.join(outputDirectory, `hud-${index + 1}-${viewport.width}x${viewport.height}.png`) })
  }

  const report = Object.freeze({
    schema: "playsrc-tf2-hud-layout-profile-v1",
    target: TARGET,
    captures: Object.freeze(captures),
  })
  await writeFile(path.join(outputDirectory, "report.json"), `${JSON.stringify(report, null, 2)}\n`)

  for (const capture of captures) {
    expect(capture.host).toEqual(rect(0, 0, capture.viewport.width, capture.viewport.height))
    const expectedPanels = expected(capture.viewport.width, capture.viewport.height)
    for (const name of PANELS) expect(capture.panels[name]!.local, `${name} at ${capture.viewport.width}x${capture.viewport.height}`).toEqual(expectedPanels[name])
  }
  const stableGeometry = (capture: Capture | undefined) => Object.fromEntries(Object.entries(capture?.panels ?? {}).map(([name, panel]) => [name, {
    id: panel.id,
    parent: panel.parent,
    local: panel.local,
  }]))
  expect(stableGeometry(captures.at(-1))).toEqual(stableGeometry(captures[0]))
})
