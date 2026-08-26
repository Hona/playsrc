import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, test } from "./application-test"
import { installBrowserFrameProfiler } from "./browser-frame-profiler"
import { chooseTf2Team } from "./team-selection-evidence"
import { loadLocalConfig, repositoryRoot } from "../src/config"

const classes = [
  [1, "scout"], [2, "sniper"], [3, "soldier"], [4, "demoman"], [5, "medic"],
  [6, "heavyweapons"], [7, "pyro"], [8, "spy"], [9, "engineer"],
] as const
const effects = process.env.PROFILE_SKINNING_EFFECTS === "1"
const waterLifecycleOnly = process.env.PROFILE_SKINNING_WATER_LIFECYCLE_ONLY === "1"
const lifecycleOnly = process.env.PROFILE_SKINNING_LIFECYCLE_ONLY === "1" || waterLifecycleOnly

test("exact headed RED/BLU authored skeletal color, depth and normal transport equivalence", async ({ page }, testInfo) => {
  const errors: string[] = []
  page.on("pageerror", error => errors.push(error.message))
  page.on("console", message => {
    if (["warning", "error"].includes(message.type()) && /webgpu|GPUValidation|device.*lost|context.*lost|THREE\./i.test(message.text())) errors.push(message.text())
  })
  await page.addInitScript(installBrowserFrameProfiler)
  await page.addInitScript(() => {
    const host = globalThis as any
    host.__skinningTargets = []
    const create = host.GPUDevice.prototype.createTexture
    host.GPUDevice.prototype.createTexture = function (descriptor: any) {
      const texture = create.call(this, descriptor)
      const width = descriptor.size.width ?? descriptor.size[0]
      const height = descriptor.size.height ?? descriptor.size[1]
      if (width === 960 && height === 640 && descriptor.format === "rgba8unorm") {
        texture.label = `${texture.label || "frame-texture"}-${host.__skinningTargets.length}`
        const record = { label: texture.label, created: new Error().stack, createdPhase: document.querySelector<HTMLElement>("main")?.dataset.phase, destroyed: "", destroyedPhase: "" }
        host.__skinningTargets.push(record)
        const destroy = texture.destroy
        texture.destroy = function () {
          record.destroyed = new Error().stack ?? ""
          record.destroyedPhase = document.querySelector<HTMLElement>("main")?.dataset.phase ?? ""
          return destroy.call(this)
        }
      }
      return texture
    }
  })
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = {} })
  await page.goto("/")
  const root = page.locator("main")
  await expect(root).toHaveAttribute("data-phase", "MainMenu")
  await page.evaluate(async url => {
    const { installSkinningEvidence } = await import(/* @vite-ignore */ url)
    ;(globalThis as any).__skinningEvidence = installSkinningEvidence()
  }, `/@fs/${repositoryRoot}/packages/presentation/rendering/src/skinning-evidence.ts`)
  const command = async (value: string) => {
    console.log(`[skinning-command] ${value}`)
    if (await root.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    const entry = page.locator("[aria-label='Console command']")
    await entry.fill(value, { timeout: 5_000 })
    await entry.press("Enter", { timeout: 5_000 })
    console.log(`[skinning-command-complete] ${value}`)
  }
  const hideConsole = async () => {
    if (await root.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
  }
  await command("map ctf_2fort")
  await expect(root).toHaveAttribute("data-team-selection-visible", "true", { timeout: 60_000 })
  await hideConsole()
  await chooseTf2Team(page, "red")
  await expect(root).toHaveAttribute("data-phase", "Ready")
  // Hold locomotion only for aligned portrait cameras; animation and the
  // authoritative simulation continue at the normal cadence.
  await command("nb_stop 1")
  const { sourceCacheDir } = await loadLocalConfig()
  const directory = path.join(sourceCacheDir, "profiles", "skinning-equivalence")
  await mkdir(directory, { recursive: true })
  const screenshot = async (label: string) => {
    console.log(`[skinning-screenshot] ${label}:begin`)
    const body = await page.screenshot({ timeout: 5_000 })
    await testInfo.attach(label, { body, contentType: "image/png" })
    await writeFile(path.join(directory, `${label}.png`), body)
    console.log(`[skinning-screenshot] ${label}:complete`)
  }
  const records: any[] = []
  let waterPlan: { selection: string | null; passes: string[] } | undefined
  const capture = async (label: string, pass: string, visible = true, allowRigid = false) => {
    console.log(`[skinning-plane] ${label}/${pass}`)
    const record = await page.evaluate(async ({ label, pass, allowRigid }) => {
      return Promise.race([
        (globalThis as any).__skinningEvidence.capture(label, pass, allowRigid),
        new Promise((_, reject) => setTimeout(() => reject(new Error(`no skinned ${pass} pass for ${label}`)), 8_000)),
      ])
    }, { label, pass, allowRigid }) as any
    records.push(record)
    expect(record.planes).toHaveLength(3)
    for (const plane of record.planes) {
      expect(plane.values).toBe(record.width * record.height * 4)
      expect(plane.mismatches, `${label}/${pass}/${plane.plane}: ${JSON.stringify(plane)}`).toBe(0)
      expect(plane.referenceSha256).toBe(plane.sha256)
      if (visible && plane.plane === "color") expect(plane.actorPixels, `${label}/${pass} must have visible skinned pixels`).toBeGreaterThan(40)
      if (visible && plane.plane === "depth") expect(plane.channels[0], `${label}/${pass} must contain rendered linear depth`).toBeGreaterThan(1)
    }
  }
  const pointer = async () => {
    await hideConsole()
    await page.bringToFront()
    await page.evaluate(async () => {
      const canvas = document.querySelector<HTMLCanvasElement>(".world-canvas")!
      if (document.pointerLockElement !== canvas) await canvas.requestPointerLock()
    })
    await expect(root).toHaveAttribute("data-pointer-locked", "true", { timeout: 5_000 })
  }
  const mouseButton = async (button: number, down: boolean) => page.evaluate(({ button, down }) => {
    dispatchEvent(new MouseEvent(down ? "mousedown" : "mouseup", { button, bubbles: true }))
  }, { button, down })
  try {
    for (const [team, teamId] of [["red", 2], ["blue", 3]] as const) {
      if (lifecycleOnly) break
      await command(`jointeam ${team}`)
      await hideConsole()
      await expect(root).toHaveAttribute("data-class-selection-team", String(teamId))
      if (team === "blue") await expect(root).toHaveAttribute("data-class-selection-visible", "true")
      for (const [identity, name] of classes) {
        if (effects && ![5, 6, 7, 8, 9].includes(identity)) continue
        if (await root.getAttribute("data-class-selection-visible") !== "true") await page.keyboard.press("Comma")
        await expect(root).toHaveAttribute("data-class-selection-visible", "true")
        await page.keyboard.press(`Digit${[0, 1, 8, 2, 4, 7, 5, 3, 9, 6][identity]}`)
        await expect(root).toHaveAttribute("data-class-selection-visible", "false")
        await expect.poll(async () => Number((await root.getAttribute("data-hud-probe"))?.split(":")[1])).toBe(identity)
        if (await root.getAttribute("data-bot-count") !== "0") {
          await command("tf_bot_kick all")
          await expect(root).toHaveAttribute("data-bot-count", "0")
        }
        await command(`tf_bot_add ${team} ${name === "heavyweapons" ? "heavy" : name} easy`)
        await expect(root).toHaveAttribute("data-bot-count", "1")
        await hideConsole()
        await page.waitForFunction(({ identity, teamId }) => (globalThis as any).__playsrcProfile.bots?.some((bot: any) => bot.class === identity && bot.team === teamId && bot.lifecycle === 1), { identity, teamId }, { timeout: 5_000 })
        await page.evaluate(({ identity, teamId }) => {
          const profile = (globalThis as any).__playsrcProfile
          const bot = profile.bots.find((bot: any) => bot.class === identity && bot.team === teamId)
          const yaw = bot.yawDegrees * Math.PI / 180
          profile.displacementCameraOverride = {
            position: [bot.position[0] - Math.cos(yaw) * 100, bot.position[1] - Math.sin(yaw) * 100, bot.position[2] + 48],
            yawDegrees: bot.yawDegrees, pitchDegrees: 0,
          }
        }, { identity, teamId })
        await page.waitForFunction(() => {
          const override = (globalThis as any).__playsrcProfile.displacementCameraOverride
          return document.querySelector<HTMLElement>(".world-canvas")?.dataset.displayCameraPosition === override.position.join(",")
        }, undefined, { timeout: 5_000 })
        const label = `${team}-${name}`
        await capture(`${label}-initial`, "main")
        await page.evaluate(() => {
          const profile = (globalThis as any).__playsrcProfile
          const bot = profile.bots[0]
          const yaw = bot.yawDegrees * Math.PI / 180
          profile.displacementCameraOverride = {
            position: [bot.position[0] + Math.cos(yaw) * 64, bot.position[1] + Math.sin(yaw) * 64, bot.position[2] + 48],
            yawDegrees: bot.yawDegrees + 180, pitchDegrees: 0,
          }
        })
        await page.waitForFunction(() => document.querySelector<HTMLElement>(".world-canvas")?.dataset.displayCameraPosition === (globalThis as any).__playsrcProfile.displacementCameraOverride.position.join(","))
        await capture(`${label}-front-eyes`, "main")
        await capture(label, "viewmodel")
        await capture(label, "hud-model")
        await screenshot(label)
        await page.evaluate(() => { delete (globalThis as any).__playsrcProfile.displacementCameraOverride })
        if (name === "heavyweapons" || name === "engineer") {
          await hideConsole()
          await page.keyboard.press("Digit3")
          await expect.poll(async () => Number((await root.getAttribute("data-hud-probe"))?.split(":")[2])).toBe(name === "heavyweapons" ? 11 : 42)
          await hideConsole()
          await capture(`${label}-melee-tool`, "viewmodel")
        }
        if (effects && name === "pyro") {
          await pointer()
          await mouseButton(0, true)
          try {
            await expect.poll(async () => Number(await root.getAttribute("data-flame-points"))).toBeGreaterThan(0)
            await capture(`${label}-flames`, "viewmodel")
            await screenshot(`${label}-flames`)
          } finally { await mouseButton(0, false) }
        }
        if (effects && name === "spy") {
          await pointer()
          await mouseButton(2, true)
          await mouseButton(2, false)
          await expect.poll(async () => Number((await root.getAttribute("data-spy-probe"))?.split(":")[1])).toBeGreaterThan(0.25)
          await capture(`${label}-cloak-watch`, "viewmodel", false)
          await screenshot(`${label}-cloak`)
          await mouseButton(2, true)
          await mouseButton(2, false)
          await hideConsole()
          await page.keyboard.press("Digit4")
          await expect(page.locator("[data-vgui-name='HudMenuSpyDisguise']")).toBeVisible()
          await page.keyboard.press("Digit2")
          await expect.poll(async () => (await root.getAttribute("data-spy-probe"))?.split(":").slice(2, 4).join(":"), { timeout: 10_000 }).toBe(`3:${teamId === 2 ? 3 : 2}`)
          await capture(`${label}-disguise`, "viewmodel")
          await capture(`${label}-disguise`, "hud-model")
          await screenshot(`${label}-disguise`)
        }
        if (effects && name === "engineer") {
          await page.keyboard.press("Digit4")
          await expect(root).toHaveAttribute("data-engineer-menu", "build")
          await page.keyboard.press("Digit1")
          await expect(root).toHaveAttribute("data-placement", /^2:/)
          await capture(`${label}-blueprint`, "main", true, true)
          await capture(`${label}-builder`, "viewmodel")
          await screenshot(`${label}-blueprint`)
          await hideConsole()
          await page.keyboard.press("Digit1")
        }
        if (effects && name === "medic") {
          const patient = await page.evaluate(() => {
            const bot = (globalThis as any).__playsrcProfile.bots[0]
            const yaw = bot.yawDegrees * Math.PI / 180
            return [bot.position[0] - Math.cos(yaw) * 100, bot.position[1] - Math.sin(yaw) * 100, bot.position[2]] as number[]
          })
          if (await root.getAttribute("data-movement-mode") !== "1") await command("noclip")
          await command(`setpos ${patient.join(" ")}`)
          await hideConsole()
          await page.keyboard.press("Digit2")
          await expect.poll(async () => Number((await root.getAttribute("data-hud-probe"))?.split(":")[2])).toBe(20)
          await pointer()
          await page.evaluate(() => {
            const host = globalThis as any
            host.__skinningAim = true
            const aim = () => {
              if (!host.__skinningAim) return
              const patient = host.__playsrcProfile.bots[0].position
              const root = document.querySelector<HTMLElement>("main")!
              const camera = root.dataset.cameraPosition!.split(",").map(Number)
              const dx = patient[0]! - camera[0]!, dy = patient[1]! - camera[1]!, dz = patient[2]! + 41 - camera[2]!
              let yaw = Math.atan2(dy, dx) * 180 / Math.PI - Number(root.dataset.cameraYaw)
              while (yaw > 180) yaw -= 360
              while (yaw < -180) yaw += 360
              const pitch = -Math.atan2(dz, Math.hypot(dx, dy)) * 180 / Math.PI - Number(root.dataset.cameraPitch)
              dispatchEvent(new MouseEvent("mousemove", { movementX: Math.round(-yaw / 0.066), movementY: Math.round(pitch / 0.066), bubbles: true }))
              requestAnimationFrame(aim)
            }
            aim()
          })
          await mouseButton(0, true)
          try {
            await expect.poll(async () => Number(await root.getAttribute("data-medigun-target")), { timeout: 5_000 }).toBeGreaterThan(0)
            await capture(`${label}-beam`, "main")
            await capture(`${label}-beam-muzzle`, "viewmodel")
            await screenshot(`${label}-beam`)
          } finally {
            await mouseButton(0, false)
            await page.evaluate(() => { (globalThis as any).__skinningAim = false })
          }
        }
      }
    }
    if (effects && !lifecycleOnly || waterLifecycleOnly) {
      await command("nb_stop 0")
      await command(`tf_bot_add ${23 - Number(await root.getAttribute("data-bot-count"))} normal`)
      await expect(root).toHaveAttribute("data-bot-count", "23")
      if (await root.getAttribute("data-movement-mode") !== "1") await command("noclip")
      await command("setpos 523 -439 250")
      await hideConsole()
      await page.evaluate(() => {
        const profile = (globalThis as any).__playsrcProfile
        const water = profile.materialAnimation.volumes.find((volume: any) => volume.surfaceZ === -180)
        if (!water) throw new Error("authored 2Fort water volume is unavailable")
        const leaves = new Set(water.leaves)
        const sample = profile.coverageSamples.filter((sample: any) => leaves.has(sample.leaf) && sample.position[2] < water.surfaceZ - 12 && sample.position[2] > water.minimumZ)
          .sort((a: any, b: any) => Math.hypot(a.position[0], a.position[1]) - Math.hypot(b.position[0], b.position[1]))[0]
        if (!sample) throw new Error("authored 2Fort underwater camera coverage is unavailable")
        profile.displacementCameraOverride = { position: [sample.position[0], sample.position[1], water.surfaceZ + 40], yawDegrees: 90, pitchDegrees: 60 }
      })
      const tick = Number(await root.getAttribute("data-snapshot-tick"))
      await page.waitForFunction(first => Number(document.querySelector<HTMLElement>("main")?.dataset.snapshotTick) >= first + 333, tick)
      waterPlan = { selection: await root.getAttribute("data-water-plan"), passes: (await root.getAttribute("data-water-passes") ?? "").split(",") }
      expect(waterPlan.passes).toContain("refraction")
      if (waterPlan.passes.includes("reflection")) await capture("23-bots-water-reflection", "reflection", false, true)
      else expect(waterPlan.selection).toMatch(/^above:expensive:0:1:/)
      await capture("23-bots-water-refraction", "refraction", false, true)
      await capture("23-bots-water-main", "main", false, true)
      await screenshot("23-bots-water")
      await page.evaluate(() => { delete (globalThis as any).__playsrcProfile.displacementCameraOverride })
    }
    await hideConsole()
    await page.setViewportSize({ width: 960, height: 640 })
    await capture("resized-model", "viewmodel")
    await command("map pl_upward")
    await expect(root).toHaveAttribute("data-team-selection-visible", "true", { timeout: 60_000 })
    await hideConsole()
    await chooseTf2Team(page, "red")
    await expect.poll(async () => Number((await root.getAttribute("data-hud-probe"))?.split(":")[1])).toBe(3)
    await capture("replacement-soldier", "viewmodel")
    expect(errors).toEqual([])
  } finally {
    const terminal = await page.evaluate(() => ({
      phase: document.querySelector<HTMLElement>("main")?.dataset.phase,
      hud: document.querySelector<HTMLElement>("main")?.dataset.hudProbe,
      bots: (globalThis as any).__playsrcProfile.bots,
      console: document.querySelector<HTMLElement>("[aria-label='Console output']")?.innerText,
      targets: (globalThis as any).__skinningTargets,
    })).catch(() => null)
    console.log(`[skinning-terminal] ${JSON.stringify(terminal)}`)
    try { await page.evaluate(() => (globalThis as any).__skinningEvidence.dispose()) }
    catch (error) { errors.push(String(error)) }
    await writeFile(path.join(directory, lifecycleOnly ? "lifecycle.json" : effects ? "effects.json" : "classes.json"), JSON.stringify({ scope: lifecycleOnly ? "lifecycle-only" : effects ? "effects-red-blu" : "all-nine-red-blu", tolerance: "exact GPU scalar equality; identical shaders, target and pose, native full-palette transport reference; positive linear view depth and encoded view normals", records, errors, terminal, waterPlan }, null, 2))
  }
})
