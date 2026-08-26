import { createHash } from "node:crypto"
import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import type { Locator, Page } from "@playwright/test"
import { configuredTf2UiResourceInput } from "../../../games/tf2/browser/src/ui-resources/configured.generated"
import type { Tf2UiResourceNode } from "../../../games/tf2/browser/src/ui-resources/types"
import { expect, test } from "./application-test"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"
import { loadLocalConfig } from "../src/config"

type Rectangle = Readonly<{ x: number; y: number; width: number; height: number }>
type Viewport = Readonly<{ width: number; height: number }>
type AuthoredPanel = Readonly<{ name: string; resource: string; parent: string | null; block?: string; nested?: string; match?: boolean }>

const LAYOUT = "scripts/hudlayout.res"
const CLASS = "resource/ui/hudplayerclass.res"
const HEALTH = "resource/ui/hudplayerhealth.res"
const AMMO = "resource/ui/hudammoweapons.res"
const FLAG = "resource/ui/hudobjectiveflagpanel.res"
const FLAG_STATUS = "resource/ui/flagstatus.res"
const MATCH = "resource/ui/hudmatchstatus.res"
const TIME = "resource/ui/hudobjectivetimepanel.res"
const SCOREBOARD = "resource/ui/scoreboard.res"
const RESOURCES = (configuredTf2UiResourceInput as Readonly<{
  resources: readonly Readonly<{ logicalPath: string; document: readonly Tf2UiResourceNode[] | null }>[]
}>).resources
const VIEWPORTS = Object.freeze([
  Object.freeze({ width: 1280, height: 720 }),
  Object.freeze({ width: 1024, height: 768 }),
  Object.freeze({ width: 1600, height: 900 }),
] satisfies readonly Viewport[])
const CLASSES = Object.freeze([
  ["scout", 1, "scout"], ["sniper", 2, "sniper"], ["soldier", 3, "soldier"],
  ["demoman", 4, "demo"], ["medic", 5, "medic"], ["heavyweapons", 6, "heavy"],
  ["pyro", 7, "pyro"], ["spy", 8, "spy"], ["engineer", 9, "engi"],
] as const)
const COMMON_PANELS = Object.freeze([
  { name: "HudPlayerStatus", resource: LAYOUT, parent: null },
  { name: "HudPlayerClass", resource: CLASS, parent: "HudPlayerStatus" },
  { name: "PlayerStatusClassImage", resource: CLASS, parent: "HudPlayerClass" },
  { name: "PlayerStatusClassImageBG", resource: CLASS, parent: "HudPlayerClass" },
  { name: "classmodelpanel", resource: CLASS, parent: "HudPlayerClass" },
  { name: "classmodelpanelBG", resource: CLASS, parent: "HudPlayerClass" },
  { name: "HudPlayerHealth", resource: HEALTH, parent: "HudPlayerStatus" },
  { name: "PlayerStatusHealthImage", resource: HEALTH, parent: "HudPlayerHealth" },
  { name: "PlayerStatusHealthValue", resource: HEALTH, parent: "HudPlayerHealth" },
  { name: "HudWeaponAmmo", resource: LAYOUT, parent: null },
  { name: "HudWeaponAmmoBG", resource: AMMO, parent: "HudWeaponAmmo" },
  { name: "AmmoInClip", resource: AMMO, parent: "HudWeaponAmmo" },
  { name: "AmmoInClipShadow", resource: AMMO, parent: "HudWeaponAmmo" },
  { name: "AmmoInReserve", resource: AMMO, parent: "HudWeaponAmmo" },
  { name: "scoreinfo", resource: SCOREBOARD, block: "scores", parent: null },
  { name: "BluePlayerList", resource: SCOREBOARD, parent: "scoreinfo" },
  { name: "RedPlayerList", resource: SCOREBOARD, parent: "scoreinfo" },
] satisfies readonly AuthoredPanel[])
const CTF_PANELS = Object.freeze([
  { name: "HudObjectiveStatus", resource: LAYOUT, parent: null },
  { name: "ObjectiveStatusFlagPanel", resource: FLAG, parent: "HudObjectiveStatus" },
  ...["BlueFlag", "RedFlag", "BlueScore", "BlueScoreShadow", "RedScore", "RedScoreShadow", "PlayingTo", "PlayingToBG"].map((name) =>
    Object.freeze({ name, resource: FLAG, parent: "ObjectiveStatusFlagPanel" })),
] satisfies readonly AuthoredPanel[])
const ROUND_PANELS = Object.freeze([
  { name: "HudMatchStatus", resource: LAYOUT, parent: null },
  { name: "WaitingForPlayersPanel", resource: LAYOUT, parent: null },
  { name: "BGFrame", resource: MATCH, parent: "HudMatchStatus", match: true },
  { name: "ObjectiveStatusTimePanel", resource: MATCH, parent: "HudMatchStatus", match: true },
  { name: "TimePanelValue", resource: MATCH, nested: "ObjectiveStatusTimePanel", parent: "ObjectiveStatusTimePanel", match: true },
  { name: "TimePanelBG", resource: TIME, parent: "ObjectiveStatusTimePanel", match: true },
  { name: "WaitingForPlayersLabel", resource: TIME, parent: "ObjectiveStatusTimePanel", match: true },
] satisfies readonly AuthoredPanel[])

