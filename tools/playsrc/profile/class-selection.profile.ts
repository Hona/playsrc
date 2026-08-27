import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, test } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { decodeScreenshot } from "./screenshot-pixels"

const CLASSES = ["scout", "soldier", "pyro", "demoman", "heavyweapons", "engineer", "medic", "sniper", "spy"] as const

async function assertPortraitMask(page: import("@playwright/test").Page, name: string): Promise<void> {
  const expected = await page.evaluate(async (name) => {
    const image = document.querySelector<HTMLElement>(`.class-selection-layer [data-vgui-name='${name}'] [data-vgui-name='SubImage']`)!
    const url = /^url\(["']?(.*?)["']?\)$/u.exec(getComputedStyle(image).backgroundImage)?.[1]
    if (!url) throw new Error("Selected class portrait has no authored image")
    const bitmap = await createImageBitmap(await (await fetch(url)).blob())
    const mask = document.createElement("canvas")
    mask.width = bitmap.width; mask.height = bitmap.height
    const maskContext = mask.getContext("2d")!
    maskContext.drawImage(bitmap, 0, 0)
    bitmap.close()
    const pixels = maskContext.getImageData(0, 0, mask.width, mask.height).data
    const background = document.querySelector<HTMLCanvasElement>(".class-selection-background-surface")!
    const bg = document.createElement("canvas")
    bg.width = background.width; bg.height = background.height
    const context = bg.getContext("2d")!
    context.drawImage(background, 0, 0)
    const rect = image.getBoundingClientRect()
    const samples: { x: number; y: number; rgb: number[] }[] = []
    for (let y = Math.ceil(mask.height * 0.3); y < mask.height * 0.6 && samples.length < 8; y += 9) {
      for (let x = 3; x < mask.width - 3 && samples.length < 8; x += 9) {
        if ([-1, 0, 1].some(dy => [-1, 0, 1].some(dx => pixels[((y + dy) * mask.width + x + dx) * 4 + 3] !== 0))) continue
        const screenX = Math.floor((rect.x + (x + 0.5) / mask.width * rect.width) * devicePixelRatio)
        const screenY = Math.floor((rect.y + (y + 0.5) / mask.height * rect.height) * devicePixelRatio)
        samples.push({ x: screenX, y: screenY, rgb: [...context.getImageData(screenX, screenY, 1, 1).data].slice(0, 3) })
      }
    }
    return samples
  }, name)
  expect(expected.length, "authored portrait transparent samples").toBe(8)
  const screenshot = decodeScreenshot(await page.screenshot())
  for (const sample of expected) {
    const at = (sample.y * screenshot.width + sample.x) * screenshot.channels
    for (let channel = 0; channel < 3; channel += 1) expect(Math.abs(screenshot.pixels[at + channel]! - sample.rgb[channel]!), "transparent portrait must reveal the authored MenuBG, not a button rectangle or opaque model canvas").toBeLessThanOrEqual(2)
  }
}

test.use({ viewport: { width: 1600, height: 900 }, deviceScaleFactor: Number(process.env.PLAYSRC_PROFILE_DEVICE_SCALE_FACTOR ?? 1.5) })

test("all nine authored class scenes, teams, viewport geometry and native resume", async ({ page }, testInfo) => {
  test.setTimeout(100_000)
  const local = await loadLocalConfig()
  const output = path.join(local.sourceCacheDir, "profiles", "class-selection", `matrix-${process.env.PLAYSRC_PROFILE_DEVICE_SCALE_FACTOR ?? 1.5}`)
  await mkdir(output, { recursive: true })
  const failures: string[] = []
  page.on("pageerror", error => failures.push(error.message))
  await page.goto("/", { waitUntil: "domcontentloaded", timeout: 10_000 })
  const main = page.locator("main")
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 15_000 })
  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  await entry.fill("map jump_beef")
  await entry.press("Enter")
  await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 20_000 })
  if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  await page.locator(".team-selection-layer [data-vgui-name='teambutton1']").click()
  await expect(main).toHaveAttribute("data-class-selection-visible", "true")
  const menu = page.locator(".class-selection-layer")
  await expect(menu.locator("[data-vgui-name='ClassMenuSelect']")).toBeVisible()
  await expect(menu.locator("[data-vgui-name='CancelButton']")).toBeHidden()
  await page.keyboard.press("Escape")
  await expect(main).toHaveAttribute("data-class-selection-visible", "true")
  const samples: unknown[] = []
  for (const team of [2, 3]) {
    if (team === 3) {
      await page.keyboard.press("Digit1")
      await expect(main).toHaveAttribute("data-class-selection-visible", "false")
      await page.keyboard.press("Backquote")
      await entry.fill("jointeam blue")
      await entry.press("Enter")
      await page.keyboard.press("Backquote")
      await expect(main).toHaveAttribute("data-class-selection-visible", "true")
    }
    await expect(main).toHaveAttribute("data-class-selection-team", String(team))
    for (const name of CLASSES) {
      await menu.locator(`[data-vgui-name='${name}']`).hover()
      const model = `models/player/${name === "demoman" ? "demo" : name === "heavyweapons" ? "heavy" : name}.mdl`
      await expect.poll(async () => JSON.parse(await main.getAttribute("data-class-selection-animation") || "{}").model).toBe(model)
      if (name === "scout") await assertPortraitMask(page, name)
      const panel = await menu.locator("[data-vgui-name='TFPlayerModel']").boundingBox()
      if (!panel) throw new Error("Model panel disappeared")
      const clip = { x: panel.x, y: panel.y + panel.height * 0.3, width: panel.width * 0.6, height: panel.height * 0.5 }
      const before = decodeScreenshot(await page.screenshot({ clip }))
      await page.waitForTimeout(350)
      const after = decodeScreenshot(await page.screenshot({ clip }))
      let changed = 0
      for (let at = 0; at < before.pixels.length; at += before.channels) {
        if (Math.abs(before.pixels[at]! - after.pixels[at]!) + Math.abs(before.pixels[at + 1]! - after.pixels[at + 1]!) + Math.abs(before.pixels[at + 2]! - after.pixels[at + 2]!) > 12) changed += 1
      }
      expect(changed, `${team}:${name} authored animated body pixels`).toBeGreaterThan(100)
      const animation = JSON.parse(await main.getAttribute("data-class-selection-animation") || "{}")
      expect(animation.weapon).toMatch(/^models\/weapons\/c_models\//u)
      if (name !== "pyro") expect(animation.flexVertices, `${name} authored facial deformation`).toBeGreaterThan(0)
      await page.screenshot({ path: path.join(output, `${team}-${name}.png`) })
      samples.push({ team, name, changed, animation })
    }
  }
  for (const viewport of [{ width: 1280, height: 960 }, { width: 1280, height: 1024 }, { width: 1950, height: 1080 }, { width: 2560, height: 1080 }]) {
    await page.setViewportSize(viewport)
    await expect.poll(async () => (await menu.locator("[data-vgui-name='MenuBG']").boundingBox())?.height).toBe(viewport.height)
    const model = await menu.locator("[data-vgui-name='TFPlayerModel']").boundingBox()
    expect(model).toMatchObject({ x: 0, y: 0, width: viewport.height, height: viewport.height })
    const footer = await menu.locator("[data-vgui-name='EditLoadoutButton']").boundingBox()
    expect(footer!.y + footer!.height).toBeLessThanOrEqual(viewport.height)
    expect(footer!.x + footer!.width).toBeLessThanOrEqual(viewport.width)
    await page.screenshot({ path: path.join(output, `${viewport.width}x${viewport.height}.png`) })
  }
  await page.keyboard.press("Digit1")
  await expect(main).toHaveAttribute("data-class-selection-visible", "false")
  await page.setViewportSize({ width: 1600, height: 900 })
  await page.bringToFront()
  await page.locator("canvas.world-canvas").focus()
  await page.locator("canvas.world-canvas").click({ position: { x: 400, y: 400 } })
  try { await expect(main).toHaveAttribute("data-pointer-locked", "true", { timeout: 5_000 }) }
  catch { throw new Error(`Native class-menu resume failed: ${JSON.stringify(await main.evaluate(element => ({ detail: element.dataset.detail, focused: document.hasFocus(), visibility: document.visibilityState, owner: document.pointerLockElement?.className })))}`) }
  await page.keyboard.press("Comma")
  await expect(main).toHaveAttribute("data-class-selection-visible", "true")
  await expect(menu.locator("[data-vgui-name='CancelButton']")).toBeVisible()
  await expect(menu.locator("[data-vgui-name='ClassMenuSelect']")).toBeHidden()
  await expect(main).toHaveAttribute("data-team-selection-visible", "false", { timeout: 2_000 })
  expect(await page.evaluate(() => document.pointerLockElement === null)).toBe(true)
  await writeFile(path.join(output, "footer-raster.json"), JSON.stringify(await menu.locator("[data-vgui-name='EditLoadoutButton']").evaluate(element => ({
    rect: element.getBoundingClientRect().toJSON(),
    rasters: [...element.querySelectorAll<HTMLCanvasElement>("canvas")].map(canvas => ({ key: canvas.dataset.vguiRaster,
      rect: canvas.getBoundingClientRect().toJSON(), width: canvas.width, height: canvas.height,
      alpha: [...canvas.getContext("2d")!.getImageData(0, 0, canvas.width, canvas.height).data].filter((_, i) => i % 4 === 3) })),
  })), null, 2))
  await menu.locator("[data-vgui-name='CancelButton']").click({ timeout: 5_000 })
  await expect(main).toHaveAttribute("data-class-selection-visible", "false")
  expect(failures).toEqual([])
  await writeFile(path.join(output, "samples.json"), JSON.stringify({ samples, failures }, null, 2))
  await testInfo.attach("class-selection-matrix", { path: path.join(output, "samples.json"), contentType: "application/json" })
})

