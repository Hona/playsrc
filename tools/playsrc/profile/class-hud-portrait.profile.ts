import { createHash } from "node:crypto"
import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import type { Page } from "@playwright/test"
import { expect, test } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { settleTf2Gameplay } from "./team-selection-evidence"

const PORTRAIT = "[data-vgui-runtime='tf2-hud'] [data-vgui-name='PlayerStatusClassImage']"
const MODEL = "[data-vgui-runtime='tf2-hud'] [data-vgui-name='classmodelpanel']"
const SETTING = "[data-vgui-runtime='tf2-advanced-options'] [data-vgui-name='AdvancedRow36'] [data-vgui-name='DescCheckButton']"

type PortraitEvidence = Readonly<{
  image: string
  width: number
  height: number
  sampledPixels: number
  matchingPixels: number
  screenshotSha256: string
  rasterCanvases: number
  backgroundImage: string
}>

async function setPlayerModel(page: Page, enabled: boolean): Promise<void> {
  await page.locator("[data-vgui-name='TF2SettingsButton']").click()
  const checkbox = page.locator(SETTING)
  await expect(checkbox).toHaveAttribute("aria-checked", String(!enabled))
  for (let attempt = 0; attempt < 64; attempt += 1) {
    const admitted = await checkbox.evaluate((element) => {
      const list = document.querySelector<HTMLElement>("[data-vgui-runtime='tf2-advanced-options'] [data-vgui-name='PanelListPanel']")
      if (!list) throw new Error("Advanced Options list is unavailable")
      const bounds = element.getBoundingClientRect()
      const listBounds = list.getBoundingClientRect()
      const target = document.elementFromPoint(bounds.left + 8, bounds.top + 8)
      if (target === element || element.contains(target)) return true
      list.dispatchEvent(new WheelEvent("wheel", {
        bubbles: true,
        cancelable: true,
        clientX: listBounds.left + listBounds.width / 2,
        clientY: listBounds.top + listBounds.height / 2,
        deltaY: bounds.bottom > listBounds.bottom ? 200 : -200,
      }))
      return false
    })
    if (admitted) break
    await page.waitForTimeout(20)
  }
  await checkbox.click({ position: { x: 8, y: 8 } })
  await expect(checkbox).toHaveAttribute("aria-checked", String(enabled))
  await page.locator("[data-vgui-runtime='tf2-advanced-options'] [data-vgui-name='OkButton']").click()
  await page.waitForFunction(() => document.querySelector<HTMLElement>("main")?.dataset.optionsVisible === "false")
}

async function capturePortrait(page: Page, image: string, output: string): Promise<PortraitEvidence> {
  await page.waitForFunction((expected) => {
    const value = document.querySelector<HTMLElement>("main")?.dataset.hudPresentationProbe
    if (!value || value === "unavailable") return false
    const probe = JSON.parse(value) as { classImage: { visible: boolean; image: string } }
    return probe.classImage.visible && probe.classImage.image === expected
  }, image)
  const element = page.locator(PORTRAIT)
  await expect(element).toBeVisible()
  const screenshot = await element.screenshot({ path: output, animations: "disabled" })
  const evidence = await element.evaluate(async (portrait, bytes) => {
    const style = getComputedStyle(portrait)
    const match = style.backgroundImage.match(/^url\("?(blob:[^"\)]+)"?\)$/u)
    if (!match) throw new Error(`Class portrait does not directly present its authored texture: ${style.backgroundImage}`)
    const screenshot = await createImageBitmap(new Blob([Uint8Array.from(bytes)], { type: "image/png" }))
    const authored = await createImageBitmap(await (await fetch(match[1]!)).blob())
    const capture = document.createElement("canvas")
    capture.width = screenshot.width
    capture.height = screenshot.height
    const context = capture.getContext("2d", { willReadFrequently: true })
    if (!context) throw new Error("Class portrait pixel evidence is unavailable")
    context.drawImage(screenshot, 0, 0)
    const screen = context.getImageData(0, 0, capture.width, capture.height).data
    context.clearRect(0, 0, capture.width, capture.height)
    context.drawImage(authored, 0, 0, capture.width, capture.height)
    const expected = context.getImageData(0, 0, capture.width, capture.height).data
    let sampledPixels = 0
    let matchingPixels = 0
    for (let y = 0; y < Math.floor(capture.height * 0.65); y += 1) {
      for (let x = 0; x < Math.floor(capture.width * 0.6); x += 1) {
        const index = (y * capture.width + x) * 4
        if (expected[index + 3]! !== 255) continue
        sampledPixels += 1
        if (Math.max(
          Math.abs(screen[index]! - expected[index]!),
          Math.abs(screen[index + 1]! - expected[index + 1]!),
          Math.abs(screen[index + 2]! - expected[index + 2]!),
        ) <= 12) matchingPixels += 1
      }
    }
    screenshot.close()
    authored.close()
    return {
      width: capture.width,
      height: capture.height,
      sampledPixels,
      matchingPixels,
      rasterCanvases: portrait.querySelectorAll(":scope > [data-vgui-raster='image-raster']").length,
      backgroundImage: style.backgroundImage,
    }
  }, [...screenshot])
  expect(evidence.sampledPixels, `${image} authored opaque pixels`).toBeGreaterThan(300)
  expect(evidence.matchingPixels / evidence.sampledPixels, `${image} visible authored pixels`).toBeGreaterThan(0.8)
  expect(evidence.rasterCanvases, `${image} CPU raster canvases`).toBe(0)
  return Object.freeze({ image, ...evidence, screenshotSha256: createHash("sha256").update(screenshot).digest("hex") })
}

