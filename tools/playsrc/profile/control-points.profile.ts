import { expect, test } from "./application-test"
import { chooseTf2Team } from "./team-selection-evidence"
import { decodeScreenshot } from "./screenshot-pixels"
import { writeFile } from "node:fs/promises"
import { Tf2BrowserAutomation } from "../../../apps/web/tf2/src/browser-automation"
import { summarizeFrameTimes } from "./profile-window"

test("headed Badlands control point world, capture HUD and moving local bot roster", async ({ page }, testInfo) => {
  test.skip(process.env.PROFILE_CP_FULL_MATCH === "1" || process.env.PROFILE_CP_CONTEST === "1")
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = {} })
  const errors: string[] = []
  page.on("pageerror", error => errors.push(error.message))
  const main = page.locator("main")
  await page.goto("/")
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 60_000 })
  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  await entry.fill("map cp_badlands"); await entry.press("Enter")
  await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 60_000 })
  if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  await chooseTf2Team(page, "red")
  await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 30_000 })
  const hud = page.locator(".hud-layer [data-vgui-name='HudControlPointIcons']")
  await expect(hud).toBeVisible()
  await expect(hud.locator("[data-vgui-name='BaseImage']")).toHaveCount(5)
  if (await main.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
  await entry.fill("tf_bot_quota 15"); await entry.press("Enter")
  await page.keyboard.press("Backquote")
  await expect(main).toHaveAttribute("data-bot-count", "15")
  await page.waitForFunction(() => { const round=(globalThis as any).__playsrcProfile.round; return round && !round.waitingForPlayers && round.state===4 }, undefined, { timeout: 40000 })
  await page.screenshot({ path: testInfo.outputPath("badlands-initial-world-and-hud.png") })
  const before = await page.evaluate(() => (globalThis as any).__playsrcProfile.bots.map((bot: any) => ({ identity: bot.identity, position: bot.position })))
  const sample = await page.evaluate(async () => {
    const root=document.querySelector<HTMLElement>("main")!, tick=Number(root.dataset.snapshotTick), start=performance.now()
    const frames:number[]=[]; let previous=start
    await new Promise<void>(resolve=>{const frame=(now:number)=>{frames.push(now-previous);previous=now;if(now-start>=5000)resolve();else requestAnimationFrame(frame)};requestAnimationFrame(frame)})
    return { frames, elapsed:(performance.now()-start)/1000, ticks:Number(root.dataset.snapshotTick)-tick, quality:(globalThis as any).__playsrcProfile.videoQuality }
  })
  expect(sample.ticks/sample.elapsed).toBeGreaterThan(63)
  const after = await page.evaluate(() => (globalThis as any).__playsrcProfile.bots.map((bot: any) => ({ identity: bot.identity, position: bot.position, objective: bot.objective })))
  const workers = await Promise.all(page.workers().map(worker => Promise.race([
    worker.evaluate(() => ({ heapBytes:(performance as any).memory?.usedJSHeapSize??null, memory:(globalThis as any).__playsrcWorkerMemory??null })).catch(() => null),
    new Promise<null>(resolve => setTimeout(() => resolve(null),1000)),
  ])))
  const memory=workers.find(worker=>worker?.memory)?.memory
  expect(memory).toBeTruthy()
  expect(memory.copiedModelSourceBytes).toBe(0)
  await writeFile(testInfo.outputPath("badlands-motion-facts.json"), JSON.stringify({ before, after, errors, sample:{...sample,frames:summarizeFrameTimes(sample.frames)},workers }))
  expect(after.length).toBeGreaterThan(0)
  expect(after.some((bot: any) => before.some((prior: any) => bot.identity === prior.identity && Math.hypot(...bot.position.map((value: number, i: number) => value - prior.position[i])) > 32))).toBe(true)
  const screenshot = await page.screenshot({ path: testInfo.outputPath("badlands-world-and-five-point-hud.png") })
  const image = decodeScreenshot(screenshot)
  let visible = 0
  for (let i = 0; i < image.pixels.length; i += image.channels) if (image.pixels[i]! + image.pixels[i + 1]! + image.pixels[i + 2]! > 72) visible++
  expect(visible).toBeGreaterThan(20_000)
  await testInfo.attach("badlands-world-and-five-point-hud", { body: screenshot, contentType: "image/png" })
  await testInfo.attach("badlands-bot-motion", { body: JSON.stringify({ before, after, errors }), contentType: "application/json" })
  expect(errors).toEqual([])
})