test("configured class-select scene advances visible Scout pixels", async ({ page }, testInfo) => {
  test.setTimeout(45_000)
  await page.goto("/", { waitUntil: "domcontentloaded", timeout: 10_000 })
  await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu", { timeout: 15_000 })
  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  await entry.fill("map jump_beef")
  await entry.press("Enter")
  await expect(page.locator("main")).toHaveAttribute("data-team-selection-visible", "true", { timeout: 20_000 })
  if (await page.locator("main").getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  await page.locator(".team-selection-layer [data-vgui-name='teambutton1']").click()
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "true")
  await page.locator(".class-selection-layer [data-vgui-name='scout']").hover()
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-selected", "1")
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-models", /TFPlayerModel:models\/player\/scout\.mdl:0:[1-9]/u)
  const panel = await page.locator(".class-selection-layer [data-vgui-name='TFPlayerModel']").boundingBox()
  if (!panel) throw new Error("Authored Scout viewport is not visible")
  // Keep the sample within the model viewport, below the portrait strip and to
  // the left of the authored tips. The configured class_select sequence moves
  // the body during this interval; non-black static geometry is not acceptance.
  const clip = { x: panel.x, y: panel.y + panel.height * 0.3, width: panel.width * 0.6, height: panel.height * 0.5 }
  const first = await page.screenshot({ clip })
  await page.waitForTimeout(350)
  const second = await page.screenshot({ clip })
  const before = decodeScreenshot(first)
  const after = decodeScreenshot(second)
  let changed = 0
  for (let offset = 0; offset < before.pixels.length; offset += before.channels) {
    if (Math.abs(before.pixels[offset]! - after.pixels[offset]!)
      + Math.abs(before.pixels[offset + 1]! - after.pixels[offset + 1]!)
      + Math.abs(before.pixels[offset + 2]! - after.pixels[offset + 2]!) > 12) changed += 1
  }
  await testInfo.attach("scout-scene-before", { body: first, contentType: "image/png" })
  await testInfo.attach("scout-scene-after", { body: second, contentType: "image/png" })
  await testInfo.attach("scout-scene-pixel-change", { body: JSON.stringify({ clip, changed }), contentType: "application/json" })
  const local = await loadLocalConfig()
  const output = path.join(local.sourceCacheDir, "profiles", "class-selection")
  await mkdir(output, { recursive: true })
  await writeFile(path.join(output, "scout-body-before.png"), first)
  await writeFile(path.join(output, "scout-body-after.png"), second)
  await page.screenshot({ path: path.join(output, "scout-composition.png") })
  expect(changed, "configured Scout class_select body animation must advance visible pixels").toBeGreaterThan(100)
})

