import { expect, test } from "./application-test"
import { settleTf2Gameplay } from "./team-selection-evidence"
import { decodeScreenshot } from "./screenshot-pixels"
import { writeFile } from "node:fs/promises"
import { summarizeFrameTimes } from "./profile-window"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"
import { scopePixelOracle } from "./scope-pixel-oracle"

function centerChange(before: ReturnType<typeof decodeScreenshot>, after: ReturnType<typeof decodeScreenshot>): number {
  let changed = 0, count = 0
  for (let y = Math.floor(before.height * .4); y < before.height * .6; y++) {
    for (let x = Math.floor(before.width * .4); x < before.width * .48; x++) {
      const a = (y * before.width + x) * before.channels, b = (y * after.width + x) * after.channels
      if ([0, 1, 2].reduce((sum, channel) => sum + Math.abs(before.pixels[a + channel]! - after.pixels[b + channel]!), 0) > 12) changed++
      count++
    }
  }
  return changed / count
}

function opaqueExterior(image: ReturnType<typeof decodeScreenshot>): void {
  for (const [x, y] of [[4, 4], [image.width - 5, 4], [4, Math.trunc(image.height / 3)], [image.width - 5, Math.trunc(image.height / 3)]]) {
    const offset = (y! * image.width + x!) * image.channels
    expect([...image.pixels.slice(offset, offset + 3)], `opaque exterior at ${x},${y}`).toEqual([0, 0, 0])
  }
}