const scaled = (value: number, viewport: Viewport): number => Math.trunc(value * viewport.height / 480)
const same = (left: string, right: string): boolean => left.toLowerCase() === right.toLowerCase()
const active = (node: Tf2UiResourceNode): boolean => {
  if (!node.condition) return true
  const symbol = node.condition.symbol.replace(/^\$/u, "").toUpperCase()
  const selected = symbol === "WIN32" || symbol === "OSX" || symbol === "POSIX"
  return node.condition.negated ? !selected : selected
}

function authored(definition: AuthoredPanel): ReadonlyMap<string, string> {
  const source = RESOURCES.find((panel) => panel.logicalPath === definition.resource)
  const owner = definition.nested
    ? source?.document?.[0]?.children.find((panel) => same(panel.name, definition.nested!))
    : source?.document?.[0]
  const block = owner?.children.find((panel) => same(panel.name, definition.block ?? definition.name))
  if (!block) throw new Error(`Authored TF2 HUD block is unavailable: ${definition.resource}:${definition.name}`)
  const result = new Map<string, string>()
  for (const property of block.children) {
    if (property.value === null || !active(property) || /_(?:hidef|lodef|minmode)$/iu.test(property.name)) continue
    const name = property.name.toLowerCase()
    if (!result.has(name)) result.set(name, property.value)
  }
  if (definition.match) {
    const condition = block.children.find((property) => property.value === null && same(property.name, "if_match"))
    for (const property of condition?.children ?? []) {
      if (property.value !== null && active(property) && !/_(?:hidef|lodef|minmode)$/iu.test(property.name)) {
        result.set(property.name.toLowerCase(), property.value)
      }
    }
  }
  return result
}

function position(value: string, own: number, parent: number, viewport: Viewport): number {
  let cursor = 0
  const term = (): number => {
    const alignment = /^[rc]/iu.test(value[cursor] ?? "") ? value[cursor++]!.toLowerCase() : ""
    const proportion = /^[sp]/iu.test(value[cursor] ?? "") ? value[cursor++]!.toLowerCase() : ""
    const number = /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)/u.exec(value.slice(cursor))
    if (!number) throw new Error(`Authored TF2 HUD coordinate is invalid: ${value}`)
    cursor += number[0].length
    const amount = Number(number[0])
    const offset = proportion === "s" ? Math.trunc(own * amount)
      : proportion === "p" ? Math.trunc(parent * amount) : scaled(Math.trunc(amount), viewport)
    return alignment === "r" ? parent - offset : alignment === "c" ? Math.trunc(parent / 2) + offset : offset
  }
  let result = term()
  while (cursor < value.length) {
    const operation = value[cursor++]
    if (operation !== "+" && operation !== "-") throw new Error(`Authored TF2 HUD coordinate operator is invalid: ${value}`)
    const offset = term()
    result = operation === "+" ? result + offset : result - offset
  }
  return result
}

