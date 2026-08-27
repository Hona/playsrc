import { expect, test } from "./application-test"
import { summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"
import { Tf2BrowserAutomation } from "../../../apps/web/tf2/src/browser-automation"
import { writeFile } from "node:fs/promises"

test("headed authored attack/defend stages, capture, timers, team switch and local bots", async ({ page }, testInfo) => {
  const map = process.env.PROFILE_ATTACK_DEFEND_MAP!
  const performanceOnly = process.env.PROFILE_ATTACK_DEFEND_PERFORMANCE === "1"
  const main = page.locator("main")
  const automation = new Tf2BrowserAutomation({
    evaluate: async <T>(expression: string): Promise<T> => page.evaluate(expression) as Promise<T>,
    press: key => page.keyboard.press(key), click: selector => page.locator(selector).click(),
    focus: selector => page.locator(selector).focus(), fill: (selector, value) => page.locator(selector).fill(value),
    waitFor: async (expression, timeout) => { await page.waitForFunction(expression, undefined, { timeout }) },
    activateCurrentTab: () => page.bringToFront(),
  })
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = {} })
  await page.goto("/")
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 60_000 })
  const command = async (text: string) => {
    if (await main.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    const entry = page.locator("[aria-label='Console command']")
    await entry.fill(text); await entry.press("Enter")
  }
  const close = async () => { if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote") }
  const json = async (name: string, value: unknown) => {
    const path = testInfo.outputPath(`${name}.json`)
    await writeFile(path, JSON.stringify(value))
    await testInfo.attach(name, { path, contentType: "application/json" })
  }
  let geometryRevision = 0
  const capture = async (name: string) => {
    await close()
    await expect(page.locator(".hud-layer [data-vgui-name='HudControlPointIcons'] [data-vgui-name='BaseImage']:visible")).toHaveCount(2)
    const revision = ++geometryRevision
    await page.evaluate(revision => { (globalThis as any).__playsrcProfile.geometryEvidenceRevision = revision }, revision)
    await page.waitForFunction(revision => (globalThis as any).__playsrcProfile.geometryEvidence?.revision === revision, revision, { timeout: 5000 })
    const geometry = await page.evaluate(() => (globalThis as any).__playsrcProfile.geometryEvidence)
    expect(geometry.geometry.samples.some((s: any) => s.disposition === "main-world" && Number.isFinite(s.depth) && s.depth > 0)).toBe(true)
    const bytes = await page.screenshot({ path: testInfo.outputPath(`${name}.png`) })
    const pixels = decodeScreenshot(bytes)
    let visible = 0
    for (let i = 0; i < pixels.pixels.length; i += pixels.channels) if (pixels.pixels[i]! + pixels.pixels[i + 1]! + pixels.pixels[i + 2]! > 72) visible++
    expect(visible).toBeGreaterThan(20_000)
    await testInfo.attach(name, { body: bytes, contentType: "image/png" })
    await json(`${name}-geometry`, geometry)
    await json(`${name}-state`, await page.evaluate(() => {
      const p = (globalThis as any).__playsrcProfile
      const d = document.querySelector<HTMLElement>("main")!.dataset
      return { points: p.controlPoints, round: p.round, player: p.player, camera: d.cameraPosition, waterPlan: d.waterPlan }
    }))
  }
  const standOnPoint = async (point: number[]) => {
    await command(`setpos ${point[0]! - 80} ${point[1]} ${point[2]}`)
    await close()
    const movement = await page.evaluate(point => {
      const d = document.querySelector<HTMLElement>("main")!.dataset
      const [x, y, z] = d.cameraPosition!.split(",").map(Number)
      const yaw = Math.atan2(point[1]! - y!, point[0]! - x!) * 180 / Math.PI
      const pitch = Math.atan2(z! - point[2]!, Math.hypot(point[0]! - x!, point[1]! - y!)) * 180 / Math.PI
      const wrap = (v: number) => ((v + 180) % 360 + 360) % 360 - 180
      return { x: wrap(Number(d.cameraYaw) - yaw) / 0.066, y: (pitch - Number(d.cameraPitch)) / 0.066 }
    }, point)
    await automation.player.lookBy(movement)
  }
  if (process.env.PROFILE_ATTACK_DEFEND_TRAINING === "1") {
    const layer = page.locator(".local-match-layer")
    await page.locator(".gameui-layer [data-vgui-name='FindAGameButton']").click()
    await page.locator(".gameui-layer [data-vgui-name='TrainingEntry'] [data-vgui-name='ModeButton']").click()
    await layer.locator("[data-vgui-name='OfflinePracticePanel'] [data-vgui-name='StartButton']").click()
    await expect(layer.locator("[data-vgui-name='GameModeLabel']")).toHaveText("Control Points")
    await layer.locator("[data-vgui-name='SelectCurrentGameModeButton']").click()
    if (map === "cp_gorge") await layer.locator("[data-vgui-name='OfflinePractice_MapSelectionPanel'] [data-vgui-name='NextButton']").click()
    await expect(layer.locator("[data-vgui-name='MapNameLabel']")).toHaveText(map === "cp_gorge" ? "Gorge" : "Dustbowl")
    await layer.locator("[data-vgui-name='OfflinePractice_MapSelectionPanel'] [data-vgui-name='DifficultyComboBox']").click()
    await page.getByRole("option", { name: "Normal", exact: true }).click()
    await layer.locator("[data-vgui-name='OfflinePractice_MapSelectionPanel'] [data-vgui-name='NumPlayersTextEntry']").fill("16")
    const path = testInfo.outputPath(`${map}-authored-offline-practice.png`)
    await page.screenshot({ path }); await testInfo.attach(`${map}-authored-offline-practice`, { path, contentType: "image/png" })
    await layer.locator("[data-vgui-name='StartOfflinePracticeButton']").click()
  } else {
    await command(`map ${map}`)
  }
  await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 60_000 })
  await close()
  await page.locator(".team-selection-layer [data-vgui-name='teambutton0']").click()
  await expect(main).toHaveAttribute("data-class-selection-visible", "true")
  await page.keyboard.press("Digit1")
  await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 30_000 })
  if (process.env.PROFILE_ATTACK_DEFEND_SMOKE === "1") {
    await capture(`${map}-initial-authored-spawn-and-hud`)
    return
  }
  if (process.env.PROFILE_ATTACK_DEFEND_WATER === "1") {
    await standOnPoint([-8288, 4176, -460])
    await page.waitForTimeout(500)
    await capture("cp_gorge-authored-ordinary-shader-water")
    await page.keyboard.down("ShiftLeft")
    try {
      await page.waitForFunction(() => document.querySelector<HTMLElement>("main")!.dataset.waterPlan?.startsWith("below"), undefined, { timeout: 5000 })
    } finally {
      await json("cp_gorge-water-duck-facts", await page.evaluate(() => {
        const d = document.querySelector<HTMLElement>("main")!.dataset, p = (globalThis as any).__playsrcProfile
        return { player: p.player, round: p.round, crouch: d.crouchFraction, waterLevel: d.waterLevel, flags: d.playerFlags, waterPlan: d.waterPlan, inWater: d.inWater }
      }))
    }
    await capture("cp_gorge-authored-underwater-material")
    await page.keyboard.up("ShiftLeft")
    return
  }
  await command("tf_bot_quota 0")
  await close()
  await page.waitForFunction(() => !(globalThis as any).__playsrcProfile.round.waitingForPlayers && (globalThis as any).__playsrcProfile.round.state === 4, undefined, { timeout: 40_000 })
  await command("ent_fire start_timer SetTime 1")
  await close()
  await page.waitForFunction(() => !(globalThis as any).__playsrcProfile.round.inSetup, undefined, { timeout: 5_000 })
  const first = map === "cp_dustbowl" ? [-216, 2792, -230] : [-6016, 4338, 80]
  await standOnPoint(first)
  if (process.env.PROFILE_ATTACK_DEFEND_STAGE_HUD === "1") {
    await command("ent_fire cap_red_1 SetOwner 3")
    await command("ent_fire cap_red_2 SetOwner 3")
    await close()
    await expect(page.locator(".hud-layer [data-vgui-name='AdvancingTeamLabel']")).toHaveText("BLU TEAM SEIZES AREA", { timeout: 5000 })
    await capture(`${map}-mini-round-advances-panel`)
    return
  }
  if (performanceOnly) {
    const samples = []
    for (const quota of [15, 23]) {
      await command(`tf_bot_quota ${quota}`)
      await close()
      await expect(main).toHaveAttribute("data-bot-count", String(quota))
      await capture(`${map}-${quota}-bots-start`)
      await page.keyboard.down("KeyW")
      const sample = await page.evaluate(async () => {
        const root = document.querySelector<HTMLElement>("main")!, profile = (globalThis as any).__playsrcProfile
        const initial = profile.bots.map((b: any) => ({ identity: b.identity, position: b.position, shots: b.shots }))
        const tick = Number(root.dataset.snapshotTick), start = performance.now(), frames: number[] = []
        const initialCamera = root.dataset.cameraPosition
        let previous = start
        await new Promise<void>(resolve => {
          const frame = (now: number) => { frames.push(now - previous); previous = now; if (now - start >= 5000) resolve(); else requestAnimationFrame(frame) }
          requestAnimationFrame(frame)
        })
        return { elapsed: (performance.now() - start) / 1000, ticks: Number(root.dataset.snapshotTick) - tick, frames, initial, bots: profile.bots, quality: profile.videoQuality, loading: root.dataset.loadPerformance, initialCamera, finalCamera: root.dataset.cameraPosition }
      })
      await page.keyboard.up("KeyW")
      await json(`${map}-${quota}-raw-sample`, sample)
      expect(sample.ticks / sample.elapsed).toBeGreaterThan(65)
      expect(sample.bots).toHaveLength(quota)
      expect(sample.bots.some((b: any) => [8, 9, 10].includes(b.objective))).toBe(true)
      expect(sample.bots.some((b: any) => sample.initial.some((p: any) => p.identity === b.identity && (b.shots > p.shots || Math.hypot(...b.position.map((v: number, i: number) => v - p.position[i])) > 1)))).toBe(true)
      const memory = await Promise.all(page.workers().map(w => Promise.race([
        w.evaluate(() => (globalThis as any).__playsrcWorkerMemory ?? null).catch(() => null),
        new Promise<null>(resolve => setTimeout(() => resolve(null), 1000)),
      ])))
      const gameplay = memory.find(m => m?.linearBytes)
      expect(gameplay).toBeTruthy()
      expect(gameplay.linearBytes).toBeLessThan(2 * 1024 ** 3)
      expect(gameplay.copiedModelSourceBytes).toBe(0)
      samples.push({ ...sample, frames: summarizeFrameTimes(sample.frames), memory })
      await capture(`${map}-${quota}-bots-finish`)
    }
    await json(`${map}-bounded-performance`, samples)
    return
  }
  const points = map === "cp_dustbowl" ? [[-216, 2792, -230], [2280, 2364, -128], [2310, -1560, 20], [-1548, -1980, -30], [-1856, 640, 18], [544, 704, 18]] : [[-6016, 4338, 80], [-6016, 1362, -64]]
  for (let index = 0; index < points.length; index++) {
    await standOnPoint(points[index]!)
    await page.waitForFunction(i => (globalThis as any).__playsrcProfile.controlPoints.points[i].progress > 0, index, { timeout: 5_000 })
    await expect(page.locator(".hud-layer [data-vgui-name='ControlPointProgressBar']")).toBeVisible()
    await capture(`${map}-point-${index}-live-capture`)
    // Gorge A is 60 authored seconds / Scout's harmonic (1 + 1/2) rate = 40s.
    await page.waitForFunction(i => (globalThis as any).__playsrcProfile.controlPoints.points[i].owner === 3, index, { timeout: 45_000 })
    await capture(`${map}-point-${index}-captured`)
    if (index % 2 === 1 && index < points.length - 1) {
      await page.waitForFunction(i => {
        const p = (globalThis as any).__playsrcProfile
        return p.round.state === 4 && p.controlPoints.points[i + 1].visible
      }, index, { timeout: 23_000 })
      await command("ent_fire start_timer SetTime 1")
      await close()
      await page.waitForFunction(() => !(globalThis as any).__playsrcProfile.round.inSetup, undefined, { timeout: 5_000 })
    }
  }
  await expect(page.locator(".hud-layer [data-vgui-name='WinningTeamLabel']")).toHaveText("BLU TEAM WINS!")
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.round.state === 3, undefined, { timeout: 20_000 })
  await expect(main).toHaveAttribute("data-team-selection-local", "2")
  await capture(`${map}-full-reset-switched-red`)
  await command("ent_fire start_timer SetTime 1")
  await close()
  await page.waitForFunction(() => !(globalThis as any).__playsrcProfile.round.inSetup, undefined, { timeout: 8_000 })
  await command("ent_fire start_timer SetTime 1")
  await close()
  await expect(page.locator(".hud-layer [data-vgui-name='WinningTeamLabel']")).toHaveText("RED TEAM WINS!", { timeout: 8_000 })
  await capture(`${map}-red-defended-time-limit`)
})
