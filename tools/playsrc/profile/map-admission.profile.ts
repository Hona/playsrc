import { test, expect } from "./application-test"
import { chooseTf2Team } from "./team-selection-evidence"
import { headedProfileTarget } from "./profile-target"
import { summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"
import { readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { spawnSync } from "node:child_process"
import { tf2MapBsp, tf2MapMode } from "@playsrc/game-tf2-browser/maps"
import { loadLocalConfig } from "../src/config"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"

const json = (value: unknown) => JSON.stringify(value, (_, value) => typeof value === "bigint" ? value.toString() : value)

test.afterEach(async ({ page }, testInfo) => {
  if (testInfo.status === testInfo.expectedStatus || page.isClosed()) return
  const evidence = await page.evaluate(() => ({ failure: (globalThis as any).__playsrcProfile?.failure,
    legacyVisuals: (globalThis as any).__playsrcProfile?.legacyVisualEvidence,
    frames: (globalThis as any).__playsrcFrameProfiler?.completedFrames,
    simulation: (globalThis as any).__playsrcFrameProfiler?.simulation,
    dataset: { ...document.querySelector<HTMLElement>("main")?.dataset } })).catch(() => null)
  await writeFile(testInfo.outputPath("map-failure.json"), json(evidence))
})

test("configured map native traversal, objective roster, visible geometry and cadence", async ({ page }, testInfo) => {
  const target = headedProfileTarget(process.env, "cp_badlands")
  const config = await loadLocalConfig()
  const facts = JSON.parse(await readFile(path.join(config.sourceCacheDir, "evidence/map-runtime", `${target}.facts.json`), "utf8"))
  expect(facts.bspSha256).toBe(tf2MapBsp(target).sha256)
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = {} })
  await page.addInitScript(installBrowserFrameProfiler)
  const main = page.locator("main")
  const errors: string[] = []
  page.on("pageerror", error => errors.push(error.message))
  const command = async (value: string) => {
    if (await main.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    const entry = page.locator("[aria-label='Console command']")
    await entry.fill(value); await entry.press("Enter")
  }
  const closeConsole = async () => { if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote") }
  let revision = 0
  const spawnChecks: unknown[] = []
  const waitPlayer = async (field: "team" | "class", value: number) => {
    try {
      await page.waitForFunction(({ field, value }) => (globalThis as any).__playsrcProfile.player?.[field] === value, { field, value }, { timeout: 5000 })
    } catch (error) {
      const state = await page.evaluate(() => ({ player: (globalThis as any).__playsrcProfile.player, dataset: { ...document.querySelector<HTMLElement>("main")!.dataset }, console: document.querySelector<HTMLElement>("[aria-label='Console output']")?.innerText }))
      await writeFile(testInfo.outputPath(`${target}-spawn-failure.json`), JSON.stringify(state))
      throw error
    }
  }
  const checkSpawn = async (team: number, label: string) => {
    await waitPlayer("team", team)
    const player = await page.evaluate(() => (globalThis as any).__playsrcProfile.player)
    const candidates = facts.spawns.filter((spawn: any) => Number(spawn.team) === team && Math.hypot(spawn.position[0] - player.position[0], spawn.position[1] - player.position[1]) < 1 && Math.abs(spawn.position[2] - player.position[2]) < 128)
    expect(candidates.length, `${target}:${label} authored spawn position`).toBeGreaterThan(0)
    const yawError = (left: number, right: number) => Math.abs(((left - right + 540) % 360) - 180)
    expect(candidates.some((spawn: any) => yawError(player.camera.yawDegrees, spawn.angles[1]) < 0.001 && Math.abs(player.camera.pitchDegrees - spawn.angles[0]) < 0.001), `${target}:${label} authored spawn angles`).toBe(true)
    spawnChecks.push({ label, player, candidates })
  }
  const capture = async (label: string) => {
    await closeConsole()
    const selected = ++revision
    await page.evaluate(revision => { (globalThis as any).__playsrcProfile.geometryEvidenceRevision = revision }, selected)
    await page.waitForFunction(revision => (globalThis as any).__playsrcProfile.geometryEvidence?.revision === revision, selected)
    const geometry = await page.evaluate(() => (globalThis as any).__playsrcProfile.geometryEvidence)
    const imagePath = testInfo.outputPath(`${target}-${label}.png`)
    await page.screenshot({ path: imagePath })
    if (process.platform === "darwin" && label === "spawn") {
      const desktopPath = testInfo.outputPath(`${target}-spawn-desktop.png`)
      const desktop = spawnSync("/usr/sbin/screencapture", ["-x", desktopPath])
      if (desktop.status !== 0) throw new Error("Native visible desktop capture failed")
      await testInfo.attach("native-desktop", { path: desktopPath, contentType: "image/png" })
    }
    const image = decodeScreenshot(await page.locator("canvas.world-canvas").screenshot({ path: testInfo.outputPath(`${target}-${label}-world.png`) }))
    const depth = geometry.geometry.samples.filter((sample: any) => sample.family !== null && Number.isFinite(sample.depth) && sample.depth > 0).map((sample: any) => {
      const x = Math.max(0, Math.min(image.width - 1, Math.round((sample.x + 1) * image.width / 2)))
      const y = Math.max(0, Math.min(image.height - 1, Math.round((1 - sample.y) * image.height / 2)))
      const offset = (y * image.width + x) * image.channels
      return { ...sample, rgb: [...image.pixels.subarray(offset, offset + 3)] }
    })
    expect(depth.some((sample: any) => sample.disposition === "main-world" && sample.rgb.some((channel: number) => channel > 3))).toBe(true)
    const facts = await page.evaluate(() => ({ points: (globalThis as any).__playsrcProfile.controlPoints, bots: (globalThis as any).__playsrcProfile.bots, round: (globalThis as any).__playsrcProfile.round }))
    const dataPath = testInfo.outputPath(`${target}-${label}.json`)
    await writeFile(dataPath, JSON.stringify({ geometry, depth, facts }))
    await testInfo.attach(label, { path: imagePath, contentType: "image/png" })
    await testInfo.attach(`${label}-depth`, { path: dataPath, contentType: "application/json" })
  }
  await page.goto("/")
  await page.bringToFront()
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 60_000 })
  await command(`map ${target}`)
  await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 60_000 })
  await closeConsole(); await chooseTf2Team(page, "red")
  await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 30_000 })
  if (process.env.PROFILE_MAP_LEGACY_GLOW === "1") {
    await page.waitForFunction(() => (globalThis as any).__playsrcProfile.player?.camera)
    const camera = await page.evaluate(() => (globalThis as any).__playsrcProfile.player.camera)
    const candidates = facts.legacyVisuals.filter((entity: any) => entity.classname === "env_lightglow").map((entity: any) => {
      const delta = camera.position.map((value: number,axis: number)=>value-entity.position[axis])
      const pitch=entity.angles[0]*Math.PI/180,yaw=entity.angles[1]*Math.PI/180
      const facing=delta[0]*Math.cos(pitch)*Math.cos(yaw)+delta[1]*Math.cos(pitch)*Math.sin(yaw)-delta[2]*Math.sin(pitch)
      return { ...entity,distance:Math.hypot(...delta),facing }
    }).filter((entity: any)=>entity.distance>Number(entity.minimumDistance??0)
      && (!(Number(entity.spawnflags)&1)||entity.facing>=0)
      && (Number(entity.outerMaximumDistance??0)<=Number(entity.maximumDistance??0)||entity.distance<Number(entity.outerMaximumDistance)))
      .sort((left: any,right: any)=>left.distance-right.distance)
    const glow=candidates[0]
    expect(glow).toBeTruthy()
    const glowPitch=glow.angles[0]*Math.PI/180,glowYaw=glow.angles[1]*Math.PI/180
    camera.position=glow.position.map((value:number,axis:number)=>value+[
      128*Math.cos(glowPitch)*Math.cos(glowYaw)+64*Math.sin(glowYaw),
      128*Math.cos(glowPitch)*Math.sin(glowYaw)-64*Math.cos(glowYaw),
      -128*Math.sin(glowPitch),
    ][axis]!)
    const delta=glow.position.map((value: number,axis: number)=>value-camera.position[axis])
    camera.yawDegrees=Math.atan2(delta[1],delta[0])*180/Math.PI
    camera.pitchDegrees=-Math.atan2(delta[2],Math.hypot(delta[0],delta[1]))*180/Math.PI
    await writeFile(testInfo.outputPath(`${target}-legacy-glow-camera.json`),json({glow,camera,candidates}))
    await page.evaluate(camera=>{const profile=(globalThis as any).__playsrcProfile;profile.legacyVisualProbe=true;profile.displacementCameraOverride=camera},camera)
    await page.waitForFunction(source=>(globalThis as any).__playsrcProfile.legacyVisualEvidence?.[0]?.quads.some((quad: any)=>quad.source===source),glow.identity,{timeout:10000})
    const beforeState=await page.evaluate(()=>(globalThis as any).__playsrcProfile.legacyVisualEvidence)
    const before=decodeScreenshot(await page.locator("canvas.world-canvas").screenshot({path:testInfo.outputPath(`${target}-legacy-glow-on.png`)}))
    await command('ent_fire env_lightglow Color "0 0 0"');await closeConsole()
    await page.waitForFunction(()=>(globalThis as any).__playsrcProfile.legacyVisualEvidence?.[0]?.quads.length===0,undefined,{timeout:5000})
    const after=decodeScreenshot(await page.locator("canvas.world-canvas").screenshot({path:testInfo.outputPath(`${target}-legacy-glow-off.png`)}))
    let changed=0
    for(let y=Math.floor(before.height/2)-64;y<before.height/2+64;y++)for(let x=Math.floor(before.width/2)-64;x<before.width/2+64;x++){
      const at=(y*before.width+x)*before.channels
      if([0,1,2].some(channel=>before.pixels[at+channel]!==after.pixels[at+channel]))changed++
    }
    await writeFile(testInfo.outputPath(`${target}-legacy-glow-pixels.json`),json({glow,camera,changed,beforeState,afterState:await page.evaluate(()=>(globalThis as any).__playsrcProfile.legacyVisualEvidence)}))
    expect(changed,"authored Color input removes visible glow pixels in the fixed central ROI").toBeGreaterThan(8)
    expect(beforeState[0].queries.some((query: any)=>query.source===glow.identity&&query.possible>0)).toBe(true)
    expect(errors).toEqual([])
    return
  }
  if (process.env.PROFILE_MAP_SKY_PARTICLE) {
    await page.waitForFunction(() => (globalThis as any).__playsrcProfile.controlPoints?.points.length > 0)
    const emitter = facts.particleSystems.find((value: any) => value.name === process.env.PROFILE_MAP_SKY_PARTICLE)
    expect(emitter).toBeTruthy()
    const sky = facts.skyCameras[0]
    expect(sky).toBeTruthy()
    const point = await page.evaluate(() => (globalThis as any).__playsrcProfile.controlPoints.points.find((point: any) => point.owner === 0).position)
    const position = [point[0], point[1], point[2] + 768]
    const targetPosition = emitter.position.map((value: number, axis: number) => (value - sky.position[axis]) * Number(sky.scale))
    const delta = targetPosition.map((value: number, axis: number) => value - position[axis]!)
    const yawDegrees = Math.atan2(delta[1], delta[0]) * 180 / Math.PI
    const pitchDegrees = -Math.atan2(delta[2], Math.hypot(delta[0], delta[1])) * 180 / Math.PI
    await page.evaluate(camera => { (globalThis as any).__playsrcProfile.displacementCameraOverride = camera }, { position, yawDegrees, pitchDegrees })
    await capture("sky-particle-visible")
    await page.evaluate(() => { (globalThis as any).__playsrcProfile.particleEvidenceRevision = 1 })
    await page.waitForFunction(() => (globalThis as any).__playsrcProfile.particleEvidence?.revision === 1)
    const particles = await page.evaluate(() => (globalThis as any).__playsrcProfile.particleEvidence)
    const skyItems = particles.items.filter((item: any) => item.sky)
    expect(skyItems.length).toBeGreaterThan(0)
    const worldPixels = async (revision: number, name: string) => {
      await page.evaluate(revision => { (globalThis as any).__playsrcProfile.hudPixelEvidenceRevision = revision }, revision)
      await page.waitForFunction(revision => (globalThis as any).__playsrcProfile.hudPixelEvidence?.revision === revision, revision)
      const bytes = Buffer.from(await page.evaluate(() => Array.from((globalThis as any).__playsrcProfile.hudPixelEvidence.before.bytes) as number[]))
      await writeFile(testInfo.outputPath(name), bytes)
      return decodeScreenshot(bytes)
    }
    const before = await worldPixels(1, "sky-particle-before.png")
    if (process.env.PROFILE_MAP_SKY_PARTICLE_DEBUG === "1") {
      const profiler = await page.evaluate(() => ({ uses: (globalThis as any).__playsrcFrameProfiler.firstParticleUses,
        preparation: (globalThis as any).__playsrcFrameProfiler.particlePreparation }))
      await writeFile(testInfo.outputPath("sky-particle-debug.json"), json({ particles, profiler }))
      return
    }
    const lifetime = Math.max(...skyItems.map((item: any) => item.lifetimeSeconds))
    expect(lifetime).toBeLessThanOrEqual(30)
    await command(`ent_fire ${emitter.name} Stop`); await closeConsole()
    await page.waitForTimeout((lifetime + 0.2) * 1000)
    await capture("sky-particle-stopped")
    const after = await worldPixels(2, "sky-particle-after.png")
    let changed = 0
    for (let y = before.height / 4; y < before.height / 2; y++) for (let x = before.width * 3 / 8; x < before.width * 5 / 8; x++) {
      const offset = (y * before.width + x) * before.channels
      if ([0, 1, 2].some(channel => Math.abs(before.pixels[offset + channel]! - after.pixels[offset + channel]!) > 3)) changed++
    }
    await writeFile(testInfo.outputPath("sky-particle-evidence.json"), json({ emitter, sky, particles, changed, lifetime }))
    expect(changed).toBeGreaterThan(10)
    expect(errors).toEqual([])
    return
  }
  await checkSpawn(2, "red-initial")
  await command("joinclass scout"); await closeConsole()
  await waitPlayer("class", 1)
  await checkSpawn(2, "red-class-respawn")
  await command("joinclass soldier"); await closeConsole()
  await waitPlayer("class", 3)
  await command("jointeam blue")
  await command("joinclass soldier"); await closeConsole()
  await checkSpawn(3, "blue-join")
  await command("joinclass scout"); await closeConsole()
  await waitPlayer("class", 1)
  await checkSpawn(3, "blue-class-respawn")
  await capture("blue-spawn")
  await command("jointeam red")
  await command("joinclass soldier"); await closeConsole()
  await waitPlayer("class", 3)
  await checkSpawn(2, "red-return")
  if (process.env.PROFILE_SPAWN_ONLY === "1") {
    await capture("red-spawn")
    await writeFile(testInfo.outputPath(`${target}-spawn-checks.json`), JSON.stringify({ target, spawnChecks }))
    return
  }
  await command("tf_bot_quota 15"); await closeConsole()
  await expect(main).toHaveAttribute("data-bot-count", "15")
  await capture("spawn")
  const before = await page.evaluate(() => (globalThis as any).__playsrcProfile.player)
  await page.locator("canvas.world-canvas").click({ force: true })
  await expect(main).toHaveAttribute("data-pointer-locked", "true")
  await page.keyboard.down("w"); await page.waitForTimeout(1000); await page.keyboard.up("w")
  const after = await page.evaluate(() => (globalThis as any).__playsrcProfile.player)
  const yaw = before.camera.yawDegrees * Math.PI / 180
  const forwardDistance = (after.position[0] - before.position[0]) * Math.cos(yaw) + (after.position[1] - before.position[1]) * Math.sin(yaw)
  expect(forwardDistance, "authored forward input moves along the selected spawn facing").toBeGreaterThan(16)
  await page.waitForFunction(() => !(globalThis as any).__playsrcProfile.round.waitingForPlayers
    && (globalThis as any).__playsrcProfile.round.state === 4, undefined, { timeout: 40_000 })
  const points = await page.evaluate(() => (globalThis as any).__playsrcProfile.controlPoints.points.map((point: any) => ({ identity: point.identity, position: point.position, owner: point.owner })))
  expect(points).toHaveLength(tf2MapMode(target) === "king-of-the-hill" ? 1 : 5)
  if (tf2MapMode(target) === "king-of-the-hill") await command("ent_fire team_control_point SetUnlockTime 1")
  const point = points.find((point: any) => point.owner === 0) ?? points[Math.floor(points.length / 2)]
  await command(`setpos ${point.position[0]} ${point.position[1]} ${point.position[2] + 8}`)
  await closeConsole()
  const sample = await page.evaluate(async () => {
    const root = document.querySelector<HTMLElement>("main")!, profile = (globalThis as any).__playsrcProfile
    const profiler = (globalThis as any).__playsrcFrameProfiler
    profiler.completedFrames.length = 0; profiler.active = true
    const start = performance.now(), tick = Number(root.dataset.snapshotTick)
    const before = profile.bots.map((bot: any) => ({ identity: bot.identity, area: bot.area, position: bot.position }))
    const frames: number[] = []; let previous = start
    await new Promise<void>(resolve => {
      const frame = (now: number) => { frames.push(now - previous); previous = now; if (now - start >= 5000) resolve(); else requestAnimationFrame(frame) }
      requestAnimationFrame(frame)
    })
    profiler.active = false
    return { seconds: (performance.now() - start) / 1000, ticks: Number(root.dataset.snapshotTick) - tick, frames, before, bots: profile.bots, points: profile.controlPoints.points,
      completedFrames: profiler.completedFrames, counters: profiler.counters, nodeBuilds: profiler.nodeBuilds,
      simulation: profiler.simulation, memoryAssets: profile.memoryAssets, failures: profile.failure }
  })
  const resultPath = testInfo.outputPath(`${target}-acceptance.json`)
  await writeFile(resultPath, json({ target, errors, spawnChecks, ...sample, frames: summarizeFrameTimes(sample.frames) }))
  await testInfo.attach("map-acceptance", { path: resultPath, contentType: "application/json" })
  expect(sample.bots).toHaveLength(15)
  expect(sample.bots.every((bot: any) => bot.area !== null)).toBe(true)
  expect(sample.bots.some((bot: any) => sample.before.some((prior: any) => prior.identity === bot.identity && Math.hypot(...bot.position.map((value: number, axis: number) => value - prior.position[axis])) > 32))).toBe(true)
  // Retain the failure, but still exercise capture/lifecycle gates so a cold-view
  // pipeline hitch cannot hide an independent gameplay admission failure.
  expect.soft(sample.ticks / sample.seconds).toBeGreaterThan(63)
  await capture("objective")
  // Exercise a real bot capture, not a point-owner input or a local-player cap.
  // This happens after the unchanged cadence window; the seeded bots still run
  // their own objective AI and must enter the actual authored capture brush.
  const botCapture = await page.evaluate(() => {
    const profile = (globalThis as any).__playsrcProfile
    const point = profile.controlPoints.points.find((point: any) => !point.locked
      && ((point.owner !== 2 && point.mayCapture[0]) || (point.owner !== 3 && point.mayCapture[1])))
    if (!point) throw new Error("No capturable authored point remains for bot acceptance")
    const team = point.owner !== 2 && point.mayCapture[0] ? 2 : 3
    const roster = JSON.parse(document.querySelector<HTMLElement>("main")!.dataset.scoreboardProbe!).players
    return { point, team, camera: profile.player.camera,
      bots: profile.bots.filter((bot: any) => bot.team === team && bot.health > 0).slice(0, 3)
        .map((bot: any) => ({ identity: bot.identity, captures: bot.captures, position: bot.position,
          name: roster.find((player: any) => player.identity === bot.identity).name })) }
  })
  expect(botCapture.bots.length).toBeGreaterThan(0)
  const home = (spawnChecks[0] as any).player.position
  await command(`setpos ${home.join(" ")}`)
  for (const bot of botCapture.bots) await command(`bot_teleport ${JSON.stringify(bot.name)} ${botCapture.point.position[0]} ${botCapture.point.position[1]} ${botCapture.point.position[2] + 8} 0 90 0`)
  await closeConsole()
  await page.evaluate(camera => { (globalThis as any).__playsrcProfile.displacementCameraOverride = camera }, botCapture.camera)
  const botPath = testInfo.outputPath(`${target}-bot-capture-state.json`)
  await writeFile(botPath, json(botCapture))
  await page.waitForFunction(({ point, team, bots }) => {
    const profile = (globalThis as any).__playsrcProfile
    return profile.controlPoints.points.find((candidate: any) => candidate.identity === point.identity)?.owner === team
      && bots.some((before: any) => profile.bots.some((bot: any) => bot.identity === before.identity && bot.captures > before.captures))
  }, botCapture, { timeout: 20_000 })
  const captured = await page.evaluate(() => ({ points: (globalThis as any).__playsrcProfile.controlPoints, bots: (globalThis as any).__playsrcProfile.bots }))
  await writeFile(botPath, json({ ...botCapture, captured }))
  await capture("bot-capture")
  await page.evaluate(() => { delete (globalThis as any).__playsrcProfile.displacementCameraOverride })
  for (const [index, point] of points.entries()) {
    await command(`setpos ${point.position[0]} ${point.position[1]} ${point.position[2] + 8}`)
    await closeConsole(); await page.waitForTimeout(300)
    await capture(`point-${index}`)
  }
  expect(errors).toEqual([])
})