function expected(definition: AuthoredPanel, viewport: Viewport, parents: ReadonlyMap<string, Rectangle>): Rectangle {
  const properties = authored(definition)
  const useParent = properties.get("proportionaltoparent") === "1"
  const parent = useParent && definition.parent ? parents.get(definition.parent) : undefined
  const parentWidth = parent?.width ?? viewport.width
  const parentHeight = parent?.height ?? viewport.height
  const dimension = (value: string | undefined, available: number): number => {
    if (!value) throw new Error(`Authored TF2 HUD dimension is unavailable: ${definition.name}`)
    if (/^f/iu.test(value)) return available - scaled(Number(value.slice(1)), viewport)
    if (/^p/iu.test(value)) {
      const proportion = Number(value.slice(1))
      return Math.trunc((available - scaled(Math.trunc(proportion), viewport)) * proportion)
    }
    return scaled(Number(value), viewport)
  }
  const width = dimension(properties.get("wide"), parentWidth)
  const height = dimension(properties.get("tall"), parentHeight)
  return Object.freeze({
    x: position(properties.get("xpos") ?? "0", width, parentWidth, viewport),
    y: position(properties.get("ypos") ?? "0", height, parentHeight, viewport),
    width,
    height,
  })
}

function overlaps(left: Rectangle, right: Rectangle): boolean {
  return left.x < right.x + right.width && right.x < left.x + left.width
    && left.y < right.y + right.height && right.y < left.y + left.height
}

async function settle(page: Page): Promise<void> {
  const before = Number(await page.locator("main").getAttribute("data-display-frame") ?? 0)
  await page.waitForFunction((frame) => Number(document.querySelector<HTMLElement>("main")?.dataset.displayFrame ?? 0) > frame, before)
  await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))))
}

