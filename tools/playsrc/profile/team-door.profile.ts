import { writeFile } from "node:fs/promises"
import { createHash } from "node:crypto"
import path from "node:path"
import { test, expect } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { startupConsoleIdle, startupNativeReader } from "./native-startup"
import { requireStartupNative } from "./static-startup-gate"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"
import { profileArtifact } from "./profile-artifacts"
import { chooseTf2Team } from "./team-selection-evidence"

const disabledTeam = process.env.PLAYSRC_TEAM_DOOR_DISABLED as "red" | "blue" | undefined

test("authored team doors real-time motion and reentry", async ({ page, context }) => {
  const { sourceCacheDir } = await loadLocalConfig(), directory = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!
  if (!directory) throw new Error("Use the checked team-door profile runner")
  const native = await startupNativeReader(page, sourceCacheDir)
  const admissions: unknown[] = [], captures: { file: string; before: number; after: number; data: string; timestamp: number; privacy: string; bytes?: number; sha256?: string }[] = [], actions: unknown[] = []
  const cdp = await context.newCDPSession(page)
  const check = async (name: string) => {
    const observation = await native.read(sampling ? undefined : path.join(directory, `${name}.desktop.png`))
    admissions.push(observation)
    requireStartupNative(observation)
  }
  await page.addInitScript(installBrowserFrameProfiler)
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = { captureTeamDoors: false } })
  let sampling = false, failure: string | null = null
  cdp.on("Page.screencastFrame", frame => {
    if (sampling && captures.length < 1024) captures.push({ file: `motion-${String(captures.length).padStart(3, "0")}.page.png`,
      before: Date.now(), after: Date.now(), timestamp: frame.metadata.timestamp! * 1000, data: frame.data, privacy: "client-only-review-required" })
    void cdp.send("Page.screencastFrameAck", { sessionId: frame.sessionId }).catch(() => {})
  })
  try {
    await page.goto("/", { waitUntil: "domcontentloaded" })
    await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu")
    await check("before-load")
    await page.keyboard.press("Backquote")
    await page.locator("[aria-label='Console command']").fill(`map ${disabledTeam ? "pl_upward" : "jump_beef"}`)
    await page.keyboard.press("Enter")
    if (await page.locator("main").getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await expect(page.locator("main")).toHaveAttribute("data-team-selection-models", /reddoor:[^|]+:\d+:\d+/, { timeout: 75_000 })
    if (disabledTeam) {
      await chooseTf2Team(page, "red", () => check("before-team-choice"))
      await expect(page.locator("main")).toHaveAttribute("data-phase", "Ready", { timeout: 60_000 })
      await page.keyboard.press("Backquote")
      await page.locator("[aria-label='Console command']").fill(`tf_bot_add 3 ${disabledTeam} soldier easy`)
      await page.keyboard.press("Enter")
      await expect(page.locator("main")).toHaveAttribute("data-bot-count", "3")
      await page.keyboard.press("Backquote")
      await page.keyboard.press("Period")
      await expect(page.locator(`.team-selection-layer [data-vgui-name='teambutton${disabledTeam === "red" ? 1 : 0}']`)).toHaveAttribute("aria-disabled", "true")
    }
    await page.waitForTimeout(2100)
    if (await startupConsoleIdle(sourceCacheDir) < 2000) throw new Error("Team-door motion requires genuine native idle")
    await check("before-motion")
    const heapBefore = await cdp.send("Runtime.getHeapUsage")
    await page.evaluate(() => { (globalThis as any).__playsrcProfile.captureTeamDoors = true; (globalThis as any).__playsrcFrameProfiler.active = true })
    const started = Date.now()
    sampling = true
    await cdp.send("Page.startScreencast", { format: "png", everyNthFrame: 2 })
    const hover = async (team: "red" | "blue" | "auto" | null, wait: number) => {
      actions.push({ at: Date.now(), team })
      if (team) await page.locator(`.team-selection-layer [data-vgui-name='teambutton${team === "blue" ? 0 : team === "red" ? 1 : 2}']`).hover()
      else await page.mouse.move(4, 4)
      await page.waitForTimeout(wait)
    }
    if (disabledTeam) {
      await hover(null, 200)
      await hover(disabledTeam, 2350)
      await hover(null, 350)
      await check("before-focus")
      // Explicit navigation invokes enter/exit; an initial desktop focus alone
      // is not a cursor-enter event in the authored PC menu.
      await page.keyboard.press("ArrowRight")
      for (let index = 0; index < (disabledTeam === "red" ? 3 : 2); index++) await page.keyboard.press("ArrowRight")
      actions.push({ at: Date.now(), focus: disabledTeam })
      await page.waitForTimeout(350)
      await page.keyboard.press("ArrowRight")
      await page.waitForTimeout(350)
      await check("before-reopen")
      await page.keyboard.press("Escape")
      await expect(page.locator("main")).toHaveAttribute("data-team-selection-visible", "false")
      await page.keyboard.press("Period")
      await expect(page.locator("main")).toHaveAttribute("data-team-selection-visible", "true")
      actions.push({ at: Date.now(), reopen: true })
      await hover(disabledTeam, 100)
      await hover(null, 100)
      await hover(disabledTeam, 350)
      await hover(null, 350)
      await page.keyboard.press("Backquote")
      await page.locator("[aria-label='Console command']").fill("tf_bot_kick all")
      await page.keyboard.press("Enter")
      await expect(page.locator("main")).toHaveAttribute("data-bot-count", "0")
      await page.keyboard.press("Backquote")
      await expect(page.locator(`.team-selection-layer [data-vgui-name='teambutton${disabledTeam === "red" ? 1 : 0}']`)).toHaveAttribute("aria-disabled", "false")
      actions.push({ at: Date.now(), enabled: disabledTeam })
      await hover(disabledTeam, 550)
      await hover(null, 650)
    } else {
    for (const team of ["auto", "blue", "red"] as const) {
      await hover(team, 550)
      await hover(null, 650)
      await check(`after-${team}`)
    }
    await hover("red", 70)
    await hover(null, 70)
    await hover("red", 650)
    await hover(null, 650)
    }
    await page.waitForTimeout(Math.max(0, 6000 - (Date.now() - started)))
    sampling = false
    await cdp.send("Page.stopScreencast")
    const activeMilliseconds = Date.now() - started
    await check("after-motion")
    const observation = await page.evaluate(() => {
      const root = globalThis as any
      root.__playsrcProfile.captureTeamDoors = false
      root.__playsrcFrameProfiler.active = false
      return { doors: root.__playsrcProfile.teamDoorFrames, frames: root.__playsrcFrameProfiler }
    })
    const heapAfter = await cdp.send("Runtime.getHeapUsage")
    await profileArtifact(async () => {
      for (const capture of captures) {
        const bytes = Buffer.from(capture.data, "base64")
        await writeFile(path.join(directory, capture.file), bytes)
        capture.data = ""
        capture.bytes = bytes.length
        capture.sha256 = createHash("sha256").update(bytes).digest("hex")
      }
      await writeFile(path.join(directory, "team-door-motion.json"), JSON.stringify({ activeMilliseconds, disabledTeam, observation, actions, captures, heapBefore, heapAfter }))
    })
    expect(activeMilliseconds).toBeGreaterThanOrEqual(5000)
    expect(activeMilliseconds).toBeLessThan(10000)
    expect(captures.length).toBeGreaterThan(30)
    if (disabledTeam) {
      const name = disabledTeam === "red" ? "reddoor" : "bluedoor"
      expect(observation.doors.some((frame: any) => frame.panels.some((panel: any) => panel.name === name && panel.animation === "hover_disabled"))).toBe(true)
      expect(observation.doors.at(-1).panels.find((panel: any) => panel.name === name).sequence).toBe("hoverclose")
    }
  } catch (error) { failure = String(error); throw error }
  finally {
    sampling = false
    await cdp.send("Page.stopScreencast").catch(() => {})
    await native.close()
    await profileArtifact(async () => { await writeFile(path.join(directory, "team-door-admission.json"), JSON.stringify({ admissions, records: native.records, actions, captures, failure })) })
  }
})
