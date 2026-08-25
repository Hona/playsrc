import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, test } from "./application-test"
import { loadLocalConfig } from "../src/config"

const CLASSES = ["scout", "soldier", "pyro", "demoman", "heavyweapons", "engineer", "medic", "sniper", "spy"] as const

test("authored TF2 class selection shows real model pixels and preserves gameplay transitions", async ({ page }) => {
  const local = await loadLocalConfig()
  const output = path.join(local.sourceCacheDir, "profiles", "class-selection")
  await mkdir(output, { recursive: true })
  await page.goto("/", { waitUntil: "domcontentloaded", timeout: 30_000 })
  await page.keyboard.press("Escape")
  await page.waitForFunction(() => document.querySelector<HTMLElement>("main")?.dataset.phase === "MainMenu", undefined, { timeout: 180_000 })
  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  await expect(entry).toBeVisible()
  await entry.fill("map jump_beef")
  await page.keyboard.press("Enter")
  await page.waitForFunction(() => document.querySelector<HTMLElement>("main")?.dataset.teamSelectionVisible === "true", undefined, { timeout: 480_000 })
  if (await page.locator("main").getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  await page.locator(".team-selection-layer [data-vgui-name='teambutton1']").click()
  await page.waitForFunction(() => {
    const main = document.querySelector<HTMLElement>("main")
    return main?.dataset.phase === "Ready" && main.dataset.classSelectionVisible === "true"
  }, undefined, { timeout: 60_000 })
  await page.waitForFunction(() => /TFPlayerModel:[^|]+:[0-9]+:[1-9][0-9]*/u.test(document.querySelector<HTMLElement>("main")?.dataset.classSelectionModels ?? ""), undefined, { timeout: 60_000 })
  await page.screenshot({ path: path.join(output, "initial-join.png") })
  const canvasScreenshot = await page.locator("canvas.world-canvas").screenshot()
  const initial = await page.evaluate(async ({ classes, screenshot }) => ({
    state: { ...document.querySelector<HTMLElement>("main")!.dataset },
    classes: classes.map((name) => {
      const element = document.querySelector<HTMLElement>(`.class-selection-layer [data-vgui-name='${name}']`)
      const image = element?.querySelector<HTMLElement>("[data-vgui-name='SubImage']")
      return { name, visible: Boolean(element && element.getBoundingClientRect().width > 0), image: image?.dataset.vguiImage ?? "", rect: element?.getBoundingClientRect().toJSON() }
    }),
    canvas: await (async () => {
      const bitmap = await createImageBitmap(await (await fetch(`data:image/png;base64,${screenshot}`)).blob())
      const copy = document.createElement("canvas")
      copy.width = bitmap.width
      copy.height = bitmap.height
      const context = copy.getContext("2d")!
      context.drawImage(bitmap, 0, 0)
      const pixels = context.getImageData(0, 0, copy.width, copy.height).data
      let nonBlack = 0
      for (let index = 0; index < pixels.length; index += 4) if (pixels[index]! > 8 || pixels[index + 1]! > 8 || pixels[index + 2]! > 8) nonBlack += 1
      return { width: copy.width, height: copy.height, nonBlack }
    })(),
  }), { classes: CLASSES, screenshot: canvasScreenshot.toString("base64") })
  expect(initial.classes.every((value) => value.visible)).toBe(true)
  expect(initial.state.classSelectionSelected).toBe("6")
  expect(initial.canvas.nonBlack).toBeGreaterThan(20_000)
  const menuPerformance = await page.evaluate(() => new Promise<{ milliseconds: number; frames: number; p95Milliseconds: number; simulationTicks: number }>((resolve) => {
    const root = document.querySelector<HTMLElement>("main")!
    const started = performance.now()
    const startTick = Number(root.dataset.snapshotTick)
    const frames: number[] = []
    let previous = started
    const sample = (now: number) => {
      frames.push(now - previous)
      previous = now
      if (now - started < 5_000) requestAnimationFrame(sample)
      else {
        const sorted = [...frames].sort((left, right) => left - right)
        resolve({
          milliseconds: now - started,
          frames: frames.length,
          p95Milliseconds: sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.95))]!,
          simulationTicks: Number(root.dataset.snapshotTick) - startTick,
        })
      }
    }
    requestAnimationFrame(sample)
  }))
  expect(menuPerformance.frames).toBeGreaterThan(120)
  expect(menuPerformance.simulationTicks).toBeGreaterThan(200)
  expect(menuPerformance.p95Milliseconds).toBeLessThan(50)
  await page.keyboard.press("Escape")
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "true")
  await page.keyboard.press("Digit2")
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "false")
  await page.keyboard.press("Comma")
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "true")
  const demoman = page.locator(".class-selection-layer [data-vgui-name='demoman']")
  await demoman.hover()
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-selected", "4")
  await page.screenshot({ path: path.join(output, "demoman-preview.png") })
  await demoman.click()
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "false")
  await page.waitForFunction(() => (document.querySelector<HTMLElement>("main")?.dataset.hudPresentationProbe ?? "").includes("class_demored"), undefined, { timeout: 30_000 })
  const identities = [1, 3, 7, 4, 6, 9, 5, 2, 8]
  const classes: { name: string; identity: number; model: string; hud: string }[] = []
  for (const [index, name] of CLASSES.entries()) {
    await page.keyboard.press("Comma")
    await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "true")
    await page.keyboard.press(`Digit${index + 1}`)
    await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "false")
    await expect.poll(async () => (await page.locator("main").getAttribute("data-hud-probe"))?.split(":")[1]).toBe(String(identities[index]))
    const state = await page.locator("main").evaluate((element) => ({
      hud: element.dataset.hudProbe ?? "",
      model: JSON.parse(element.dataset.hudPresentationProbe ?? "{}").classModel?.model ?? "",
    }))
    classes.push({ name, identity: identities[index]!, ...state })
  }
  await page.keyboard.press("Backquote")
  await entry.fill("jointeam blue")
  await page.keyboard.press("Enter")
  await page.keyboard.press("Backquote")
  await expect.poll(async () => {
    const value = JSON.parse((await page.locator("main").getAttribute("data-hud-presentation-probe")) ?? "{}")
    return value.classModel?.scalars?.team
  }).toBe(3)
  await page.keyboard.press("Comma")
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-team", "3")
  await page.screenshot({ path: path.join(output, "blue-team-preview.png") })
  const previousClass = (await page.locator("main").getAttribute("data-hud-probe"))?.split(":")[1]
  await page.locator(".class-selection-layer [data-vgui-name='random']").click()
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "false")
  await expect.poll(async () => (await page.locator("main").getAttribute("data-hud-probe"))?.split(":")[1]).not.toBe(previousClass)
  await page.keyboard.press("Comma")
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "true")
  await page.keyboard.press("Escape")
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "false")
  const terminal = await page.locator("main").evaluate((element) => ({ ...element.dataset }))
  expect(terminal.phase).toBe("Ready")
  expect(terminal.gameui).toBe("in-game")
  await writeFile(path.join(output, "report.json"), `${JSON.stringify({ initial, menuPerformance, classes, terminal }, null, 2)}\n`)
})
