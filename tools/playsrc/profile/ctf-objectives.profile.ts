import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
import { expect, test } from "./application-test"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { chooseTf2Team } from "./team-selection-evidence"

const BLUE_FLAG = [489.005, -3348.51, -170] as const
const RED_CAPTURE = [-500, 3366, -170] as const
const BLUE_DROP = [450, -3300, -170] as const

test("headed ctf_2fort intelligence, objective HUD, announcer, and round victory", async ({ page }, testInfo) => {
  await page.addInitScript(() => {
    ;(globalThis as typeof globalThis & { __playsrcProfile?: object }).__playsrcProfile = {}
  })
  await page.goto("/")
  const root = page.locator("main")
  await expect(root).toHaveAttribute("data-phase", "MainMenu", { timeout: 120_000 })
  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  await entry.fill("map ctf_2fort")
  await entry.press("Enter")
  await page.waitForFunction(() => {
    const main = document.querySelector<HTMLElement>("main")
    return main?.dataset.teamSelectionVisible === "true" || main?.dataset.phase === "Ready" || main?.dataset.phase === "Failed"
  }, undefined, { timeout: 600_000, polling: 50 })
  if (await root.getAttribute("data-team-selection-visible") === "true") {
    if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await chooseTf2Team(page, "red")
  }
  await expect(root).toHaveAttribute("data-phase", "Ready", { timeout: 600_000 })
  if (await root.getAttribute("data-class-selection-visible") === "true") {
    await page.keyboard.press("Digit2")
    await expect(root).toHaveAttribute("data-class-selection-visible", "false")
  }
  if (await root.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
  await expect(root).toHaveAttribute("data-ctf", /^0:0:3:0:/)
  await expect(page.locator("[data-vgui-name='HudObjectiveStatus']")).toBeVisible()
  await expect(page.locator("[data-vgui-name='RedFlag']")).toBeVisible()
  await expect(page.locator("[data-vgui-name='BlueFlag']")).toBeVisible()

  await page.keyboard.press("Backquote")
  await expect(root).toHaveAttribute("data-console-visible", "false")
  await page.keyboard.press("KeyW")
  await page.keyboard.press("Backquote")
  await expect(root).toHaveAttribute("data-console-visible", "true")

  await page.evaluate(() => {
    ;(globalThis as any).__playsrcProfile.displacementCameraOverride = {
      position: [489, -3175, -110],
      yawDegrees: -90,
      pitchDegrees: 4,
    }
  })
  await page.waitForFunction(() => document.querySelector<HTMLElement>(".world-canvas")?.dataset.displayCameraPosition === "489,-3175,-110")
  const homePixels = await page.locator(".world-canvas").screenshot()
  await testInfo.attach("ctf-2fort-blue-intelligence-home", { body: homePixels, contentType: "image/png" })

  await entry.fill("noclip")
  await entry.press("Enter")
  await expect(root).toHaveAttribute("data-movement-mode", "1")

  const setpos = async (position: readonly [number, number, number]): Promise<void> => {
    await entry.fill(`setpos ${position.join(" ")}`)
    await entry.press("Enter")
  }
  await setpos(BLUE_FLAG)
  await expect.poll(async () => root.getAttribute("data-ctf")).toMatch(/,3,1,1(?:,|\||$)/)
  await expect(page.locator("[data-vgui-name='CarriedImage']")).toBeVisible()
  await expect(page.locator("[data-vgui-name='RedFlag']")).toBeHidden()
  await expect(page.locator("[data-vgui-name='BlueFlag']")).toBeHidden()
  await expect.poll(async () => root.getAttribute("data-audio-starts")).toContain("CaptureFlag.TeamStolen")
  const carriedPixels = await page.locator(".world-canvas").screenshot()
  await testInfo.attach("ctf-2fort-blue-intelligence-carried", { body: carriedPixels, contentType: "image/png" })

  await setpos(BLUE_DROP)
  await page.waitForTimeout(100)
  await entry.fill("dropitem")
  await entry.press("Enter")
  await expect.poll(async () => root.getAttribute("data-ctf")).toMatch(/,3,2,0,/)
  await expect(page.locator("[data-vgui-name='BlueFlag']")).toBeVisible()
  await expect.poll(async () => root.getAttribute("data-audio-starts")).toContain("CaptureFlag.TeamDropped")
  await expect.poll(async () => root.getAttribute("data-ctf"), { timeout: 6_000 }).toMatch(/,3,1,1(?:,|\||$)/)

  await setpos(RED_CAPTURE)
  await expect.poll(async () => root.getAttribute("data-ctf")).toMatch(/^1:0:3:0:/)
  await expect.poll(async () => root.getAttribute("data-audio-starts")).toContain("CaptureFlag.TeamCaptured")
  for (let capture = 2; capture <= 3; capture += 1) {
    await page.waitForTimeout(300)
    await setpos(BLUE_FLAG)
    await expect.poll(async () => root.getAttribute("data-ctf")).toMatch(/,3,1,1(?:,|\||$)/)
    await setpos(RED_CAPTURE)
    await expect.poll(async () => root.getAttribute("data-ctf")).toMatch(new RegExp(`^${capture}:0:3:${capture === 3 ? 2 : 0}:`))
  }
  await expect.poll(async () => root.getAttribute("data-audio-starts")).toContain("Game.YourTeamWon")
  const objectivePixels = await page.screenshot()
  await testInfo.attach("ctf-2fort-red-round-victory-hud", { body: objectivePixels, contentType: "image/png" })

  await page.keyboard.press("Backquote")
  await expect(root).toHaveAttribute("data-console-visible", "false")
  const seconds = profileSampleSeconds()
  const performance = await page.evaluate(async (duration) => {
    const root = document.querySelector<HTMLElement>("main")!
    const firstTick = Number(root.dataset.snapshotTick)
    const started = window.performance.now()
    let previous = started
    const frames: number[] = []
    const objectivePreparation: number[] = []
    await new Promise<void>((resolve) => {
      const frame = (now: number): void => {
        frames.push(now - previous)
        previous = now
        const detail = root.dataset.performanceDetail
        if (detail) objectivePreparation.push(JSON.parse(detail).models)
        if (now - started >= duration * 1000) resolve()
        else requestAnimationFrame(frame)
      }
      requestAnimationFrame(frame)
    })
    return {
      seconds: (window.performance.now() - started) / 1000,
      firstTick,
      lastTick: Number(root.dataset.snapshotTick),
      frames,
      objectivePreparation,
    }
  }, seconds)
  expect(performance.lastTick - performance.firstTick).toBeGreaterThan(seconds * 60)

  const pixelDelta = await page.evaluate(async ({ before, after }) => {
    const decode = async (source: string): Promise<ImageData> => {
      const image = new Image()
      image.src = `data:image/png;base64,${source}`
      await image.decode()
      const canvas = document.createElement("canvas")
      canvas.width = image.width
      canvas.height = image.height
      const context = canvas.getContext("2d")!
      context.drawImage(image, 0, 0)
      return context.getImageData(0, 0, image.width, image.height)
    }
    const [home, carried] = await Promise.all([decode(before), decode(after)])
    let changed = 0
    for (let y = 100; y < home.height - 100; y += 1) {
      for (let x = 320; x < home.width - 320; x += 1) {
        const index = (y * home.width + x) * 4
        const delta = Math.abs(home.data[index]! - carried.data[index]!)
          + Math.abs(home.data[index + 1]! - carried.data[index + 1]!)
          + Math.abs(home.data[index + 2]! - carried.data[index + 2]!)
        if (delta > 32) changed += 1
      }
    }
    return changed
  }, { before: homePixels.toString("base64"), after: carriedPixels.toString("base64") })
  expect(pixelDelta).toBeGreaterThan(100)

  const report = {
    schema: "playsrc-tf2-headed-ctf-objectives-v1",
    headed: true,
    target: "ctf_2fort",
    bspSha256: "cbd191411c0be57099da73458167001ec80d58bf37c71cb3c36b2911b6e80fd7",
    captures: { red: 3, blue: 0, limit: 3, winner: 2 },
    events: ["pickup", "drop", "pickup", "capture", "capture", "capture", "round-win"],
    announcer: await root.getAttribute("data-audio-starts"),
    visibleBriefcasePixelsChangedAfterPickup: pixelDelta,
    simulation: {
      seconds: Number(performance.seconds.toFixed(3)),
      firstTick: performance.firstTick,
      lastTick: performance.lastTick,
      ticksPerSecond: Number(((performance.lastTick - performance.firstTick) / performance.seconds).toFixed(2)),
    },
    frames: summarizeFrameTimes(performance.frames),
    modelPreparation: summarizeFrameTimes(performance.objectivePreparation),
  }
  const local = await loadLocalConfig()
  const directory = path.join(local.sourceCacheDir, "evidence", "tf2-ctf-objectives")
  await mkdir(directory, { recursive: true })
  await Promise.all([
    writeFile(path.join(directory, "ctf_2fort-objectives.json"), `${JSON.stringify(report, null, 2)}\n`),
    writeFile(path.join(directory, "ctf_2fort-intelligence-home.png"), homePixels),
    writeFile(path.join(directory, "ctf_2fort-intelligence-carried.png"), carriedPixels),
    writeFile(path.join(directory, "ctf_2fort-round-victory.png"), objectivePixels),
  ])
  console.log(`[ctf-objectives] ${JSON.stringify(report)}`)
})