test("profile TF2 authored class HUD pixels, mode changes, and frame cadence", async ({ page }) => {
  const local = await loadLocalConfig()
  const directory = path.join(local.sourceCacheDir, "profiles", "class-hud", "jump_beef")
  await mkdir(directory, { recursive: true })

  await page.goto("/", { waitUntil: "domcontentloaded", timeout: 30_000 })
  await page.waitForFunction(() => ["Startup", "MainMenu", "Failed"].includes(document.querySelector<HTMLElement>("main")?.dataset.phase ?? ""))
  if (await page.locator("main").getAttribute("data-phase") === "Startup") await page.keyboard.press("Escape")
  await page.waitForFunction(() => document.querySelector<HTMLElement>("main")?.dataset.phase === "MainMenu", undefined, { timeout: 180_000 })
  await setPlayerModel(page, false)

  await page.keyboard.press("Backquote")
  const command = page.locator("[aria-label='Console command']")
  await command.fill("map jump_beef")
  await page.keyboard.press("Enter")
  await settleTf2Gameplay(page)
  await page.waitForFunction(() => {
    const main = document.querySelector<HTMLElement>("main")
    return !!main?.dataset.hudPresentationProbe && main.dataset.hudPresentationProbe !== "unavailable"
  }, undefined, { timeout: 30_000 })
  await page.keyboard.press("Backquote")
  const soldier = await capturePortrait(page, "../hud/class_soldierred", path.join(directory, "soldier-red.png"))

  await page.keyboard.press("Backquote")
  await command.fill("class demoman")
  await page.keyboard.press("Enter")
  await page.waitForFunction(() => document.querySelector<HTMLElement>("main")?.dataset.hudProbe?.includes(":4:"))
  await page.keyboard.press("Backquote")
  const demoman = await capturePortrait(page, "../hud/class_demored", path.join(directory, "demoman-red.png"))
  expect(demoman.screenshotSha256).not.toBe(soldier.screenshotSha256)

  await page.keyboard.press("Escape")
  await page.waitForFunction(() => document.querySelector<HTMLElement>("main")?.dataset.gameui === "pause")
  await setPlayerModel(page, true)
  await expect(page.locator(PORTRAIT)).toBeHidden()
  await expect(page.locator(MODEL)).toBeVisible()
  const model = await page.evaluate(() => JSON.parse(document.querySelector<HTMLElement>("main")!.dataset.hudPresentationProbe!))
  expect(model.classModel).toMatchObject({ visible: true, model: "models/player/demo.mdl", scalars: { class: 4, team: 2, skin: 0 } })
  await setPlayerModel(page, false)
  await expect(page.locator(PORTRAIT)).toBeVisible()
  await expect(page.locator(MODEL)).toBeHidden()
  await page.keyboard.press("Escape")
  await page.waitForFunction(() => document.querySelector<HTMLElement>("main")?.dataset.gameui === "in-game")
  const restored = await capturePortrait(page, "../hud/class_demored", path.join(directory, "demoman-red-restored.png"))

  const cadence = await page.evaluate(() => new Promise<{ milliseconds: number; frames: number; ticks: number; p95Milliseconds: number }>((resolve) => {
    const main = document.querySelector<HTMLElement>("main")!
    const started = performance.now()
    const initialTick = Number(main.dataset.snapshotTick)
    let previous = started
    const gaps: number[] = []
    const frame = (now: number) => {
      gaps.push(now - previous)
      previous = now
      if (now - started < 5_000) { requestAnimationFrame(frame); return }
      const ordered = gaps.slice().sort((left, right) => left - right)
      resolve({
        milliseconds: now - started,
        frames: gaps.length,
        ticks: Number(main.dataset.snapshotTick) - initialTick,
        p95Milliseconds: ordered[Math.min(ordered.length - 1, Math.floor(ordered.length * 0.95))]!,
      })
    }
    requestAnimationFrame(frame)
  }))
  expect(cadence.milliseconds).toBeGreaterThanOrEqual(5_000)
  expect(cadence.frames).toBeGreaterThan(60)
  expect(cadence.ticks).toBeGreaterThan(150)

  const report = Object.freeze({
    schema: "playsrc-tf2-class-hud-profile-v1",
    target: "jump_beef",
    portraits: Object.freeze([soldier, demoman, restored]),
    playerModel: model.classModel,
    cadence,
  })
  await writeFile(path.join(directory, "report.json"), `${JSON.stringify(report, null, 2)}\n`)
  console.log(JSON.stringify(report))
})
