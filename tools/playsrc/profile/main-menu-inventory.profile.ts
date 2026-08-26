import { createHash } from "node:crypto"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
import { expect, test } from "./application-test"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"

type AuthoredNode = Readonly<{ name: string; value: string | null; children: readonly AuthoredNode[] }>
type AuthoredResource = Readonly<{
  domain: string
  logicalPath: string
  sha256: string
  document: readonly AuthoredNode[] | null
}>
type AuthoredResources = Readonly<{ contentBuild: string; resources: readonly AuthoredResource[] }>

const scalar = (node: AuthoredNode, name: string): string | null =>
  node.children.find((property) => property.name.toLowerCase() === name.toLowerCase() && property.value !== null)?.value ?? null

async function authoredResources(): Promise<AuthoredResources> {
  const generated = await readFile(new URL("../../../games/tf2/browser/src/ui-resources/configured.generated.ts", import.meta.url), "utf8")
  const prefix = "export const configuredTf2UiResourceInput: unknown = "
  const start = generated.indexOf(prefix)
  if (start < 0) throw new Error("TF2 configured authored-resource ledger is unavailable")
  return JSON.parse(generated.slice(start + prefix.length)) as AuthoredResources
}

function localized(resources: AuthoredResources, token: string): string {
  const name = token.replace(/^#/u, "").toLowerCase()
  let value: string | null = null
  for (const resource of resources.resources.filter((source) => source.domain === "localization")) {
    for (const root of resource.document ?? []) {
      const tokens = root.children.find((child) => child.name.toLowerCase() === "tokens")
      const selected = tokens?.children.find((child) => child.name.toLowerCase() === name && child.value !== null)
      if (selected?.value) value = selected.value
    }
  }
  if (!value) throw new Error(`TF2 authored localization token is unavailable: ${token}`)
  return value
}

const CONDITIONAL_MAIN_MENU_CONTROLS = Object.freeze({
  coordinator: ["RankPanel", "RankModelPanel", "CycleRankTypeButton", "RankTooltipPanel", "NoGCMessage", "NoGCImage", "RankBorder"],
  notifications: ["Notifications_ShowButtonPanel", "Notifications_Panel"],
  messageOfTheDay: ["MOTD_ShowButtonPanel", "MOTD_Panel"],
  disabledClientFeatures: ["WatchStreamButton", "QuestLogButton"],
  promotions: ["EventPromo", "ShowPromoCodesButton", "StoreHasNewItemsImage"],
  virtualReality: ["VRBGPanel", "VRModeButton"],
  alternativeFooterAndInternalRendering: ["SettingsButtonSDK", "TF2SettingsButtonSDK", "icon_generator"],
})

const CORE_FOOTER_CONTROLS = Object.freeze([
  "CharacterSetupButton", "GeneralStoreButton", "SettingsButton", "TF2SettingsButton", "NewUserForumsButton",
  "AchievementsButton", "CommentaryButton", "CoachPlayersButton", "WorkshopButton", "ReplayButton", "ReportBugButton",
])

const UNSUPPORTED_CORE_CONTROLS = Object.freeze([
  "CharacterSetupButton", "GeneralStoreButton", "AchievementsButton", "CommentaryButton", "CoachPlayersButton",
  "WorkshopButton", "ReplayButton", "ReportBugButton", "ToggleChatButton",
])

type Rectangle = Readonly<{ x: number; y: number; width: number; height: number }>

function screenRegion(bytes: Buffer, bounds: Rectangle) {
  const screenshot = decodeScreenshot(bytes)
  const x0 = Math.max(0, Math.floor(bounds.x))
  const y0 = Math.max(0, Math.floor(bounds.y))
  const x1 = Math.min(screenshot.width, Math.ceil(bounds.x + bounds.width))
  const y1 = Math.min(screenshot.height, Math.ceil(bounds.y + bounds.height))
  const colors = new Set<string>()
  let samples = 0
  let nonBlack = 0
  for (let y = y0; y < y1; y += 2) for (let x = x0; x < x1; x += 2) {
    const offset = (y * screenshot.width + x) * screenshot.channels
    const red = screenshot.pixels[offset]!
    const green = screenshot.pixels[offset + 1]!
    const blue = screenshot.pixels[offset + 2]!
    if (colors.size < 4_096) colors.add(`${red},${green},${blue}`)
    if (red > 24 || green > 24 || blue > 24) nonBlack += 1
    samples += 1
  }
  return Object.freeze({ bounds, samples, nonBlack, colors: colors.size })
}

test("headed TF2 Main Menu retains every authored core control, party portrait, playlist image, and local map entrypoint", async ({ page }, testInfo) => {
  const main = page.locator("main")
  const gameUi = page.locator(".gameui-layer")
  await page.goto("/")
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 180_000 })
  await expect(main).toHaveAttribute("data-startup-state", "Skipped")

  const resources = await authoredResources()
  const resource = (identity: string) => {
    const selected = resources.resources.find((source) => source.logicalPath === identity)
    if (!selected?.document?.[0]) throw new Error(`TF2 authored Main Menu resource is unavailable: ${identity}`)
    return { source: selected, root: selected.document[0] }
  }
  const menuResource = resource("resource/ui/mainmenuoverride.res")
  const gameMenuResource = resource("resource/gamemenu.res")
  const dashboardResource = resource("resource/ui/matchmakingdashboard.res")
  const playlistResource = resource("resource/ui/matchmakingplaylist.res")
  const conditional = new Set(Object.values(CONDITIONAL_MAIN_MENU_CONTROLS).flat())
  const sessionConditional = new Set(gameMenuResource.root.children.filter((node) =>
    scalar(node, "OnlyInGame") === "1" || scalar(node, "OnlyInReplay") === "1" || scalar(node, "OnlyWhenVREnabled") === "1")
    .map((node) => node.name))
  const authored = menuResource.root.children.filter((node) => node.value === null && node.name !== "MainMenuOverride")
    .map((node) => Object.freeze({
      name: scalar(node, "fieldName") ?? node.name,
      authoredVisible: scalar(node, "visible") !== "0",
    }))

  const actual = await page.evaluate(() => {
    const owner = document.querySelector<HTMLElement>('.gameui-layer [data-vgui-name="MainMenuOverride"]')
    if (!owner) throw new Error("TF2 authored Main Menu owner is unavailable")
    return [...owner.children]
      .filter((element): element is HTMLElement => element instanceof HTMLElement && element.hasAttribute("data-vgui-panel"))
      .map((element) => {
        const bounds = element.getBoundingClientRect()
        return {
          name: element.dataset.vguiName!,
          visible: getComputedStyle(element).display !== "none"
            && element.getAttribute("aria-hidden") === "false"
            && !element.hidden,
          disabled: element.getAttribute("aria-disabled") === "true",
          bounds: { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height },
        }
      })
  })
  expect(actual.map((control) => control.name).toSorted()).toEqual(authored.map((control) => control.name).toSorted())
  for (const control of authored) {
    const expected = control.authoredVisible && !conditional.has(control.name) && !sessionConditional.has(control.name)
    expect(actual.find((candidate) => candidate.name === control.name)?.visible, control.name).toBe(expected)
  }

  for (const name of CORE_FOOTER_CONTROLS) await expect(gameUi.locator(`[data-vgui-name="${name}"]`)).toBeVisible()
  for (const name of UNSUPPORTED_CORE_CONTROLS) await expect(gameUi.locator(`[data-vgui-name="${name}"]`)).toBeDisabled()
  for (const name of ["SettingsButton", "TF2SettingsButton", "NewUserForumsButton", "FindAGameButton", "QuitButton"]) {
    await expect(gameUi.locator(`[data-vgui-name="${name}"]`)).toBeEnabled()
  }

  const dashboard = await page.evaluate(() => {
    const owner = document.querySelector<HTMLElement>('.gameui-layer [data-vgui-name="MMDashboard"]')!
    const top = owner.querySelector<HTMLElement>('[data-vgui-name="TopBar"]')!
    const bounds = owner.getBoundingClientRect()
    return {
      bounds: { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height },
      controls: [...top.children]
        .filter((element): element is HTMLElement => element instanceof HTMLElement && element.hasAttribute("data-vgui-panel"))
        .map((element) => element.dataset.vguiName!),
      queueClips: ["QueueContainer", "JoinPartyLobbyContainer"].map((name) => {
        const element = top.querySelector<HTMLElement>(`[data-vgui-name="${name}"]`)!
        return { name, visible: getComputedStyle(element).display !== "none", clipPath: getComputedStyle(element).clipPath }
      }),
    }
  })
  const topBar = dashboardResource.root.children.find((node) => node.name === "TopBar")!
  const authoredDashboard = topBar.children.filter((node) => node.value === null && scalar(node, "ControlName") !== null)
    .map((node) => scalar(node, "fieldName") ?? node.name)
  expect(dashboard.controls.toSorted()).toEqual(authoredDashboard.toSorted())
  expect(dashboard.bounds).toMatchObject({ x: 0, y: -7, width: 1_280, height: 52 })
  expect(dashboard.queueClips.every((entry) => entry.visible && entry.clipPath !== "none")).toBe(true)

  const parties: Rectangle[] = []
  for (let slot = 0; slot < 6; slot += 1) {
    const owner = gameUi.locator(`[data-vgui-name="PartySlot${slot}"]`)
    await expect(owner).toBeVisible()
    await expect(owner.locator('[data-vgui-name="EmptyImage"]')).toBeVisible()
    await expect(owner.locator('[data-vgui-name="InteractButton"]')).toBeDisabled()
    for (const name of ["avatar", "LeaderIcon", "BannedIcon", "OutOfDateIcon", "OfflineIcon", "StatusDimmer", "Spinner"]) {
      await expect(owner.locator(`[data-vgui-name="${name}"]`)).toBeHidden()
    }
    const bounds = await owner.boundingBox()
    if (!bounds) throw new Error(`TF2 dashboard party slot ${slot} has no visible bounds`)
    parties.push(bounds)
  }

  const footerBounds = await Promise.all(CORE_FOOTER_CONTROLS.map(async (name) => {
    const bounds = await gameUi.locator(`[data-vgui-name="${name}"]`).boundingBox()
    if (!bounds) throw new Error(`TF2 footer control ${name} has no visible bounds`)
    return { name, ...bounds }
  }))
  const character = await gameUi.locator('[data-vgui-name="TFCharacterImage"]').boundingBox()
  if (!character) throw new Error("TF2 authored Main Menu character has no visible bounds")
  const menuScreen = await page.screenshot()
  await testInfo.attach("headed-authored-complete-main-menu", { body: menuScreen, contentType: "image/png" })
  const footerPixels = screenRegion(menuScreen, { x: 200, y: 650, width: 900, height: 55 })
  const partyPixels = screenRegion(menuScreen, { x: 60, y: 0, width: 216, height: 38 })
  const characterPixels = screenRegion(menuScreen, { x: 640, y: 130, width: 550, height: 450 })
  expect(footerPixels.colors).toBeGreaterThan(24)
  expect(partyPixels.colors).toBeGreaterThan(12)
  expect(characterPixels.colors).toBeGreaterThan(128)

  await gameUi.locator('[data-vgui-name="FindAGameButton"]').click()
  const side = gameUi.locator('[data-vgui-name="ExpandableList"]')
  await expect(side).toBeVisible()
  const playlistTitle = localized(resources, "#TF_Matchmaking_HeaderModeSelect")
  await expect(side.locator('[data-vgui-name="Title"]')).toHaveText(playlistTitle)
  const illustrations: Array<{ name: string; image: string; enabled: boolean; bounds: Rectangle }> = []
  for (const definition of playlistResource.root.children.filter((node) =>
    node.value === null && node.name !== "EventEntry" && node.name !== "ScrollBar")) {
    const owner = side.locator(`[data-vgui-name="${definition.name}"]`)
    const image = owner.locator('[data-vgui-name="ModeImage"]')
    const mode = owner.locator('[data-vgui-name="ModeButton"]')
    const description = owner.locator('[data-vgui-name="DescLabel"]')
    await expect(owner).toBeVisible()
    const painted = await image.evaluate((element) => ({
      background: getComputedStyle(element).backgroundImage,
      raster: element.querySelector('canvas[data-vgui-raster="image-raster"]') !== null,
    }))
    expect(painted.background !== "none" || painted.raster, definition.name).toBe(true)
    await expect(description).not.toHaveText(/^(?:#|\[unknown\])/u)
    const enabled = ["TrainingEntry", "CreateServerEntry"].includes(definition.name)
    if (enabled) await expect(mode).toBeEnabled()
    else await expect(mode).toBeDisabled()
    const bounds = await image.boundingBox()
    if (!bounds) throw new Error(`TF2 authored playlist illustration ${definition.name} has no visible bounds`)
    illustrations.push({ name: definition.name, image: scalar(definition, "image_name")!, enabled, bounds })
  }
  await expect(side.locator('[data-vgui-name="EventEntry"]')).toBeHidden()
  const playlistScreen = await page.screenshot()
  await testInfo.attach("headed-authored-complete-playlist", { body: playlistScreen, contentType: "image/png" })
  const illustrationPixels = illustrations.map((entry) => ({ name: entry.name, ...screenRegion(playlistScreen, entry.bounds) }))
  for (const entry of illustrationPixels) expect(entry.colors, entry.name).toBeGreaterThan(16)

  await side.locator('[data-vgui-name="TrainingEntry"] [data-vgui-name="ModeButton"]').click()
  await expect(main).toHaveAttribute("data-local-match-entry", "training")
  await expect(page.locator('.local-match-layer [data-vgui-name="TitleLabel"]')).toHaveText("SELECT A TRAINING MODE")
  await page.locator('.local-match-layer [data-vgui-name="Container"] > [data-vgui-name="CancelButton"]').click()
  await expect(main).toHaveAttribute("data-local-match-visible", "false")

  await gameUi.locator('[data-vgui-name="FindAGameButton"]').click()
  await side.locator('[data-vgui-name="CreateServerEntry"] [data-vgui-name="ModeButton"]').click()
  await expect(main).toHaveAttribute("data-local-match-entry", "create-server")
  const createServer = page.locator(".local-match-layer").getByRole("dialog", { name: "CREATE SERVER" })
  await createServer.locator('[data-vgui-name="MapList"]').click()
  const targets = await page.evaluate(async () => {
    const response = await fetch("/playsrc-config.json")
    return (await response.json() as { targets: Array<{ target: string }> }).targets.map((target) => target.target)
  })
  for (const target of targets) await expect(page.getByRole("option", { name: target })).toBeVisible()
  expect(targets).toEqual(["jump_beef", "pl_upward", "ctf_2fort"])
  await page.getByRole("option", { name: "jump_beef" }).click()
  await page.keyboard.press("Escape")
  await expect(main).toHaveAttribute("data-local-match-visible", "false")

  await gameUi.locator('[data-vgui-name="SettingsButton"]').click()
  await expect(main).toHaveAttribute("data-options-visible", "true")
  await page.keyboard.press("Escape")
  await expect(main).toHaveAttribute("data-options-visible", "false")

  await page.setViewportSize({ width: 1_024, height: 768 })
  await expect(gameUi.locator('[data-vgui-name="MainMenuOverride"]')).toHaveCSS("width", "1024px")
  const narrower = await page.evaluate(() => {
    const rectangle = (name: string) => {
      const bounds = document.querySelector<HTMLElement>(`.gameui-layer [data-vgui-name="${name}"]`)!.getBoundingClientRect()
      return { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height }
    }
    return { dashboard: rectangle("MMDashboard"), store: rectangle("GeneralStoreButton"), footer: rectangle("ReplayButton") }
  })
  expect(narrower.dashboard.height).toBe(Math.trunc(35 * 768 / 480))
  expect(narrower.footer.y + narrower.footer.height).toBeLessThanOrEqual(768)
  await gameUi.locator('[data-vgui-name="FindAGameButton"]').click()
  await expect(side).toHaveCSS("width", "448px")
  const narrowSide = await side.boundingBox()
  expect(narrowSide).toMatchObject({ x: 576, width: 448 })
  await page.setViewportSize({ width: 1_280, height: 720 })
  await expect(gameUi.locator('[data-vgui-name="MainMenuOverride"]')).toHaveCSS("width", "1280px")

  const seconds = profileSampleSeconds()
  const performance = await page.evaluate(async (duration) => {
    const started = globalThis.performance.now()
    let previous = started
    const frames: number[] = []
    await new Promise<void>((resolve) => {
      const frame = (now: number) => {
        frames.push(now - previous)
        previous = now
        if (now - started >= duration * 1_000) resolve()
        else requestAnimationFrame(frame)
      }
      requestAnimationFrame(frame)
    })
    const memory = (globalThis.performance as Performance & { memory?: { usedJSHeapSize: number } }).memory
    return {
      seconds: (globalThis.performance.now() - started) / 1_000,
      frames,
      heapBytes: memory?.usedJSHeapSize ?? null,
      menuNodes: document.querySelectorAll(".gameui-layer *").length,
    }
  }, seconds)
  expect(performance.frames.length).toBeGreaterThan(seconds * 35)

  const report = {
    schema: "playsrc-tf2-headed-main-menu-inventory-v1",
    headed: true,
    contentBuild: resources.contentBuild,
    sources: {
      mainMenu: menuResource.source.sha256,
      gameMenu: gameMenuResource.source.sha256,
      dashboard: dashboardResource.source.sha256,
      party: resource("resource/ui/dashboardpartymember.res").source.sha256,
      playlist: playlistResource.source.sha256,
    },
    mainMenu: actual,
    conditional: CONDITIONAL_MAIN_MENU_CONTROLS,
    dashboard,
    party: parties,
    footer: footerBounds,
    playlist: illustrations,
    configuredMaps: targets,
    aspect: { wide: { width: 1_280, height: 720 }, narrower, narrowSide },
    pixels: {
      mainMenuSha256: createHash("sha256").update(menuScreen).digest("hex"),
      playlistSha256: createHash("sha256").update(playlistScreen).digest("hex"),
      footer: footerPixels,
      party: partyPixels,
      character: characterPixels,
      illustrations: illustrationPixels,
    },
    performance: {
      seconds: Number(performance.seconds.toFixed(3)),
      frames: summarizeFrameTimes(performance.frames),
      heapBytes: performance.heapBytes,
      menuNodes: performance.menuNodes,
    },
  }
  const local = await loadLocalConfig()
  const directory = path.join(local.sourceCacheDir, "profiles", "main-menu")
  await mkdir(directory, { recursive: true })
  await Promise.all([
    writeFile(path.join(directory, "main-menu-inventory.json"), `${JSON.stringify(report, null, 2)}\n`),
    writeFile(path.join(directory, "main-menu.png"), menuScreen),
    writeFile(path.join(directory, "playlist.png"), playlistScreen),
  ])
  console.log(`[main-menu] ${JSON.stringify({
    controls: actual.length,
    visible: actual.filter((control) => control.visible).length,
    footer: CORE_FOOTER_CONTROLS.length,
    partySlots: parties.length,
    playlist: illustrations.length,
    maps: targets,
    seconds: report.performance.seconds,
    frames: report.performance.frames,
    pixels: { footer: footerPixels.colors, party: partyPixels.colors, character: characterPixels.colors },
    report: path.join(directory, "main-menu-inventory.json"),
  })}`)
})