test("Sniper Mouse2 retains real world pixels through the authored Refract scope", async ({ page }, testInfo) => {
  const errors: string[] = []
  page.on("pageerror", error => errors.push(error.message))
  page.on("console", message => {
    if (message.type() === "error" && !message.location().url.endsWith("/favicon.ico")) errors.push(`${message.text()} ${message.location().url}`)
  })
  await page.addInitScript(installBrowserFrameProfiler)
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = {} })
  const root = page.locator("main")
  const cdp = await page.context().newCDPSession(page)
  await cdp.send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 720, deviceScaleFactor: 1, mobile: false })
  // Playwright's screenshot helper restores the context's launch DPR. Capture
  // the headed compositor directly so fractional-DPR resize tests stay live.
  const screenshot = async (options?: { path: string }) => {
    const result = await cdp.send("Page.captureScreenshot", { format: "png", fromSurface: true, captureBeyondViewport: false })
    const bytes = Buffer.from(result.data, "base64")
    if (options) await writeFile(options.path, bytes)
    return bytes
  }
  const canvasPixels = async (name: string) => {
    const data = await page.locator(".world-canvas").evaluate((canvas: HTMLCanvasElement) => canvas.toDataURL())
    const bytes = Buffer.from(data.split(",")[1]!, "base64")
    await writeFile(testInfo.outputPath(`${name}-canvas.png`), bytes)
    return decodeScreenshot(bytes)
  }
  const captureRejections: object[] = []
  const capturePointer = async () => {
    await page.bringToFront()
    for (let attempt = 0; attempt < 3; attempt++) {
      if (await root.getAttribute("data-pointer-locked") !== "true") await page.locator(".world-canvas").click()
      try { await expect(root).toHaveAttribute("data-pointer-locked", "true", { timeout: 750 }); return }
      catch {
        const state = await root.evaluate(element => ({ detail: (element as HTMLElement).dataset.detail, focus: document.hasFocus(), visibility: document.visibilityState, locked: Boolean(document.pointerLockElement) }))
        if (!state.detail?.includes("Too many pointer lock requests") || attempt === 2) throw new Error(`Native scope capture failed: ${JSON.stringify(state)}`)
        captureRejections.push(state)
        // Respect the browser's real unlock/relock policy outside sampling.
        // Never replace capture or input with synthetic event dispatch.
        await page.waitForTimeout(2100)
      }
    }
  }
  await page.goto("/")
  await expect(root).toHaveAttribute("data-phase", "MainMenu", { timeout: 60_000 })
  const entry = page.locator("[aria-label='Console command']")
  const command = async (value: string) => {
    if (await root.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    await entry.fill(value)
    await entry.press("Enter")
    const output = await page.locator("[aria-label='Console output']").innerText()
    expect(output.split("\n").at(-1), value).not.toMatch(/unknown|rejected|invalid/i)
  }
  await command("map ctf_2fort")
  await settleTf2Gameplay(page, "red")
  await command("joinclass sniper")
  await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[2]).toBe("12")
  await command("noclip")
  await expect(root).toHaveAttribute("data-movement-mode", "1")
  for (let index = 0; index < 15; index++) {
    await command(`tf_bot_add ${index % 2 ? "blue" : "red"} ${index === 0 ? "heavy" : "soldier"} normal`)
    await expect(root).toHaveAttribute("data-bot-count", String(index + 1))
  }
  console.log("[sniper] 15 live bots admitted")
  const target = await page.evaluate(() => (globalThis as any).__playsrcProfile.bots.find((bot: any) => bot.team === 2 && bot.class === 6 && bot.lifecycle === 1))
  expect(target).toBeTruthy()
  const angle = target.yawDegrees * Math.PI / 180
  await command(`setpos ${target.position[0] - Math.cos(angle) * 180} ${target.position[1] - Math.sin(angle) * 180} ${target.position[2]}`)
  await page.keyboard.press("Backquote")
  await capturePointer()
  console.log("[sniper] native mouse captured")
  let pointerX = 640, pointerY = 360
  const aim = async (point?: readonly [number, number, number]) => {
    for (let attempt = 0; attempt < 6; attempt++) {
      const angles = await page.evaluate(({ identity, point }) => {
      const bot = (globalThis as any).__playsrcProfile.bots.find((bot: any) => bot.identity === identity)
      const main = document.querySelector<HTMLElement>("main")!
      const camera = main.dataset.cameraPosition!.split(",").map(Number)
      const target = point ?? [bot.position[0], bot.position[1], bot.position[2] + 48]
      const dx = target[0] - camera[0], dy = target[1] - camera[1], dz = target[2] - camera[2]
      const yaw = Math.atan2(dy, dx) * 180 / Math.PI, pitch = -Math.atan2(dz, Math.hypot(dx, dy)) * 180 / Math.PI
      return { dx: -(((yaw - Number(main.dataset.cameraYaw) + 180) % 360 + 360) % 360 - 180) / .066, dy: (pitch - Number(main.dataset.cameraPitch)) / .066, yaw, pitch }
      }, { identity: target.identity, point })
      if (Math.abs(angles.dx) < 10 && Math.abs(angles.dy) < 10) return
      const revision = Number(await page.locator(".world-canvas").getAttribute("data-display-mouse-revision"))
      pointerX += angles.dx; pointerY += angles.dy
      await page.mouse.move(pointerX, pointerY)
      await page.waitForFunction(({ revision, pitch, yaw }) => {
        const root = document.querySelector<HTMLElement>("main")!
        return Number(document.querySelector<HTMLElement>(".world-canvas")?.dataset.displayMouseRevision) > revision
          || Math.abs(Number(root.dataset.cameraPitch) - pitch) < 1 && Math.abs(((Number(root.dataset.cameraYaw) - yaw + 180) % 360 + 360) % 360 - 180) < 1
      }, { revision, pitch: angles.pitch, yaw: angles.yaw }, { timeout: 2000 }).catch(async error => {
        console.log("[sniper] native aim state", { attempt, revision, angles, pointerX, pointerY, state: await root.evaluate(element => {
          const data = (element as HTMLElement).dataset
          return { phase: data.phase, pointerLocked: data.pointerLocked, yaw: data.cameraYaw, pitch: data.cameraPitch }
        }) })
        throw error
      })
      const error = await root.evaluate((element, angles) => ({ pitch: Math.abs(Number((element as HTMLElement).dataset.cameraPitch) - angles.pitch), yaw: Math.abs(((Number((element as HTMLElement).dataset.cameraYaw) - angles.yaw + 180) % 360 + 360) % 360 - 180) }), angles)
      if (error.pitch < 1 && error.yaw < 1) return
    }
    throw new Error("Native mouse failed to converge on the live bot after a viewport change")
  }
  await aim()
  console.log("[sniper] aimed at Heavy")
  await screenshot({ path: testInfo.outputPath("unscoped-composited.png") })
  await page.mouse.down({ button: "right" }); await page.mouse.up({ button: "right" })
  const scope = page.locator("[data-tf2-scope='authored']")
  await expect(scope).toBeVisible()
  await expect.poll(async () => Number(await page.locator("[data-tf2-scope-charge]").getAttribute("data-charge"))).toBe(1)
  await aim()
  await page.evaluate(() => { (globalThis as any).__playsrcProfile.hudPixelEvidenceRevision = 1 })
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.hudPixelEvidence?.revision === 1)
  const witness = await page.evaluate(() => {
    const evidence = (globalThis as any).__playsrcProfile.hudPixelEvidence
    return { before: Array.from(evidence.before.bytes) as number[], after: Array.from(evidence.after.bytes) as number[], quality: (globalThis as any).__playsrcProfile.videoQuality }
  })
  const worldBytes = Buffer.from(witness.before), scopeBytes = Buffer.from(witness.after)
  await writeFile(testInfo.outputPath("same-frame-world.png"), worldBytes)
  await writeFile(testInfo.outputPath("same-frame-scope.png"), scopeBytes)
  const oracle = scopePixelOracle(decodeScreenshot(worldBytes), decodeScreenshot(scopeBytes), witness.quality)
  await writeFile(testInfo.outputPath("pixel-oracle.json"), JSON.stringify(oracle, null, 2))
  expect(oracle.maximumDifference, "Source material RGB must match actual framebuffer samples within 8-bit readback/filter rounding").toBeLessThanOrEqual(3)
  await page.evaluate(() => { (globalThis as any).__playsrcProfile.worldLightingEvidenceRevision = 1 })
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.worldLighting?.revision === 1)
  const depth = await page.evaluate(() => {
    const profile = (globalThis as any).__playsrcProfile
    return profile.worldLighting.worldGeometry.samples.flatMap((sample: any) => {
      const bot = profile.bots.find((bot: any) => bot.lifecycle === 1 && sample.identity === 0x60000000 + bot.identity)
      return bot ? [{ ...sample, bot: { identity: bot.identity, class: bot.class, team: bot.team, position: bot.position } }] : []
    })
  })
  console.log("SNIPER_BOT_DEPTH", depth)
  expect(depth.length, "live player targets must have visible scene-depth samples through scope").toBeGreaterThan(0)
  expect(depth.every((sample: any) => sample.modelDepth > 0 && (sample.worldDepth === null || sample.modelDepth <= sample.worldDepth))).toBe(true)
  await writeFile(testInfo.outputPath("visible-bot-depth.json"), JSON.stringify(depth, null, 2))
  const composited = await screenshot({ path: testInfo.outputPath("scoped-composited.png") })
  const canvas = Buffer.from((await page.locator(".world-canvas").evaluate((canvas: HTMLCanvasElement) => canvas.toDataURL())).split(",")[1]!, "base64")
  await writeFile(testInfo.outputPath("scoped-canvas.png"), canvas)
  pointerX += 200
  await page.mouse.move(pointerX, pointerY)
  await expect.poll(async () => page.locator(".world-canvas").getAttribute("data-display-mouse-revision")).not.toBe(null)
  const moved = decodeScreenshot(await screenshot({ path: testInfo.outputPath("scoped-moved-composited.png") }))
  const image = decodeScreenshot(composited)
  opaqueExterior(image)
  let redTargetPixels = 0
  for (let y = Math.floor(image.height * .3); y < image.height * .65; y++) {
    for (let x = Math.floor(image.width * .3); x < image.width * .6; x++) {
      const i = (y * image.width + x) * image.channels
      if (image.pixels[i]! > 50 && image.pixels[i]! > image.pixels[i + 1]! * 1.4 && image.pixels[i]! > image.pixels[i + 2]! * 1.4) redTargetPixels++
    }
  }
  expect(redTargetPixels, "identifiable RED player pixels, not only a scope widget or geometry record").toBeGreaterThan(100)
  const colors = new Set<string>()
  let changed = 0, count = 0
  for (let y = Math.floor(image.height * .4); y < image.height * .6; y++) {
    for (let x = Math.floor(image.width * .4); x < image.width * .48; x++) {
      const offset = (y * image.width + x) * image.channels
      colors.add(Array.from(image.pixels.slice(offset, offset + 3)).join(","))
      if (Math.abs(image.pixels[offset]! - moved.pixels[offset]!) + Math.abs(image.pixels[offset + 1]! - moved.pixels[offset + 1]!) + Math.abs(image.pixels[offset + 2]! - moved.pixels[offset + 2]!) > 12) changed++
      count++
    }
  }
  console.log("SNIPER_SCOPE_CENTER", { colors: colors.size, changed, count })
  expect(changed / count).toBeGreaterThan(.1)
  const matrix: object[] = []
  let revision = 0
  for (const dpr of [1, 1.25, 1.5, 2]) {
    for (const [width, height] of [[1280, 720], [801, 721], [390, 844]]) {
      await page.setViewportSize({ width: width!, height: height! })
      await cdp.send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: dpr, mobile: false })
      await page.waitForFunction(({ width, height, dpr }) => {
        const canvas = document.querySelector<HTMLCanvasElement>(".world-canvas")!
        return canvas.width === Math.floor(width * dpr) && canvas.height === Math.floor(height * dpr)
      }, { width: width!, height: height!, dpr })
      if (await root.getAttribute("data-pointer-locked") !== "true") {
        await capturePointer()
        pointerX = width! / 2; pointerY = height! / 2
      }
      await expect(root).toHaveAttribute("data-pointer-locked", "true")
      await aim()
      const before = decodeScreenshot(await screenshot({ path: testInfo.outputPath(`scope-${width}x${height}-dpr${dpr}.png`) }))
      const raw = await canvasPixels(`scope-${width}x${height}-dpr${dpr}`)
      expect(raw.width).toBe(Math.floor(width! * dpr))
      expect(raw.height).toBe(Math.floor(height! * dpr))
      // The compositor rounds fractional device extents independently of the
      // renderer's integer drawing buffer. HUD panels may cover the exterior.
      expect(before.width).toBeGreaterThanOrEqual(raw.width)
      expect(before.width).toBeLessThanOrEqual(Math.ceil(width! * dpr))
      expect(before.height).toBeGreaterThanOrEqual(raw.height)
      expect(before.height).toBeLessThanOrEqual(Math.ceil(height! * dpr))
      opaqueExterior(raw)
      await page.evaluate(revision => { (globalThis as any).__playsrcProfile.geometryEvidenceRevision = revision }, ++revision)
      await page.waitForFunction(revision => (globalThis as any).__playsrcProfile.geometryEvidence?.revision === revision, revision)
      const geometry = await page.evaluate(() => (globalThis as any).__playsrcProfile.geometryEvidence)
      expect(geometry.camera.verticalFovDegrees).toBeCloseTo(2 * Math.atan(Math.tan(10 * Math.PI / 180) * .75) * 180 / Math.PI, 4)
      const sceneDepth = geometry.geometry.samples.filter((sample: any) => sample.depth !== null && sample.depth > 0)
      expect(sceneDepth.length, "scope zoom must retain real scene geometry and depth").toBeGreaterThan(0)
      pointerX += 150
      await page.mouse.move(pointerX, pointerY)
      const after = decodeScreenshot(await screenshot())
      const change = centerChange(before, after)
      expect(change, `${width}x${height} DPR ${dpr} retains moving world contrast`).toBeGreaterThan(.05)
      matrix.push({ width, height, dpr, change, sceneDepth: sceneDepth.length, fov: geometry.camera.verticalFovDegrees })
    }
  }
  await page.setViewportSize({ width: 1280, height: 720 })
  await cdp.send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 720, deviceScaleFactor: 1, mobile: false })
  await writeFile(testInfo.outputPath("viewport-matrix.json"), JSON.stringify(matrix, null, 2))
  await aim()
  const fired = Number(await root.getAttribute("data-fire-events"))
  await page.mouse.down({ button: "left" })
  await expect.poll(async () => Number(await root.getAttribute("data-fire-events"))).toBeGreaterThan(fired)
  await page.mouse.up({ button: "left" })
  await page.keyboard.press("KeyR")
  await expect(scope).not.toBeVisible()
  await screenshot({ path: testInfo.outputPath("charged-shot.png") })
  for (let index = 15; index < 23; index++) {
    await command(`tf_bot_add ${index % 2 ? "blue" : "red"} soldier normal`)
    await expect(root).toHaveAttribute("data-bot-count", String(index + 1))
  }
  await page.keyboard.press("Backquote")
  await capturePointer()
  pointerX = 640; pointerY = 360
  await expect(root).toHaveAttribute("data-pointer-locked", "true")
  await page.keyboard.press("Digit1")
  await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[2]).toBe("12")
  if (!await scope.isVisible()) { await page.mouse.down({ button: "right" }); await expect(scope).toBeVisible(); await page.mouse.up({ button: "right" }) }
  await aim()
  opaqueExterior(decodeScreenshot(await screenshot({ path: testInfo.outputPath("23-bots-scoped.png") })))
  if (process.env.PROFILE_SCOPE_SAMPLE === "1") {
      const measurement = await page.evaluate(async () => {
      const profiler = (globalThis as any).__playsrcFrameProfiler
      profiler.completedFrames.length = 0
      profiler.active = true
      const root = document.querySelector<HTMLElement>("main")!
      const started = performance.now(), firstTick = Number(root.dataset.snapshotTick)
      const resources = performance.getEntriesByType("resource").length
      const memoryBefore = (performance as any).memory?.usedJSHeapSize ?? null
      const frames: number[] = [], work: unknown[] = []
      let previous: number | undefined
      await new Promise<void>(resolve => {
        const frame = (now: number) => {
          // The first RAF timestamp may precede the task that armed this
          // sample. It establishes the baseline, not a negative frame interval.
          if (previous !== undefined) frames.push(now - previous)
          previous = now
          if (root.dataset.performanceDetail) work.push(JSON.parse(root.dataset.performanceDetail))
          if (now - started >= 5000) resolve(); else requestAnimationFrame(frame)
        }
        requestAnimationFrame(frame)
      })
      profiler.active = false
      return { frames, work, rendererFrames: profiler.completedFrames, losses: profiler.losses, seconds: (performance.now() - started) / 1000, ticks: Number(root.dataset.snapshotTick) - firstTick,
        resources: performance.getEntriesByType("resource").length - resources, memoryBefore, memoryAfter: (performance as any).memory?.usedJSHeapSize ?? null }
    })
    await writeFile(testInfo.outputPath("measurement.json"), JSON.stringify(measurement, null, 2))
    await writeFile(testInfo.outputPath("profile.json"), JSON.stringify({ ...measurement, frameTimes: summarizeFrameTimes(measurement.frames) }, null, 2))
    expect(measurement.ticks / measurement.seconds).toBeGreaterThan(64)
    expect(measurement.ticks / measurement.seconds).toBeLessThan(69)
    expect(measurement.resources).toBe(0)
    expect(measurement.losses).toEqual([])
    expect(measurement.rendererFrames.length).toBeGreaterThan(60)
    const passes = measurement.rendererFrames.flatMap((frame: any) => frame.renderer?.passes.filter((pass: any) => pass.identity === "hud-materials") ?? [])
    expect(passes.length).toBeGreaterThan(60)
    expect(passes.every((pass: any) => pass.drawCalls === 8 && pass.renderPipelines === 0 && pass.nodeBuilderMisses === 0)).toBe(true)
  }
  await page.keyboard.press("Escape")
  await expect(root).toHaveAttribute("data-gameui", "pause")
  await screenshot({ path: testInfo.outputPath("paused-scope.png") })
  await page.keyboard.press("Escape")
  await expect(root).toHaveAttribute("data-gameui", "in-game")
  await page.keyboard.press("Digit2")
  await expect(scope).not.toBeVisible()
  await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[2]).toBe("13")
  await page.keyboard.press("Comma")
  await expect(root).toHaveAttribute("data-class-selection-visible", "true")
  await page.keyboard.press("Digit2")
  await expect(root).toHaveAttribute("data-class-selection-visible", "false")
  await expect(scope).not.toBeVisible()
  for (const map of ["pl_upward", "jump_beef"]) {
    const generation = Number(await root.getAttribute("data-generation"))
    await command(`map ${map}`)
    await expect.poll(async () => Number(await root.getAttribute("data-generation"))).toBeGreaterThan(generation)
    await settleTf2Gameplay(page, "red")
    await expect(scope).not.toBeVisible()
    await command("joinclass sniper")
    await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[2]).toBe("12")
    if (map === "pl_upward") {
      if (await root.getAttribute("data-movement-mode") !== "1") await command("noclip")
      await expect(root).toHaveAttribute("data-movement-mode", "1")
      await command("setpos -1674 -1536 82")
      for (let index = 0; index < 15; index++) {
        await command(`tf_bot_add ${index % 2 ? "red" : "blue"} soldier normal`)
        await expect(root).toHaveAttribute("data-bot-count", String(index + 1))
      }
    }
    await page.keyboard.press("Backquote")
    if (await root.getAttribute("data-pointer-locked") !== "true") {
      await capturePointer()
      pointerX = 640; pointerY = 360
    }
    await expect(root).toHaveAttribute("data-pointer-locked", "true")
    if (map === "pl_upward") await aim([-1000, -1536, -75])
    await page.mouse.down({ button: "right" }); await expect(scope).toBeVisible(); await page.mouse.up({ button: "right" })
    await page.evaluate(revision => { (globalThis as any).__playsrcProfile.geometryEvidenceRevision = revision }, ++revision)
    await page.waitForFunction(revision => (globalThis as any).__playsrcProfile.geometryEvidence?.revision === revision, revision)
    expect(await page.evaluate(() => (globalThis as any).__playsrcProfile.geometryEvidence.target)).toBe(map)
    const before = decodeScreenshot(await screenshot({ path: testInfo.outputPath(`${map}-scoped.png`) }))
    opaqueExterior(before)
    pointerX += 150
    await page.mouse.move(pointerX, pointerY)
    const after = decodeScreenshot(await screenshot({ path: testInfo.outputPath(`${map}-scoped-moved.png`) }))
    expect(centerChange(before, after), `${map} scope retains moving world contrast after replacement`).toBeGreaterThan(.05)
    await page.mouse.down({ button: "right" }); await expect(scope).not.toBeVisible(); await page.mouse.up({ button: "right" })
  }
  expect(errors).toEqual([])
  expect(await page.evaluate(() => (globalThis as any).__playsrcFrameProfiler.losses)).toEqual([])
  await writeFile(testInfo.outputPath("native-capture-rejections.json"), JSON.stringify(captureRejections, null, 2))
})