async function submit(page: Page, entry: Locator, command: string): Promise<void> {
  if (await page.locator("main").getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
  await expect(entry).toBeVisible()
  await entry.fill(command)
  await entry.press("Enter")
}

async function worldPixels(page: Page): Promise<Buffer> {
  await page.evaluate(() => {
    for (const element of document.querySelectorAll<HTMLElement>(".hud-layer, .engineer-layer")) {
      element.dataset.hudEvidenceDisplay = element.style.display
      element.style.display = "none"
    }
  })
  try {
    return await page.locator(".world-canvas").screenshot()
  } finally {
    await page.evaluate(() => {
      for (const element of document.querySelectorAll<HTMLElement>("[data-hud-evidence-display]")) {
        element.style.display = element.dataset.hudEvidenceDisplay ?? ""
        delete element.dataset.hudEvidenceDisplay
      }
    })
  }
}

async function hudGeometry(page: Page, viewport: Viewport, definitions: readonly AuthoredPanel[]) {
  const parents = new Map<string, Rectangle>()
  const wanted = definitions.map((definition) => {
    const selected = { ...definition, local: expected(definition, viewport, parents) }
    parents.set(definition.name, selected.local)
    return selected
  })
  const measured = await page.evaluate((panels) => {
    const hosts = document.querySelectorAll<HTMLElement>("[data-vgui-runtime='tf2-hud']")
    if (hosts.length !== 1) throw new Error(`TF2 HUD owns ${hosts.length} runtime roots`)
    const read = (element: Element) => {
      const value = element.getBoundingClientRect()
      return { x: value.x, y: value.y, width: value.width, height: value.height }
    }
    const host = hosts[0]!
    const values: Record<string, { local: Rectangle; absolute: Rectangle; visible: boolean; count: number; text: string; z: number; fontSize: number }> = {}
    for (const panel of panels) {
      const matches = host.querySelectorAll<HTMLElement>(`[data-vgui-name="${panel.name}"]`)
      const scoped = [...matches].filter((candidate) => candidate.parentElement?.dataset.vguiName === (panel.parent ?? "HudViewport"))
      const element = scoped[0]
      if (!element) throw new Error(`TF2 HUD panel is unavailable: ${panel.parent ?? "HudViewport"}/${panel.name}`)
      const style = getComputedStyle(element)
      values[panel.name] = {
        local: { x: Number.parseFloat(element.style.left), y: Number.parseFloat(element.style.top),
          width: Number.parseFloat(element.style.width), height: Number.parseFloat(element.style.height) },
        absolute: read(element),
        visible: style.display !== "none" && style.visibility !== "hidden",
        count: scoped.length,
        text: element.textContent ?? "",
        z: Number(style.zIndex),
        fontSize: Number.parseFloat(style.fontSize),
      }
    }
    const viewportPanel = host.querySelector<HTMLElement>("[data-vgui-name='HudViewport']")
    if (!viewportPanel) throw new Error("TF2 HUD viewport panel is unavailable")
    const crosshair = document.querySelector<HTMLElement>("[data-tf2-crosshair='authored']")
    return { host: read(host), viewport: read(viewportPanel), panels: values,
      crosshair: crosshair ? { bounds: read(crosshair), visible: getComputedStyle(crosshair).display !== "none" } : null }
  }, wanted)
  expect(measured.host).toEqual({ x: 0, y: 0, ...viewport })
  expect(measured.viewport).toEqual({ x: 0, y: 0, ...viewport })
  for (const panel of wanted) {
    const actual = measured.panels[panel.name]!
    expect(actual.local, `${panel.name} at ${viewport.width}x${viewport.height}`).toEqual(panel.local)
    expect(actual.count, `${panel.name} duplicate roots`).toBe(1)
  }
  expect(measured.panels.PlayerStatusHealthValue?.fontSize).toBe(scaled(16, viewport))
  expect(measured.panels.AmmoInClip?.fontSize).toBe(scaled(44, viewport))
  if (measured.panels.BlueScore) expect(measured.panels.BlueScore.fontSize).toBe(scaled(36, viewport))
  if (measured.crosshair?.visible) {
    expect(measured.crosshair.bounds.x + measured.crosshair.bounds.width / 2).toBe(viewport.width / 2)
    expect(measured.crosshair.bounds.y + measured.crosshair.bounds.height / 2).toBe(viewport.height / 2)
  }
  return measured
}

async function authoredImagePixels(page: Page, selector: string) {
  const panel = page.locator(selector)
  await expect(panel).toBeVisible()
  const bytes = await panel.screenshot({ animations: "disabled" })
  const result = await panel.evaluate(async (element, input) => {
    const match = getComputedStyle(element).backgroundImage.match(/^url\("?(blob:[^"\)]+)"?\)$/u)
    if (!match) throw new Error(`Authored TF2 HUD texture is not directly visible: ${element.dataset.vguiName}`)
    const [screen, source] = await Promise.all([
      createImageBitmap(new Blob([Uint8Array.from(input)], { type: "image/png" })),
      createImageBitmap(await (await fetch(match[1]!)).blob()),
    ])
    const canvas = document.createElement("canvas")
    canvas.width = screen.width
    canvas.height = screen.height
    const context = canvas.getContext("2d", { willReadFrequently: true })
    if (!context) throw new Error("Authored TF2 HUD pixel evidence is unavailable")
    context.drawImage(screen, 0, 0)
    const observed = context.getImageData(0, 0, canvas.width, canvas.height).data
    context.clearRect(0, 0, canvas.width, canvas.height)
    context.drawImage(source, 0, 0, canvas.width, canvas.height)
    const expected = context.getImageData(0, 0, canvas.width, canvas.height).data
    let opaque = 0
    let matching = 0
    for (let index = 0; index < expected.length; index += 4) {
      if (expected[index + 3]! !== 255) continue
      opaque += 1
      if (Math.max(Math.abs(observed[index]! - expected[index]!), Math.abs(observed[index + 1]! - expected[index + 1]!),
        Math.abs(observed[index + 2]! - expected[index + 2]!)) <= 16) matching += 1
    }
    screen.close()
    source.close()
    return { width: canvas.width, height: canvas.height, opaque, matching }
  }, [...bytes])
  expect(result.opaque).toBeGreaterThan(50)
  expect(result.matching / result.opaque).toBeGreaterThan(0.55)
  return { ...result, sha256: createHash("sha256").update(bytes).digest("hex") }
}

function changedRegion(before: Buffer, after: Buffer, bounds: Rectangle): number {
  const left = decodeScreenshot(before)
  const right = decodeScreenshot(after)
  if (left.width !== right.width || left.height !== right.height) throw new Error("TF2 HUD model comparisons use different viewports")
  let changed = 0
  for (let y = Math.max(0, bounds.y); y < Math.min(left.height, bounds.y + bounds.height); y += 1) {
    for (let x = Math.max(0, bounds.x); x < Math.min(left.width, bounds.x + bounds.width); x += 1) {
      const a = (y * left.width + x) * left.channels
      const b = (y * right.width + x) * right.channels
      if (Math.abs(left.pixels[a]! - right.pixels[b]!) + Math.abs(left.pixels[a + 1]! - right.pixels[b + 1]!)
        + Math.abs(left.pixels[a + 2]! - right.pixels[b + 2]!) > 36) changed += 1
    }
  }
  return changed
}

