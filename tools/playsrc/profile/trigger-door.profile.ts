import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, test } from "./application-test"
import { chooseTf2Team } from "./team-selection-evidence"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"
import { loadLocalConfig } from "../src/config"
import { Tf2BrowserAutomation } from "../../../apps/web/tf2/src/browser-automation"
import { decodeScreenshot } from "./screenshot-pixels"

const mapName = process.env.PROFILE_DOOR_MAP ?? "ctf_2fort"
const baseline = process.env.PROFILE_DOOR_BASELINE === "1"
for (const team of (mapName === "pl_upward" ? ["red"] : ["red", "blue"]) as ("red" | "blue")[]) test(`headed normal WASD trigger door approach: ${team}`, async ({ page }, testInfo) => {
  await page.addInitScript(installBrowserFrameProfiler)
  await page.addInitScript(() => { (window as any).__playsrcProfile = {} })
  const root = page.locator("main")
  const automation = new Tf2BrowserAutomation({
    evaluate: async <T>(expression: string): Promise<T> => page.evaluate(expression) as Promise<T>,
    press: async key => { await page.keyboard.press(key) },
    click: async selector => { await page.locator(selector).click() },
    focus: async selector => { await page.locator(selector).focus() },
    fill: async (selector, value) => { await page.locator(selector).fill(value) },
    waitFor: async (expression, timeout) => { await page.waitForFunction(expression, undefined, { timeout }) },
    activateCurrentTab: async () => { await page.bringToFront() },
  })
  await page.goto("/")
  await expect(root).toHaveAttribute("data-phase", "MainMenu", { timeout: 60_000 })
  await automation.maps.load(mapName)
  await page.waitForFunction(() => {
    const d = document.querySelector<HTMLElement>("main")?.dataset
    return d?.teamSelectionVisible === "true" || d?.phase === "Ready" || d?.phase === "Failed"
  }, undefined, { timeout: 90_000 })
  if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  if (await root.getAttribute("data-team-selection-visible") === "true") await chooseTf2Team(page, team)
  await expect(root).toHaveAttribute("data-phase", "Ready")
  if (await root.getAttribute("data-class-selection-visible") === "true") {
    await page.keyboard.press("Digit2")
    await expect(root).toHaveAttribute("data-class-selection-visible", "false")
  }
  const config = await loadLocalConfig()
  const directory = path.join(config.sourceCacheDir, "evidence", "trigger-door", process.env.PROFILE_DOOR_LABEL ?? "after")
  await mkdir(directory, { recursive: true })
  const report: unknown[] = []
  const aim = async (target: readonly number[]) => {
    const movement = await page.evaluate(target => {
      const d = document.querySelector<HTMLElement>("main")!.dataset
      const p = d.cameraPosition!.split(",").map(Number)
      const yaw = Math.atan2(target[1]! - p[1]!, target[0]! - p[0]!) * 180 / Math.PI
      const wrap = (n: number) => ((n + 180) % 360 + 360) % 360 - 180
      return { x: wrap(Number(d.cameraYaw) - yaw) / 0.066, y: -Number(d.cameraPitch) / 0.066 }
    }, target)
    await automation.player.lookBy(movement)
  }
    expect(await root.getAttribute("data-movement-mode")).toBe("0")
    const pointer = await automation.pointer.capture(`trigger door ${team}`)
    const sign = team === "red" ? 1 : -1
    await aim(mapName === "pl_upward" ? [512, 1268] : [sign * 1534, sign * 1440])
    await page.evaluate(targets => {
      const p = (window as any).__playsrcProfile
      p.doorEvidenceTargets = targets; p.doorEvidence = []
    }, mapName === "pl_upward" ? [538] : team === "red" ? [868, 869, 783] : [856, 857, 385])
    const visualStarted = Date.now()
    await page.keyboard.down("w")
    await page.waitForTimeout(1_650)
    await page.keyboard.up("w")
    await aim(mapName === "pl_upward" ? [512, 1040] : [sign * 700, sign * 1440])
    await page.keyboard.down("w")
    await page.waitForFunction(({ sign, upward }) => {
      const position = document.querySelector<HTMLElement>("main")!.dataset.cameraPosition!.split(",").map(Number)
      return upward ? position[1]! < 1150 : sign * position[0]! < 720
    }, { sign, upward: mapName === "pl_upward" }, { timeout: 15_000 })
    await page.keyboard.up("w")
    await page.waitForTimeout(600)
    const visual = JSON.parse(await page.evaluate(() => {
      const profile = (window as any).__playsrcProfile
      profile.doorEvidenceTargets = []
      return JSON.stringify({ captures: profile.doorEvidence, position: document.querySelector<HTMLElement>("main")!.dataset.cameraPosition }, (_, value) => typeof value === "bigint" ? value.toString() : value)
    }))
    // Synchronous PNG extraction and geometry rays are evidence work, not
    // gameplay. Both revisions use this same separate five-second return walk
    // to measure simulation, frame CPU and submissions without readback stalls.
    const visualWallMilliseconds = Date.now() - visualStarted
    await aim(mapName === "pl_upward" ? [512, 1568] : [sign * 1888, sign * 1440])
    const started = await page.evaluate(() => {
      const p = (window as any).__playsrcFrameProfiler
      p.active = true; p.completedFrames = []
      return { time: performance.now(), tick: Number(document.querySelector<HTMLElement>("main")!.dataset.snapshotTick) }
    })
    await page.keyboard.down("w")
    await page.waitForTimeout(5_000)
    await page.keyboard.up("w")
    const result = JSON.parse(await page.evaluate(start => {
      const p = (window as any).__playsrcFrameProfiler
      p.active = false
      const elapsed = performance.now() - start.time
      return JSON.stringify({ elapsed, simHz: (Number(document.querySelector<HTMLElement>("main")!.dataset.snapshotTick) - start.tick) / elapsed * 1_000,
        frames: p.completedFrames, captures: (window as any).__playsrcProfile.doorEvidence,
        position: document.querySelector<HTMLElement>("main")!.dataset.cameraPosition }, (_, value) => typeof value === "bigint" ? value.toString() : value)
    }, started))
    result.captures = visual.captures
    result.passagePosition = visual.position
    result.visualWallMilliseconds = visualWallMilliseconds
    const doorSource = mapName === "pl_upward" ? 538 : team === "red" ? 783 : 385
    const visibleSource = mapName === "pl_upward" ? 537 : team === "red" ? 784 : 386
    const opening = result.captures.find((capture: any) => capture.key === `${doorSource}:2`)
    const opened = result.captures.find((capture: any) => capture.key === `${doorSource}:3`)
    const center = (capture: any) => capture.geometry.samples.find((sample: any) => sample.x === 0 && sample.y === 0)
    const rgb = (capture: any) => {
      const image = decodeScreenshot(Buffer.from(capture.pixels.split(",")[1], "base64"))
      const at = (Math.floor(image.height / 2) * image.width + Math.floor(image.width / 2)) * image.channels
      return [...image.pixels.subarray(at, at + 3)]
    }
    if (opening && opened) result.pixelDepth = { opening: { ...center(opening), rgb: rgb(opening) }, opened: { ...center(opened), rgb: rgb(opened) } }
    for (const capture of result.captures) {
      const filename = `${team}-${capture.key.replace(":", "-")}.png`
      await writeFile(path.join(directory, filename), Buffer.from(capture.pixels.split(",")[1], "base64"))
      capture.pixels = filename
    }
    report.push({ team, pointer, ...result })
    await writeFile(path.join(directory, `${team}-report.json`), JSON.stringify(report, null, 2))
    console.log("[door-result]", JSON.stringify({ team, simHz: result.simHz, elapsed: result.elapsed, position: result.position, captures: result.captures.map((value: any) => value.key) }))
    if (!baseline) {
    for (const phase of ["1", "2", "3", "4", "closed-after"]) expect(result.captures.some((capture: any) => capture.key === `${doorSource}:${phase}`), `${team} authored door ${phase}`).toBe(true)
    expect(result.pixelDepth.opening.object).toBe(visibleSource)
    expect(result.pixelDepth.opened.object).not.toBe(visibleSource)
    expect(result.pixelDepth.opened.depth).toBeGreaterThan(result.pixelDepth.opening.depth + 100)
    expect(result.pixelDepth.opened.rgb.reduce((sum: number, value: number, axis: number) => sum + Math.abs(value - result.pixelDepth.opening.rgb[axis]), 0)).toBeGreaterThan(20)
    }
    expect(result.simHz).toBeGreaterThan(60)
    const passage = result.passagePosition.split(",").map(Number)
    expect(mapName === "pl_upward" ? passage[1] : sign * passage[0]).toBeLessThan(mapName === "pl_upward" ? 1200 : 850)
  await testInfo.attach("door-evidence", { body: JSON.stringify({ directory, report }), contentType: "application/json" })
})
