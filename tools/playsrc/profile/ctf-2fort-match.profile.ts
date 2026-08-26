import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
import { expect, test } from "./application-test"
import { profileSampleSeconds, summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"
import { chooseTf2Team } from "./team-selection-evidence"

const RED_FLAG = [-488.66, 3348.51, -170] as const
const BLUE_CAPTURE = [500.3447, -3366, -170] as const
const CLASSES = Object.freeze([
  ["scout", 1, "blue", 4],
  ["sniper", 2, "red", 12],
  ["soldier", 3, "blue", 1],
  ["demoman", 4, "red", 18],
  ["medic", 5, "blue", 19],
  ["heavy", 6, "red", 9],
  ["pyro", 7, "blue", 15],
  ["spy", 8, "red", 50],
  ["engineer", 9, "blue", 40],
] as const)

test("headed Create Server completes a nine-class 2Fort bot match with Source doors, combat, intelligence return, and BLU victory", async ({ page }, testInfo) => {
  await page.addInitScript(() => {
    const profile = { objectiveEvents: [] as string[], entityTransitions: [] as string[] }
    ;(globalThis as typeof globalThis & { __playsrcProfile?: object }).__playsrcProfile = profile
    const install = () => {
      const root = document.querySelector<HTMLElement>("main")
      if (!root) { setTimeout(install, 0); return }
      new MutationObserver(records => {
        for (const record of records) {
          if (record.attributeName === "data-ctf-events" && root.dataset.ctfEvents) {
            profile.objectiveEvents.push(...root.dataset.ctfEvents.split("|"))
          }
          if (record.attributeName === "data-entity-trace" && root.dataset.entityTrace) {
            const values = root.dataset.entityTrace.split(":").map(Number)
            if (values[0]! > 0 || values[2]! > 0) profile.entityTransitions.push(root.dataset.entityTrace)
          }
        }
      }).observe(root, { attributes: true, attributeFilter: ["data-ctf-events", "data-entity-trace"] })
    }
    if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", install, { once: true })
    else install()
  })

  const root = page.locator("main")
  await page.goto("/")
  await expect(root).toHaveAttribute("data-phase", "MainMenu", { timeout: 120_000 })
  await page.locator(".gameui-layer [data-vgui-name='FindAGameButton']").click()
  await page.locator(".gameui-layer [data-vgui-name='CreateServerEntry'] [data-vgui-name='ModeButton']").click()
  const dialog = page.locator(".local-match-layer").getByRole("dialog", { name: "CREATE SERVER" })
  await expect(dialog).toBeVisible()
  await dialog.locator("[data-vgui-name='MapList']").click()
  await page.getByRole("option", { name: "ctf_2fort" }).click()
  await dialog.getByRole("tab", { name: "GAME" }).click()
  const settings = dialog.locator("[data-vgui-name='GameplayPage']")
  await settings.locator("[data-vgui-name='DifficultyComboBox']").click()
  await page.getByRole("option", { name: "Expert" }).click()
  await settings.locator("[data-vgui-name='NumPlayersTextEntry']").fill("0")
  const launchPixels = await page.screenshot()
  await testInfo.attach("headed-2fort-authored-create-server", { body: launchPixels, contentType: "image/png" })
  await dialog.getByRole("button", { name: "Start" }).click()
  await expect(root).toHaveAttribute("data-team-selection-visible", "true", { timeout: 600_000 })
  await chooseTf2Team(page, "red")
  await expect(root).toHaveAttribute("data-phase", "Ready", { timeout: 120_000 })
  await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[1]).toBe("3")

  await page.keyboard.press("Backquote")
  const entry = page.locator("[aria-label='Console command']")
  const command = async (value: string): Promise<void> => {
    await entry.fill(value)
    await entry.press("Enter")
    const output = await page.locator("[aria-label='Console output']").innerText()
    const latest = output.split("\n").at(-1) ?? ""
    if (/rejected|invalid|unknown|unavailable|earlier bot command/i.test(latest)) {
      throw new Error(`${value}: ${latest}`)
    }
  }
  await command("tf_flag_caps_per_round 1")
  await expect(root).toHaveAttribute("data-ctf", /^0:0:1:0:/)
  await command("tf_flag_return_on_touch 1")

  for (const [index, [name,, team]] of CLASSES.entries()) {
    await command(`tf_bot_add ${team} ${name} expert`)
    await expect(root).toHaveAttribute("data-bot-count", String(index + 1), { timeout: 30_000 })
  }
  await page.waitForFunction(() => {
    const bots = (globalThis as any).__playsrcProfile?.bots
    return bots?.length === 9 && bots.every((bot: any) => bot.area !== null && bot.remainingPathAreas > 0)
  }, undefined, { timeout: 30_000 })
  const initial = await page.evaluate(() => structuredClone((globalThis as any).__playsrcProfile.bots))
  expect(initial.map((bot: any) => [bot.class, bot.team, bot.weapon?.identity ?? null]))
    .toEqual(CLASSES.map(([, identity, team, weapon]) => [identity, team === "red" ? 2 : 3, weapon]))
  expect(initial.filter((bot: any) => bot.team === 2).every((bot: any) => bot.position[1] > 0)).toBe(true)
  expect(initial.filter((bot: any) => bot.team === 3).every((bot: any) => bot.position[1] < 0)).toBe(true)
  console.log("[2fort-match] all nine stock classes spawned on authored RED/BLU routes")

  const navigation = await page.evaluate(async (starting) => {
    const profile = (globalThis as any).__playsrcProfile
    const started = performance.now()
    const origins = new Map(starting.map((bot: any) => [bot.identity, { position: bot.position, area: bot.area }]))
    const progress = new Map<number, { team: number; distance: number; areas: Set<number> }>()
    while (performance.now() - started < 3500) {
      const root = document.querySelector<HTMLElement>("main")!
      if (root.dataset.phase === "Failed") throw new Error(root.dataset.detail)
      for (const bot of profile.bots ?? []) {
        const origin: any = origins.get(bot.identity)
        if (!origin) continue
        const previous = progress.get(bot.identity) ?? { team: bot.team, distance: 0, areas: new Set([origin.area]) }
        previous.distance = Math.max(previous.distance, Math.hypot(...bot.position.map((value: number, axis: number) => value - origin.position[axis])))
        if (bot.area !== null) previous.areas.add(bot.area)
        progress.set(bot.identity, previous)
      }
      await new Promise<void>(resolve => requestAnimationFrame(() => resolve()))
    }
    return {
      elapsedMilliseconds: Math.round(performance.now() - started),
      bots: [...progress].map(([identity, value]) => ({ identity, team: value.team, distance: Number(value.distance.toFixed(2)), areas: value.areas.size })),
      entityTransitions: profile.entityTransitions.length,
    }
  }, initial)
  expect(navigation.bots.filter(bot => bot.team === 2).some(bot => bot.distance > 100 && bot.areas > 1)).toBe(true)
  expect(navigation.bots.filter(bot => bot.team === 3).some(bot => bot.distance > 100 && bot.areas > 1)).toBe(true)
  expect(navigation.entityTransitions).toBeGreaterThan(0)

  await page.keyboard.press("Backquote")
  await page.keyboard.down("Tab")
  await expect(root).toHaveAttribute("data-scoreboard-visible", "true")
  const liveScoreboard = JSON.parse(await root.getAttribute("data-scoreboard-probe") ?? "{}")
  expect(liveScoreboard.red.playerCount).toBe(5)
  expect(liveScoreboard.blue.playerCount).toBe(5)
  await expect(page.locator(".hud-layer [data-vgui-name='mapname']")).toHaveText("ctf_2fort")
  const scoreboardPixels = await page.screenshot()
  await testInfo.attach("headed-2fort-nine-class-live-scoreboard", { body: scoreboardPixels, contentType: "image/png" })
  await page.keyboard.up("Tab")
  await page.keyboard.press("Backquote")

  const scout = await page.evaluate(() => {
    const profile = (globalThis as any).__playsrcProfile
    const bot = profile.bots.find((value: any) => value.team === 3 && value.class === 1 && value.lifecycle === 1)
    const score = profile.combat.scores.find((value: any) => value.identity === bot?.identity)
    if (!bot || !score) throw new Error("the live BLU Scout is absent")
    return { identity: bot.identity, name: score.name as string, position: bot.position as [number, number, number] }
  })
  await command(`setpos ${scout.position[0]} ${scout.position[1]} ${scout.position[2]}`)
  await expect.poll(async () => {
    const scoreboard = JSON.parse(await root.getAttribute("data-scoreboard-probe") ?? "{}")
    return scoreboard.players?.find((player: any) => player.identity === 1)?.deaths ?? 0
  }, { timeout: 20_000 }).toBeGreaterThan(0)
  const death = JSON.parse(await root.getAttribute("data-scoreboard-probe") ?? "{}")
  expect(death.players.some((player: any) => player.team === 3 && player.kills > 0)).toBe(true)
  await expect.poll(async () => {
    const scoreboard = JSON.parse(await root.getAttribute("data-scoreboard-probe") ?? "{}")
    return scoreboard.players?.find((player: any) => player.identity === 1)?.alive ?? false
  }, { timeout: 35_000 }).toBe(true)
  const respawn = JSON.parse(await root.getAttribute("data-scoreboard-probe") ?? "{}")
  console.log("[2fort-match] live bot combat produced an authored death and respawn wave")

  await command("joinclass medic")
  await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":").slice(1, 3).join(":"))
    .toBe("5:19")
  await page.keyboard.press("Backquote")
  for (const [key, weapon] of [["Digit2", "20"], ["Digit3", "21"], ["Digit1", "19"]] as const) {
    await page.keyboard.press(key)
    await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[2]).toBe(weapon)
  }
  const stockWeaponPixels = await page.screenshot()
  await testInfo.attach("headed-2fort-authored-medic-stock-syringe-gun", { body: stockWeaponPixels, contentType: "image/png" })
  await page.keyboard.press("Backquote")
  await command("joinclass scout")
  await expect.poll(async () => (await root.getAttribute("data-hud-probe"))?.split(":")[1]).toBe("1")
  const firstFire = Number(await root.getAttribute("data-fire-events"))
  await command("+attack")
  await page.waitForFunction(previous => {
    const main = document.querySelector<HTMLElement>("main")!
    return Number(main.dataset.fireEvents) > previous && Number(main.dataset.hudProbe?.split(":")[3]) < 6
  }, firstFire, { timeout: 10_000 })
  await command("-attack")
  await expect.poll(async () => Number((await root.getAttribute("data-hud-probe"))?.split(":")[4]), { timeout: 10_000 }).toBeLessThan(32)
  const reserveBefore = Number((await root.getAttribute("data-hud-probe"))?.split(":")[4])
  const pickup = await page.evaluate(() => {
    const position = (document.querySelector<HTMLElement>("main")?.dataset.cameraPosition ?? "").split(",").map(Number)
    const item = (globalThis as any).__playsrcProfile.pickups
      .filter((value: any) => value.kind === "ammo" && value.available && (value.team === null || value.team === 2))
      .toSorted((left: any, right: any) => Math.hypot(...left.origin.map((value: number, axis: number) => value - position[axis]!))
        - Math.hypot(...right.origin.map((value: number, axis: number) => value - position[axis]!)))[0]
    if (!item) throw new Error("ctf_2fort has no available authored ammunition pickup")
    return structuredClone(item)
  })
  await command(`setpos ${pickup.origin.join(" ")}`)
  await page.waitForFunction(identity =>
    (globalThis as any).__playsrcProfile.pickups.find((item: any) => item.identity === identity)?.available === false,
  pickup.identity, { timeout: 10_000 })
  const reserveAfter = Number((await root.getAttribute("data-hud-probe"))?.split(":")[4])
  expect(reserveAfter).toBeGreaterThan(reserveBefore)

  await expect.poll(async () => (await root.getAttribute("data-round-probe"))?.split(":").slice(0, 2).join(":"), { timeout: 45_000 }).toBe("4:0")
  console.log("[2fort-match] stock weapons, ammunition pickup, and running round are active")

  await command("noclip")
  await expect(root).toHaveAttribute("data-movement-mode", "1")
  const quote = (name: string): string => `"${name.replaceAll("\\", "\\\\").replaceAll('"', '\\"')}"`
  const teleport = async (name: string, position: readonly number[]): Promise<void> => {
    await command(`bot_teleport ${quote(name)} ${position.join(" ")} 0 0 0`)
  }
  await teleport(scout.name, RED_FLAG)
  await page.waitForFunction(identity => {
    const bot = (globalThis as any).__playsrcProfile?.bots?.find((value: any) => value.identity === identity)
    return bot?.carryingFlag === true && bot.objective === 4
  }, scout.identity, { timeout: 10_000 })
  const carried = await page.evaluate(identity => {
    const profile = (globalThis as any).__playsrcProfile
    const bot = profile.bots.find((value: any) => value.identity === identity)
    profile.displacementCameraOverride = {
      position: [bot.position[0] - 120, bot.position[1], bot.position[2] + 58],
      yawDegrees: 0,
      pitchDegrees: 0,
    }
    return structuredClone(bot)
  }, scout.identity)
  await page.waitForFunction(() => {
    const override = (globalThis as any).__playsrcProfile.displacementCameraOverride
    return document.querySelector<HTMLElement>(".world-canvas")?.dataset.displayCameraPosition === override.position.join(",")
  })
  await page.keyboard.press("Backquote")
  const carrierPixels = await page.locator(".world-canvas").screenshot()
  await testInfo.attach("headed-2fort-blu-scout-carries-red-intelligence", { body: carrierPixels, contentType: "image/png" })
  await page.keyboard.press("Backquote")
  await command(`bot_whack ${quote(scout.name)}`)
  await expect.poll(async () => root.getAttribute("data-ctf")).toMatch(/,2,2,0,/)
  const droppedPixels = await page.locator(".world-canvas").screenshot()
  await testInfo.attach("headed-2fort-red-intelligence-dropped-after-scout-death", {
    body: droppedPixels,
    contentType: "image/png",
  })
  const dropped = await page.evaluate(identity => ({
    bot: structuredClone((globalThis as any).__playsrcProfile.bots.find((value: any) => value.identity === identity)),
    flag: structuredClone((globalThis as any).__playsrcProfile.objectives.find((flag: any) => flag.team === 2)),
  }), scout.identity)
  expect(dropped.bot).toMatchObject({ lifecycle: 2, health: 0, carryingFlag: false })
  expect(dropped.bot.deaths).toBeGreaterThanOrEqual(1)
  await command(`setpos ${dropped.flag.position[0]} ${dropped.flag.position[1]} ${dropped.flag.position[2]}`)
  await expect.poll(async () => root.getAttribute("data-ctf")).toMatch(/,2,0,0,/)
  console.log("[2fort-match] BLU intelligence theft, carrier death, drop, and RED return are authoritative")

  const finisher = await page.evaluate(excluded => {
    const profile = (globalThis as any).__playsrcProfile
    const bot = profile.bots.find((value: any) => value.identity !== excluded && value.team === 3 && value.class === 3 && value.lifecycle === 1)
      ?? profile.bots.find((value: any) => value.identity !== excluded && value.team === 3 && value.lifecycle === 1)
    const score = profile.combat.scores.find((value: any) => value.identity === bot?.identity)
    if (!bot || !score) throw new Error("the live BLU Soldier is unavailable to finish the round")
    return { identity: bot.identity, name: score.name as string, class: bot.class as number }
  }, scout.identity)
  await command("setpos 0 0 1000")
  await teleport(finisher.name, RED_FLAG)
  await page.waitForFunction(identity => {
    const bot = (globalThis as any).__playsrcProfile?.bots?.find((value: any) => value.identity === identity)
    return bot?.carryingFlag === true && bot.objective === 4
  }, finisher.identity, { timeout: 10_000 })
  await teleport(finisher.name, BLUE_CAPTURE)
  await expect(root).toHaveAttribute("data-ctf", /^0:1:1:3:/, { timeout: 15_000 })
  await expect(root).toHaveAttribute("data-round-probe", /^5:0:0:0:3:0:1:/)
  const winPanel = page.locator(".hud-layer [data-vgui-name='WinPanel']")
  await expect(winPanel).toBeVisible()
  await expect(page.locator(".hud-layer [data-vgui-name='WinningTeamLabel']")).toHaveText("BLU TEAM WINS!")
  await expect(winPanel.locator("[data-vgui-name='Player1Name']")).toHaveText(finisher.name)
  const className = ["", "Scout", "Sniper", "Soldier", "Demoman", "Medic", "Heavy", "Pyro", "Spy", "Engineer"][finisher.class]!
  await expect(winPanel.locator("[data-vgui-name='Player1Class']")).toHaveText(className)
  await expect(winPanel).not.toContainText("[unknown]")
  await expect.poll(async () => root.getAttribute("data-audio-starts")).toContain("Game.YourTeamLost")
  const victory = JSON.parse(await root.getAttribute("data-scoreboard-probe") ?? "{}")
  expect(victory.blue.score).toBe(1)
  const winningPlayer = victory.players.find((player: any) => player.identity === finisher.identity)
  expect(winningPlayer.captures).toBeGreaterThanOrEqual(1)
  expect(winningPlayer.score).toBeGreaterThanOrEqual(2)
  await page.keyboard.press("Backquote")
  const victoryPixels = await page.screenshot()
  await testInfo.attach("headed-2fort-real-blu-victory-red-defeat", { body: victoryPixels, contentType: "image/png" })

  const seconds = profileSampleSeconds()
  const measured = await page.evaluate(async duration => {
    const main = document.querySelector<HTMLElement>("main")!
    const started = performance.now()
    const firstTick = Number(main.dataset.snapshotTick)
    let previous = started
    const frames: number[] = []
    const modelTimes: number[] = []
    const bots: number[] = []
    await new Promise<void>(resolve => {
      const frame = (now: number): void => {
        frames.push(now - previous)
        previous = now
        if (main.dataset.performanceDetail) {
          const sample = JSON.parse(main.dataset.performanceDetail)
          modelTimes.push(sample.models)
          bots.push(sample.bots)
        }
        if (now - started >= duration * 1000) resolve()
        else requestAnimationFrame(frame)
      }
      requestAnimationFrame(frame)
    })
    return {
      seconds: (performance.now() - started) / 1000,
      firstTick,
      lastTick: Number(main.dataset.snapshotTick),
      frames,
      modelTimes,
      bots,
      objectiveEvents: [...(globalThis as any).__playsrcProfile.objectiveEvents],
    }
  }, seconds)
  expect(measured.lastTick - measured.firstTick).toBeGreaterThan(seconds * 60)
  expect(measured.bots.every(count => count === 9)).toBe(true)
  const frames = summarizeFrameTimes(measured.frames)
  expect(frames.p95Milliseconds).toBeLessThan(34)
  for (const event of ["2:1:", "2:4:", "2:5:", "2:2:", "8:3:"]) {
    expect(measured.objectiveEvents.some(value => value.startsWith(event))).toBe(true)
  }

  const visible = (bytes: Buffer): number => {
    const image = decodeScreenshot(bytes)
    let count = 0
    for (let index = 0; index < image.pixels.length; index += image.channels) {
      if (image.pixels[index]! > 24 || image.pixels[index + 1]! > 24 || image.pixels[index + 2]! > 24) count += 1
    }
    return count
  }
  const carrierImage = decodeScreenshot(carrierPixels)
  const droppedImage = decodeScreenshot(droppedPixels)
  let changedCarrierPixels = 0
  let teamColoredCarrierPixels = 0
  for (let y = 100; y < carrierImage.height - 100; y += 1) {
    for (let x = 320; x < carrierImage.width - 320; x += 1) {
      const offset = (y * carrierImage.width + x) * carrierImage.channels
      const red = carrierImage.pixels[offset]!
      const green = carrierImage.pixels[offset + 1]!
      const blue = carrierImage.pixels[offset + 2]!
      const delta = Math.abs(red - droppedImage.pixels[offset]!)
        + Math.abs(green - droppedImage.pixels[offset + 1]!)
        + Math.abs(blue - droppedImage.pixels[offset + 2]!)
      if (delta > 36) {
        changedCarrierPixels += 1
        if (Math.abs(red - blue) > 8) teamColoredCarrierPixels += 1
      }
    }
  }
  expect(changedCarrierPixels).toBeGreaterThan(500)
  expect(teamColoredCarrierPixels).toBeGreaterThan(100)
  const pixels = {
    createServer: visible(launchPixels),
    scoreboard: visible(scoreboardPixels),
    stockWeapons: visible(stockWeaponPixels),
    carrier: visible(carrierPixels),
    dropped: visible(droppedPixels),
    victory: visible(victoryPixels),
    changedCarrierPixels,
    teamColoredCarrierPixels,
  }
  expect(Math.min(pixels.createServer, pixels.scoreboard, pixels.stockWeapons, pixels.carrier, pixels.dropped, pixels.victory)).toBeGreaterThan(20_000)

  const report = {
    schema: "playsrc-tf2-headed-complete-2fort-match-v1",
    headed: true,
    target: "ctf_2fort",
    navigation: {
      sha256: "6c1e5b37b3cffb9ad97c554aa9e104119a5c5fb38bd6c9d2903a4d405f609017",
      areas: 1128,
      ...navigation,
    },
    classes: initial.map((bot: any) => ({ identity: bot.class, team: bot.team, weapon: bot.weapon?.identity ?? null })),
    medicStockWeapons: [19, 20, 21],
    combat: {
      localDeaths: respawn.players.find((player: any) => player.identity === 1)?.deaths,
      blueKills: death.players.filter((player: any) => player.team === 3).map((player: any) => player.kills),
      scoutDeaths: dropped.bot.deaths,
    },
    pickup: { identity: pickup.identity, kind: pickup.kind, reserveBefore, reserveAfter },
    objectives: {
      events: measured.objectiveEvents,
      droppedPosition: dropped.flag.position,
      firstCarrier: carried.identity,
      winningCarrier: finisher.identity,
      redCaptures: 0,
      blueCaptures: 1,
      limit: 1,
      winningTeam: "blue",
      localResult: "loss",
    },
    scoreboard: victory,
    pixels,
    simulation: {
      seconds: Number(measured.seconds.toFixed(3)),
      ticks: measured.lastTick - measured.firstTick,
      ticksPerSecond: Number(((measured.lastTick - measured.firstTick) / measured.seconds).toFixed(2)),
    },
    frames,
    modelPreparation: summarizeFrameTimes(measured.modelTimes),
  }
  const { sourceCacheDir } = await loadLocalConfig()
  const directory = path.join(sourceCacheDir, "evidence", "tf2-complete-2fort-match")
  await mkdir(directory, { recursive: true })
  await Promise.all([
    writeFile(path.join(directory, "ctf_2fort-complete-match.json"), `${JSON.stringify(report, null, 2)}\n`),
    writeFile(path.join(directory, "create-server.png"), launchPixels),
    writeFile(path.join(directory, "nine-class-scoreboard.png"), scoreboardPixels),
    writeFile(path.join(directory, "medic-stock-syringe-gun.png"), stockWeaponPixels),
    writeFile(path.join(directory, "blu-scout-red-intelligence.png"), carrierPixels),
    writeFile(path.join(directory, "red-intelligence-dropped.png"), droppedPixels),
    writeFile(path.join(directory, "blu-victory-red-defeat.png"), victoryPixels),
  ])
  console.log(`[2fort-complete-match] ${JSON.stringify(report)}`)
})