test("headed Source desktop HUD retains authored positions, pixels, class lifecycle, and real-time cadence on all maps", async ({ page }, testInfo) => {
  const local = await loadLocalConfig()
  const directory = path.join(local.sourceCacheDir, "profiles", "hud", "three-map")
  await mkdir(directory, { recursive: true })
  await page.goto("/", { waitUntil: "domcontentloaded", timeout: 30_000 })
  const main = page.locator("main")
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 180_000 })
  const entry = page.locator("[aria-label='Console command']")
  const captures: Record<string, unknown>[] = []

  const load = async (target: string, team: "red" | "blue", captureMenus = false): Promise<void> => {
    await submit(page, entry, `map ${target}`)
    await page.waitForFunction((name) => {
      const root = document.querySelector<HTMLElement>("main")
      return root?.dataset.teamSelectionVisible === "true" || root?.dataset.phase === "Failed"
        || root?.dataset.phase === "Ready" && root.dataset.detail === `Playing ${name}`
    }, target, { timeout: 600_000, polling: 25 })
    await page.waitForFunction(() => {
      const root = document.querySelector<HTMLElement>("main")
      if (!root || root.dataset.phase === "Failed") return true
      if (root.dataset.teamSelectionVisible === "true" || root.dataset.classSelectionVisible === "true") return true
      const player = document.querySelector<HTMLElement>(".hud-layer [data-vgui-name='HudPlayerStatus']")
      return root.dataset.phase === "Ready" && Boolean(player && getComputedStyle(player).display !== "none")
    }, undefined, { timeout: 30_000 })
    if (await main.getAttribute("data-team-selection-visible") === "true") {
      if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
      const selected = page.locator(`.team-selection-layer [data-vgui-name='${team === "red" ? "teambutton1" : "teambutton0"}']`)
      await expect(selected).toBeVisible()
      if (captureMenus) {
        const menu = await page.screenshot({ path: path.join(directory, "ctf-2fort-team-selection.png") })
        await testInfo.attach("headed-desktop-team-selection", { body: menu, contentType: "image/png" })
        const count = page.locator(".team-selection-layer [data-vgui-name='BlueCount']")
        expect(Number.parseFloat(await count.evaluate((element) => element.style.top))).toBe(scaled(53, VIEWPORTS[0]!))
      }
      await selected.click()
    }
    await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 600_000 })
    await page.waitForFunction(() => {
      const root = document.querySelector<HTMLElement>("main")
      if (!root || root.dataset.phase === "Failed") return true
      if (root.dataset.classSelectionVisible === "true") return true
      const player = document.querySelector<HTMLElement>(".hud-layer [data-vgui-name='HudPlayerStatus']")
      return Boolean(player && getComputedStyle(player).display !== "none")
    }, undefined, { timeout: 30_000 })
    if (await main.getAttribute("data-class-selection-visible") === "true") {
      await expect(page.locator(".hud-layer")).toBeHidden()
      if (captureMenus) {
        const menu = await page.screenshot({ path: path.join(directory, "ctf-2fort-class-selection.png") })
        await testInfo.attach("headed-desktop-class-selection", { body: menu, contentType: "image/png" })
        expect(Number.parseFloat(await page.locator(".class-selection-layer [data-vgui-name='scout']").evaluate((element) => element.style.top)))
          .toBe(scaled(-5, VIEWPORTS[0]!))
      }
      await page.keyboard.press("Digit2")
      await expect(main).toHaveAttribute("data-class-selection-visible", "false")
    }
    if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await settle(page)
  }

  await load("ctf_2fort", "red", true)
  await expect(main).toHaveAttribute("data-ctf", /^0:0:3:0:/u)
  let ctf = await hudGeometry(page, VIEWPORTS[0]!, [...COMMON_PANELS, ...CTF_PANELS])
  expect(ctf.panels.classmodelpanel!.visible).toBe(true)
  const withModel = await worldPixels(page)
  await submit(page, entry, "cl_hud_playerclass_use_playermodel 0")
  await page.keyboard.press("Backquote")
  await expect(page.locator(".hud-layer [data-vgui-name='PlayerStatusClassImage']")).toBeVisible()
  await settle(page)
  const withoutModel = await worldPixels(page)
  const modelPixels = changedRegion(withModel, withoutModel, ctf.panels.classmodelpanel!.local)
  expect(modelPixels, "default authored 3D class portrait pixels").toBeGreaterThan(150)
  const portrait = await authoredImagePixels(page, ".hud-layer [data-vgui-name='PlayerStatusClassImage']")
  const flagIcons = await Promise.all(["RedFlag", "BlueFlag"].map((name) =>
    authoredImagePixels(page, `.hud-layer [data-vgui-name='${name}'] [data-vgui-name='StatusIcon']`)))
  const leftIcon = page.locator(".hud-layer [data-vgui-name='BlueFlag'] [data-vgui-name='StatusIcon']")
  const rightIcon = page.locator(".hud-layer [data-vgui-name='RedFlag'] [data-vgui-name='StatusIcon']")
  const iconBounds = await Promise.all([leftIcon, rightIcon].map(async (icon) => {
    const value = await icon.boundingBox()
    if (!value) throw new Error("Authored TF2 intelligence icon has no visible browser bounds")
    return value
  }))
  expect(overlaps(iconBounds[0]!, iconBounds[1]!)).toBe(false)
  expect(overlaps(ctf.panels.BlueScore!.absolute, ctf.panels.RedScore!.absolute)).toBe(false)
  expect(ctf.panels.BlueScore!.z).toBeGreaterThan(ctf.panels.PlayingToBG!.z)
  expect(ctf.panels.BlueScore!.local.y + ctf.panels.BlueScore!.local.height).toBeLessThanOrEqual(VIEWPORTS[0]!.height)
  await page.screenshot({ path: path.join(directory, "ctf-2fort-red-desktop.png") })

  for (const viewport of VIEWPORTS.slice(1)) {
    await page.setViewportSize(viewport)
    await settle(page)
    ctf = await hudGeometry(page, viewport, [...COMMON_PANELS, ...CTF_PANELS])
    captures.push({ map: "ctf_2fort", viewport, flags: [ctf.panels.BlueFlag!.local, ctf.panels.RedFlag!.local],
      scores: [ctf.panels.BlueScore!.local, ctf.panels.RedScore!.local], playingTo: ctf.panels.PlayingTo!.local })
    await page.screenshot({ path: path.join(directory, `ctf-2fort-${viewport.width}x${viewport.height}.png`) })
  }
  await page.setViewportSize(VIEWPORTS[0]!)
  await settle(page)

  for (const team of ["red", "blue"] as const) {
    if (team === "blue") {
      await submit(page, entry, "jointeam spectate")
      await expect.poll(async () => JSON.parse(await main.getAttribute("data-scoreboard-probe") ?? "{}").spectators).toContain("unnamed")
      await page.keyboard.press("Backquote")
      await expect(page.locator(".hud-layer [data-vgui-name='HudPlayerStatus']")).toBeHidden()
      await expect(page.locator(".hud-layer [data-vgui-name='HudWeaponAmmo']")).toBeHidden()
      await page.keyboard.down("Tab")
      await expect(page.locator(".hud-layer [data-vgui-name='scoreinfo']")).toBeVisible()
      await expect(page.locator(".hud-layer [data-vgui-name='Spectators']")).toContainText("unnamed")
      await page.keyboard.up("Tab")
      await submit(page, entry, "jointeam blue")
      await page.keyboard.press("Backquote")
      await page.waitForFunction(() => {
        const root = document.querySelector<HTMLElement>("main")
        if (!root || root.dataset.teamSelectionLocal !== "3") return false
        if (root.dataset.classSelectionVisible === "true") return true
        try { return JSON.parse(root.dataset.hudPresentationProbe ?? "{}").classModel?.scalars?.team === 3 }
        catch { return false }
      })
      if (await main.getAttribute("data-class-selection-visible") === "true") await page.keyboard.press("Digit2")
      await expect(page.locator(".hud-layer [data-vgui-name='HudPlayerStatus']")).toBeVisible()
    }
    for (const [name, identity, image] of CLASSES) {
      await submit(page, entry, `joinclass ${name}`)
      await expect.poll(async () => (await main.getAttribute("data-hud-probe"))?.split(":")[1]).toBe(String(identity))
      const presentation = JSON.parse(await main.getAttribute("data-hud-presentation-probe") ?? "{}")
      expect(presentation.classImage).toMatchObject({ visible: true, image: `../hud/class_${image}${team === "red" ? "red" : "blue"}` })
      expect(presentation.roots).toEqual({ playerStatus: 1, ammo: 1 })
    }
    if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  }
  await page.keyboard.press("Escape")
  await expect(main).toHaveAttribute("data-gameui", "pause")
  expect(await page.locator("[data-vgui-runtime='tf2-hud']").count()).toBe(1)
  await page.keyboard.press("Escape")
  await expect(main).toHaveAttribute("data-gameui", "in-game")

  await submit(page, entry, "cl_hud_playerclass_use_playermodel 1")
  await page.keyboard.press("Backquote")
  await expect(page.locator(".hud-layer [data-vgui-name='HudPlayerClass'] > [data-vgui-name='classmodelpanel']")).toBeVisible()
  await settle(page)
  const blu = await page.screenshot({ path: path.join(directory, "ctf-2fort-blue-model.png") })
  await testInfo.attach("headed-ctf-authored-intelligence-and-class-model", { body: blu, contentType: "image/png" })

  await load("pl_upward", "blue")
  let upward = await hudGeometry(page, VIEWPORTS[0]!, [...COMMON_PANELS, ...ROUND_PANELS])
  if (upward.panels.WaitingForPlayersPanel!.visible) {
    await expect(page.locator(".hud-layer [data-vgui-name='WaitingForPlayersPanel']")).toContainText(/Waiting/u)
    await page.waitForFunction(() => {
      const values = document.querySelector<HTMLElement>("main")?.dataset.roundProbe?.split(":")
      return values?.[1] === "0"
    }, undefined, { timeout: 45_000 })
    await settle(page)
    upward = await hudGeometry(page, VIEWPORTS[0]!, [...COMMON_PANELS, ...ROUND_PANELS])
  }
  expect(upward.panels.HudPlayerStatus!.visible).toBe(true)
  expect(upward.panels.HudWeaponAmmo!.visible).toBe(true)
  expect(upward.crosshair?.visible).toBe(true)
  expect(upward.panels.ObjectiveStatusTimePanel!.visible).toBe(true)
  expect(upward.panels.BGFrame!.visible).toBe(true)
  expect(upward.panels.TimePanelBG!.visible).toBe(false)
  expect(upward.panels.WaitingForPlayersPanel!.visible).toBe(false)
  expect(upward.panels.TimePanelValue!.text).toMatch(/^\d+:\d{2}$/u)
  const upwardCapture = await page.screenshot({ path: path.join(directory, "pl-upward-round-hud.png") })
  await testInfo.attach("headed-payload-authored-round-hud", { body: upwardCapture, contentType: "image/png" })
  await page.keyboard.down("Tab")
  await expect(page.locator(".hud-layer [data-vgui-name='scoreinfo']")).toBeVisible()
  const scoreboard = await hudGeometry(page, VIEWPORTS[0]!, COMMON_PANELS)
  expect(overlaps(scoreboard.panels.BluePlayerList!.absolute, scoreboard.panels.RedPlayerList!.absolute)).toBe(false)
  await page.keyboard.up("Tab")
  captures.push({ map: "pl_upward", viewport: VIEWPORTS[0], timer: upward.panels.ObjectiveStatusTimePanel!.local,
    value: upward.panels.TimePanelValue!.local, scoreboard: scoreboard.panels.scoreinfo!.local })
  await page.setViewportSize(VIEWPORTS[1]!)
  await settle(page)
  const upwardWindow = await hudGeometry(page, VIEWPORTS[1]!, [...COMMON_PANELS, ...ROUND_PANELS])
  captures.push({ map: "pl_upward", viewport: VIEWPORTS[1], timer: upwardWindow.panels.ObjectiveStatusTimePanel!.local })
  await page.screenshot({ path: path.join(directory, "pl-upward-1024x768.png") })
  await page.setViewportSize(VIEWPORTS[0]!)
  await settle(page)

  const origin = new URL(page.url()).origin
  await page.goto("about:blank", { waitUntil: "domcontentloaded" })
  const storage = await page.context().newCDPSession(page)
  await storage.send("Storage.clearDataForOrigin", { origin, storageTypes: "indexeddb" })
  await storage.detach()
  await page.goto("/", { waitUntil: "domcontentloaded", timeout: 30_000 })
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 180_000 })
  await load("jump_beef", "red")
  const jump = await hudGeometry(page, VIEWPORTS[0]!, COMMON_PANELS)
  expect(jump.crosshair?.visible).toBe(true)
  const jumpComposed = await page.screenshot({ path: path.join(directory, "jump-beef-hud.png") })
  const jumpWorld = await worldPixels(page)
  const visiblePixels = {
    health: changedRegion(jumpComposed, jumpWorld, jump.panels.PlayerStatusHealthImage!.absolute),
    ammo: changedRegion(jumpComposed, jumpWorld, jump.panels.HudWeaponAmmo!.absolute),
    crosshair: changedRegion(jumpComposed, jumpWorld, jump.crosshair!.bounds),
  }
  expect(visiblePixels.health).toBeGreaterThan(100)
  expect(visiblePixels.ammo).toBeGreaterThan(100)
  expect(visiblePixels.crosshair).toBeGreaterThan(4)
  expect(overlaps(jump.panels.PlayerStatusHealthImage!.absolute, jump.panels.HudWeaponAmmo!.absolute)).toBe(false)
  captures.push({ map: "jump_beef", viewport: VIEWPORTS[0], health: jump.panels.PlayerStatusHealthImage!.absolute,
    classModel: jump.panels.classmodelpanel!.local, ammo: jump.panels.HudWeaponAmmo!.local, crosshair: jump.crosshair })
  await page.setViewportSize(VIEWPORTS[2]!)
  await settle(page)
  const jumpWindow = await hudGeometry(page, VIEWPORTS[2]!, COMMON_PANELS)
  captures.push({ map: "jump_beef", viewport: VIEWPORTS[2], health: jumpWindow.panels.PlayerStatusHealthImage!.absolute,
    ammo: jumpWindow.panels.HudWeaponAmmo!.local, crosshair: jumpWindow.crosshair })
  await page.screenshot({ path: path.join(directory, "jump-beef-1600x900.png") })
  await page.setViewportSize(VIEWPORTS[0]!)
  await settle(page)

  const seconds = profileSampleSeconds()
  const cadence = await page.evaluate(async (duration) => {
    const root = document.querySelector<HTMLElement>("main")!
    const started = performance.now()
    const firstTick = Number(root.dataset.snapshotTick)
    let previous = started
    const frames: number[] = []
    const hudModels: number[] = []
    await new Promise<void>((resolve) => {
      const sample = (now: number): void => {
        frames.push(now - previous)
        previous = now
        const detail = JSON.parse(root.dataset.performanceDetail ?? "{}") as { hudModel?: number }
        if (typeof detail.hudModel === "number") hudModels.push(detail.hudModel)
        if (now - started >= duration * 1000) resolve()
        else requestAnimationFrame(sample)
      }
      requestAnimationFrame(sample)
    })
    return { seconds: (performance.now() - started) / 1000, ticks: Number(root.dataset.snapshotTick) - firstTick, frames, hudModels }
  }, seconds)
  expect(cadence.ticks / cadence.seconds).toBeGreaterThan(60)
  expect(cadence.frames.length / cadence.seconds).toBeGreaterThan(45)
  expect(cadence.hudModels.some((value) => value > 0)).toBe(true)

  const report = Object.freeze({
    schema: "playsrc-tf2-headed-authored-desktop-hud-v3",
    headed: true,
    maps: Object.freeze(["ctf_2fort", "pl_upward", "jump_beef"]),
    viewports: VIEWPORTS,
    teams: Object.freeze(["red", "blue"]),
    classes: CLASSES.map(([, identity]) => identity),
    modelPixels,
    portrait,
    visiblePixels,
    intelligenceIcons: flagIcons,
    captures,
    simulationHz: Number((cadence.ticks / cadence.seconds).toFixed(2)),
    frames: summarizeFrameTimes(cadence.frames),
    hudModel: summarizeFrameTimes(cadence.hudModels),
  })
  const serialized = `${JSON.stringify(report, null, 2)}\n`
  await writeFile(path.join(directory, "report.json"), serialized)
  await testInfo.attach("headed-three-map-authored-hud", { body: Buffer.from(serialized), contentType: "application/json" })
  console.log(`PLAYSRC_HUD_PROFILE ${JSON.stringify(report)}`)
})
