import { readFile, writeFile } from "node:fs/promises"
import { createHash } from "node:crypto"
import path from "node:path"
import { test, expect } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { startupConsoleIdle, startupNativeReader } from "./native-startup"
import { requireStartupNative } from "./static-startup-gate"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"
import { profileArtifact } from "./profile-artifacts"

test("authored team doors real-time motion and reentry", async ({ page, context }) => {
  const { sourceCacheDir } = await loadLocalConfig(), directory = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!
  if (!directory) throw new Error("Use the checked team-door profile runner")
  const native = await startupNativeReader(page, sourceCacheDir)
  const admissions: unknown[] = [], captures: unknown[] = [], actions: unknown[] = []
  const cdp = await context.newCDPSession(page)
  const check = async (name: string) => {
    const observation = await native.read(path.join(directory, `${name}.desktop.png`))
    admissions.push(observation)
    requireStartupNative(observation)
  }
  await page.addInitScript(installBrowserFrameProfiler)
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = { captureTeamDoors: false } })
  let sampling = false, captureTask: Promise<void> | undefined, failure: string | null = null
  try {
    await page.goto("/", { waitUntil: "domcontentloaded" })
    await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu")
    await check("before-load")
    await page.keyboard.press("Backquote")
    await page.locator("[aria-label='Console command']").fill("map jump_beef")
    await page.keyboard.press("Enter")
    if (await page.locator("main").getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await expect(page.locator("main")).toHaveAttribute("data-team-selection-models", /reddoor:[^|]+:\d+:\d+/, { timeout: 75_000 })
    await page.waitForTimeout(2100)
    if (await startupConsoleIdle(sourceCacheDir) < 2000) throw new Error("Team-door motion requires genuine native idle")
    await check("before-motion")
    const heapBefore = await cdp.send("Runtime.getHeapUsage")
    await page.evaluate(() => { (globalThis as any).__playsrcProfile.captureTeamDoors = true; (globalThis as any).__playsrcFrameProfiler.active = true })
    const started = Date.now()
    sampling = true
    captureTask = (async () => {
      while (sampling) {
        const file = `motion-${String(captures.length).padStart(3, "0")}.page.png`, before = Date.now()
        await page.screenshot({ path: path.join(directory, file) })
        captures.push({ file, before, after: Date.now(), privacy: "client-only-review-required" })
      }
    })()
    const hover = async (team: "red" | "blue" | "auto" | null, wait: number) => {
      actions.push({ at: Date.now(), team })
      if (team) await page.locator(`.team-selection-layer [data-vgui-name='teambutton${team === "blue" ? 0 : team === "red" ? 1 : 2}']`).hover()
      else await page.mouse.move(4, 4)
      await page.waitForTimeout(wait)
    }
    for (const team of ["auto", "blue", "red"] as const) {
      await hover(team, 550)
      await hover(null, 650)
      await check(`after-${team}`)
    }
    await hover("red", 70)
    await hover(null, 70)
    await hover("red", 650)
    await hover(null, 650)
    await page.waitForTimeout(Math.max(0, 6000 - (Date.now() - started)))
    sampling = false
    await captureTask
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
      for (const capture of captures as { file: string; bytes?: number; sha256?: string }[]) {
        const bytes = await readFile(path.join(directory, capture.file))
        capture.bytes = bytes.length
        capture.sha256 = createHash("sha256").update(bytes).digest("hex")
      }
      await writeFile(path.join(directory, "team-door-motion.json"), JSON.stringify({ activeMilliseconds, observation, actions, captures, heapBefore, heapAfter }))
    })
    expect(activeMilliseconds).toBeGreaterThanOrEqual(5000)
    expect(activeMilliseconds).toBeLessThan(10000)
    expect(captures.length).toBeGreaterThan(30)
  } catch (error) { failure = String(error); throw error }
  finally {
    sampling = false
    await captureTask?.catch(() => {})
    await native.close()
    await profileArtifact(async () => { await writeFile(path.join(directory, "team-door-admission.json"), JSON.stringify({ admissions, records: native.records, actions, captures, failure })) })
  }
})