test("fractional-DPR BLU/RED class admission preserves model pixels and three-map replacement", async ({ page }, testInfo) => {
  const failures: string[] = []
  page.on("pageerror", (error) => failures.push(error.message))
  page.on("console", (message) => {
    if (/MissingScenario|TransitionFailed|game-advance|FATAL:/u.test(message.text())) failures.push(message.text())
  })
  await page.addInitScript(() => {
    const commands: Array<{ generation: number; configuration: number; selectors: number }> = []
    ;(globalThis as any).__playsrcAdmissionCommands = commands
    const original = Worker.prototype.postMessage
    Worker.prototype.postMessage = function (message: any, transfer?: any) {
      if (message?.command instanceof ArrayBuffer && message.command.byteLength >= 48) {
        const bytes = new DataView(message.command)
        commands.push({ generation: message.generation, configuration: bytes.getUint32(44, true), selectors: bytes.getUint32(32, true) })
      }
      return original.call(this, message, transfer)
    }
  })
  await page.goto("/", { waitUntil: "domcontentloaded", timeout: 30_000 })
  const main = page.locator("main")
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 60_000 })
  await page.locator(".gameui-layer [data-vgui-name='FindAGameButton']").click()
  await page.locator(".gameui-layer [data-vgui-name='CreateServerEntry'] [data-vgui-name='ModeButton']").click()
  const dialog = page.locator(".local-match-layer").getByRole("dialog", { name: "CREATE SERVER" })
  await dialog.locator("[data-vgui-name='MapList']").click()
  await page.getByRole("option", { name: "pl_upward" }).click()
  await dialog.getByRole("tab", { name: "GAME" }).click()
  await dialog.locator("[data-vgui-name='NumPlayersTextEntry']").fill("5")
  await dialog.getByRole("button", { name: "Start" }).click()

  for (const [index, map] of ["pl_upward", "ctf_2fort", "jump_beef"].entries()) {
    if (index > 0) {
      await page.keyboard.press("Backquote")
      const entry = page.locator("[aria-label='Console command']")
      await entry.fill(`map ${map}`)
      await entry.press("Enter")
    }
    await expect.poll(async () => ({
      phase: await main.getAttribute("data-phase"),
      team: await main.getAttribute("data-team-selection-visible"),
      detail: await main.getAttribute("data-detail"),
    }), { timeout: 45_000 }).toMatchObject({ team: "true" })
    if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await page.locator(`.team-selection-layer [data-vgui-name='teambutton${index === 0 ? 2 : 1}']`).click()
    await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 30_000 })
    if (await main.getAttribute("data-class-selection-visible") === "true") {
      await expect.poll(async () => await main.getAttribute("data-class-selection-models"), { timeout: 15_000 })
        .toMatch(/TFPlayerModel:[^|]+:[0-9]+:[1-9][0-9]*/u)
      const preview = decodeScreenshot(await page.locator("canvas.world-canvas").screenshot())
      let modelPixels = 0
      for (let offset = 0; offset < preview.pixels.length; offset += preview.channels) {
        if (preview.pixels[offset]! > 8 || preview.pixels[offset + 1]! > 8 || preview.pixels[offset + 2]! > 8) modelPixels += 1
      }
      expect(modelPixels, `${map} visible class-selection world/model pixels`).toBeGreaterThan(20_000)
      await page.keyboard.press("Digit2")
    }
    await expect(main).toHaveAttribute("data-class-selection-visible", "false")
    await expect.poll(async () => (await main.getAttribute("data-hud-probe"))?.split(":")[1], { timeout: 10_000 }).toBe("3")
    const pixels = decodeScreenshot(await page.locator("canvas.world-canvas").screenshot())
    let visible = 0
    for (let offset = 0; offset < pixels.pixels.length; offset += pixels.channels) {
      if (pixels.pixels[offset]! > 8 || pixels.pixels[offset + 1]! > 8 || pixels.pixels[offset + 2]! > 8) visible += 1
    }
    expect(visible, `${map} visible world pixels`).toBeGreaterThan(20_000)
    expect(await main.getAttribute("data-hud-probe")).not.toContain("unknown")
    expect(failures).toEqual([])
    if (index === 0) {
      const cadence = await page.evaluate(() => new Promise<{ frames: number; ticks: number }>((resolve) => {
        const root = document.querySelector<HTMLElement>("main")!
        const initialTick = Number(root.dataset.snapshotTick)
        const started = performance.now()
        let frames = 0
        const sample = (now: number) => {
          frames += 1
          if (now - started < 5_000) requestAnimationFrame(sample)
          else resolve({ frames, ticks: Number(root.dataset.snapshotTick) - initialTick })
        }
        requestAnimationFrame(sample)
      }))
      expect(cadence.frames).toBeGreaterThan(100)
      expect(cadence.ticks).toBeGreaterThan(250)
    }
    const generation = Number(await main.getAttribute("data-generation"))
    const commands = await page.evaluate((current) =>
      (globalThis as any).__playsrcAdmissionCommands.filter((command: any) => command.generation === current), generation)
    if (map === "jump_beef") expect(commands.every((command: any) => command.configuration === 0)).toBe(true)
    await testInfo.attach(`admission-${index}-${map}`, {
      body: JSON.stringify({ generation, devicePixelRatio: await page.evaluate(() => devicePixelRatio), visiblePixels: visible, commands: commands.length, failures }),
      contentType: "application/json",
    })
  }
})

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
  const decoded = decodeScreenshot(canvasScreenshot)
  let nonBlack = 0
  for (let index = 0; index < decoded.pixels.length; index += decoded.channels) {
    if (decoded.pixels[index]! > 8 || decoded.pixels[index + 1]! > 8 || decoded.pixels[index + 2]! > 8) nonBlack += 1
  }
  const initial = await page.evaluate(({ classes, canvas }) => ({
    state: { ...document.querySelector<HTMLElement>("main")!.dataset },
    classes: classes.map((name) => {
      const element = document.querySelector<HTMLElement>(`.class-selection-layer [data-vgui-name='${name}']`)
      const image = element?.querySelector<HTMLElement>("[data-vgui-name='SubImage']")
      return { name, visible: Boolean(element && element.getBoundingClientRect().width > 0), image: image?.dataset.vguiImage ?? "", rect: element?.getBoundingClientRect().toJSON() }
    }),
    canvas,
  }), { classes: CLASSES, canvas: { width: decoded.width, height: decoded.height, nonBlack } })
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
  if (await page.locator("main").getAttribute("data-class-selection-visible") !== "true") await page.keyboard.press("Comma")
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-visible", "true")
  await expect(page.locator("main")).toHaveAttribute("data-class-selection-team", "3")
  await page.screenshot({ path: path.join(output, "blue-team-preview.png") })
  const previousClass = (await page.locator("main").getAttribute("data-hud-probe"))?.split(":")[1]
  await page.locator(".class-selection-layer [data-vgui-name='random']").click({ timeout: 5_000 })
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