test("headed full Badlands round uses walking capture bots, real local input, point victory and map restart", async ({ page }, testInfo) => {
  test.skip(process.env.PROFILE_CP_FULL_MATCH !== "1" && process.env.PROFILE_CP_CONTEST !== "1")
  const contestOnly = process.env.PROFILE_CP_CONTEST === "1"
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = {} })
  const main = page.locator("main")
  const automation = new Tf2BrowserAutomation({
    evaluate: async <T>(expression: string): Promise<T> => page.evaluate(expression) as Promise<T>,
    press: key => page.keyboard.press(key), click: selector => page.locator(selector).click(),
    focus: selector => page.locator(selector).focus(), fill: (selector, value) => page.locator(selector).fill(value),
    waitFor: async (expression, timeout) => { await page.waitForFunction(expression, undefined, { timeout }) },
    activateCurrentTab: () => page.bringToFront(),
  })
  await page.goto("/")
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 60_000 })
  const command = async (text: string) => {
    if (await main.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    const input = page.locator("[aria-label='Console command']")
    await input.fill(text); await input.press("Enter")
    const last = (await page.locator("[aria-label='Console output']").innerText()).split("\n").at(-1) ?? ""
    if (/rejected|invalid|unknown|unavailable/iu.test(last)) throw new Error(`${text}: ${last}`)
  }
  const closeConsole = async () => { if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote") }
  await command("map cp_badlands")
  await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 60_000 })
  await closeConsole(); await chooseTf2Team(page, "red")
  await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 30_000 })
  await command(contestOnly ? "tf_bot_quota 0" : "tf_bot_add 15 red scout hard")
  await closeConsole()
  await expect(main).toHaveAttribute("data-bot-count", contestOnly ? "0" : "15")
  await page.waitForFunction(() => { const r=(globalThis as any).__playsrcProfile.round; return r && !r.waitingForPlayers && r.state===4 }, undefined, { timeout: 40_000 })
  const initial = await page.evaluate(() => ({ bots: (globalThis as any).__playsrcProfile.bots, points: (globalThis as any).__playsrcProfile.controlPoints }))
  // Stage the camera outside the real authored trigger, then cross its boundary
  // with ordinary movement. Every bot starts at a real authored spawn and walks.
  await command("setpos -330 0 500")
  await page.waitForFunction(() => { const position=document.querySelector<HTMLElement>("main")!.dataset.cameraPosition!.split(",").map(Number); return Math.abs(position[0]!+330)<1 && Math.abs(position[1]!)<1 })
  await closeConsole()
  const look = await page.evaluate(() => { const d=document.querySelector<HTMLElement>("main")!.dataset; return { x: Number(d.cameraYaw)/0.066, y: -Number(d.cameraPitch)/0.066 } })
  if (Math.abs(look.x) + Math.abs(look.y) > 0.001) await automation.player.lookBy(look)
  else await page.locator("canvas.world-canvas").click()
  await page.keyboard.press("Digit3")
  await page.keyboard.down("KeyW"); await page.waitForTimeout(1200); await page.keyboard.up("KeyW")
  await writeFile(testInfo.outputPath("badlands-local-input.json"), JSON.stringify(await page.evaluate(() => ({ dataset: {...document.querySelector<HTMLElement>("main")!.dataset}, points:(globalThis as any).__playsrcProfile.controlPoints }))))
  await page.screenshot({ path: testInfo.outputPath("badlands-local-input.png") })
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.controlPoints.localPoint===2, undefined, { timeout: 5000 })
  await expect(page.locator(".hud-layer [data-vgui-name='ControlPointProgressBar']")).toBeVisible()
  await page.evaluate(() => { (globalThis as any).__playsrcProfile.geometryEvidenceRevision = 1 })
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.geometryEvidence?.revision === 1)
  const geometry = await page.evaluate(() => (globalThis as any).__playsrcProfile.geometryEvidence)
  expect(geometry.geometry.samples.some((sample:any)=>sample.family!==null&&Number.isFinite(sample.depth)&&sample.depth>0)).toBe(true)
  await writeFile(testInfo.outputPath("badlands-capture-world-depth.json"), JSON.stringify(geometry))
  await page.screenshot({ path: testInfo.outputPath("badlands-local-walk-capture-hud.png") })
  if (contestOnly) {
    await page.waitForFunction(() => (globalThis as any).__playsrcProfile.controlPoints.points[2].progress > 0.1, undefined, { timeout: 5000 })
    await page.keyboard.down("KeyD"); await page.waitForTimeout(1400); await page.keyboard.up("KeyD")
    await page.waitForFunction(() => (globalThis as any).__playsrcProfile.controlPoints.localPoint === null, undefined, { timeout: 5000 })
    const abandoned = await page.evaluate(() => (globalThis as any).__playsrcProfile.controlPoints.points[2])
    await page.waitForTimeout(3000)
    const decayed = await page.evaluate(() => (globalThis as any).__playsrcProfile.controlPoints.points[2])
    expect(decayed.progress).toBeLessThan(abandoned.progress)
    await writeFile(testInfo.outputPath("badlands-abandoned-and-decayed.json"), JSON.stringify({ abandoned, decayed }))
    await command("setpos -330 0 500")
    await page.waitForFunction(() => Math.abs(Number(document.querySelector<HTMLElement>("main")!.dataset.cameraPosition!.split(",")[0])+330)<1, undefined, { timeout: 5000 })
    await closeConsole()
    const returnLook = await page.evaluate(() => { const d=document.querySelector<HTMLElement>("main")!.dataset; return { x:Number(d.cameraYaw)/0.066,y:-Number(d.cameraPitch)/0.066 } })
    if (Math.abs(returnLook.x)+Math.abs(returnLook.y)>0.001) await automation.player.lookBy(returnLook)
    else await page.locator("canvas.world-canvas").click()
    await page.keyboard.down("KeyW"); await page.waitForTimeout(1200); await page.keyboard.up("KeyW")
    await page.waitForFunction(() => (globalThis as any).__playsrcProfile.controlPoints.localPoint === 2, undefined, { timeout: 5000 })
    await command("tf_bot_add 1 blue heavy hard")
    await expect(main).toHaveAttribute("data-bot-count", "1")
    const name = await page.evaluate(() => JSON.parse(document.querySelector<HTMLElement>("main")!.dataset.scoreboardProbe!).players.find((player:any) => player.identity !== 1).name)
    // This isolated contested-contact check stages an active opposing bot.
    // The complete match above separately requires every cap to have walking bots.
    await command(`bot_teleport "${name}" 100 150 330 0 180 0`)
    await closeConsole()
    await page.waitForFunction(() => (globalThis as any).__playsrcProfile.controlPoints.points[2].blocked, undefined, { timeout: 5000 })
    const contested = await page.evaluate(() => (globalThis as any).__playsrcProfile.controlPoints.points[2])
    expect(contested.owner).toBe(0)
    await page.screenshot({ path: testInfo.outputPath("badlands-real-opponent-contested-hud.png") })
    await writeFile(testInfo.outputPath("badlands-contest-and-decay.json"), JSON.stringify({ abandoned, decayed, contested }))
    return
  }
  const match = await page.evaluate(async () => {
    const profile=(globalThis as any).__playsrcProfile, start=performance.now()
    const timeline: any[]=[]; let sampled=0
    await new Promise<void>(resolve=>{
      const frame=(now:number)=>{
        if(now-sampled>=100){ sampled=now; timeline.push({ now:now-start, round:profile.round, points:profile.controlPoints, bots:profile.bots.map((b:any)=>({identity:b.identity,position:b.position,area:b.area,objective:b.objective,captures:b.captures})) }) }
        if(profile.round.state===5||now-start>=90000||document.querySelector<HTMLElement>("main")!.dataset.phase==="Failed")resolve();else requestAnimationFrame(frame)
      };requestAnimationFrame(frame)
    })
    return { timeline, round:profile.round, points:profile.controlPoints }
  })
  await writeFile(testInfo.outputPath("badlands-full-round.json"), JSON.stringify({ initial, match }))
  await page.screenshot({ path: testInfo.outputPath("badlands-full-round-end.png") })
  expect(match.round.winningTeam).toBe(2)
  expect(match.points.points.map((p:any)=>p.owner)).toEqual([2,2,2,2,2])
  for(const point of [2,1,0]) expect(match.timeline.some(t=>t.points.points[point].touching.some((id:number)=>id!==1))).toBe(true)
  await expect(page.locator(".hud-layer [data-vgui-name='WinningTeamLabel']")).toHaveText("RED TEAM WINS!")
  await expect(page.locator(".hud-layer [data-vgui-name='WinReasonLabel']")).toContainText("captured all")
  await page.waitForFunction(()=>{const r=(globalThis as any).__playsrcProfile.round;return r.state===3&&r.roundsPlayed===1},undefined,{timeout:20000})
  await page.screenshot({ path: testInfo.outputPath("badlands-round-restart.png") })
  expect(await page.evaluate(()=>(globalThis as any).__playsrcProfile.controlPoints.points.map((p:any)=>p.owner))).toEqual([3,3,0,2,2])
})
