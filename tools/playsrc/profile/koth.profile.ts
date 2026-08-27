import { expect, test } from "./application-test"
import { chooseTf2Team } from "./team-selection-evidence"
import { summarizeFrameTimes } from "./profile-window"
import { decodeScreenshot } from "./screenshot-pixels"

test("headed Viaduct local KOTH capture, contest, overtime, victory and restart with independent team clocks", async ({ page }, testInfo) => {
  await page.addInitScript(() => { (globalThis as any).__playsrcProfile = {} })
  const main = page.locator("main")
  await page.goto("/")
  await expect(main).toHaveAttribute("data-phase", "MainMenu", { timeout: 60_000 })
  const command = async (text: string) => {
    if (await main.getAttribute("data-console-visible") !== "true") await page.keyboard.press("Backquote")
    const entry = page.locator("[aria-label='Console command']")
    await entry.fill(text); await entry.press("Enter")
  }
  const closeConsole = async () => { if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote") }
  await command("map koth_viaduct")
  await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 60_000 })
  await closeConsole()
  await chooseTf2Team(page, "red")
  await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 30_000 })
  await command("tf_bot_quota 15")
  await command("setpos -1536 -220 230")
  await command("setang 0 90 0")
  await closeConsole()
  await expect(main).toHaveAttribute("data-bot-count", "15")

  const pixels = async (name: string) => {
    await closeConsole()
    const bytes = await page.screenshot()
    const image = decodeScreenshot(bytes)
    let visible = 0
    for (let i = 0; i < image.pixels.length; i += image.channels) if (image.pixels[i]! + image.pixels[i + 1]! + image.pixels[i + 2]! > 72) visible++
    expect(visible).toBeGreaterThan(20_000)
    await testInfo.attach(name, { body: bytes, contentType: "image/png" })
  }
  const sample = async (bots: number) => {
    await expect(main).toHaveAttribute("data-bot-count", String(bots))
    await closeConsole()
    const result = await page.evaluate(async () => {
      const root = document.querySelector<HTMLElement>("main")!
      const tick = Number(root.dataset.snapshotTick), start = performance.now()
      let previous = start
      const frames: number[] = []
      await new Promise<void>(resolve => {
        const frame = (now: number) => { frames.push(now - previous); previous = now; if (now - start >= 5000) resolve(); else requestAnimationFrame(frame) }
        requestAnimationFrame(frame)
      })
      return { elapsed: (performance.now() - start) / 1000, ticks: Number(root.dataset.snapshotTick) - tick, frames,
        bots: (globalThis as any).__playsrcProfile.bots.map((bot: any) => ({ identity: bot.identity, team: bot.team, objective: bot.objective, area: bot.area, position: bot.position })) }
    })
    expect(result.ticks / result.elapsed).toBeGreaterThan(63)
    expect(result.bots).toHaveLength(bots)
    expect(result.bots.every((bot: any) => bot.area !== null)).toBe(true)
    await pixels(`headed-koth-${bots}-bots-world-and-hud`)
    return { ...result, frames: summarizeFrameTimes(result.frames) }
  }
  const samples = [await sample(15)]
  await command("tf_bot_quota 23")
  samples.push(await sample(23))
  // Deterministic gameplay below uses real brush contact and a real opposing bot.
  // Entity inputs shorten match duration, not clocks, tick rate, or capture time.
  await command("tf_bot_kick all")
  await expect(main).toHaveAttribute("data-bot-count", "0")
  await page.waitForFunction(() => !(globalThis as any).__playsrcProfile.round.waitingForPlayers && (globalThis as any).__playsrcProfile.round.state === 4, undefined, { timeout: 40_000 })
  await command("ent_fire control_point_1 SetUnlockTime 1")
  await command("setpos -1536 0 230")
  await closeConsole()
  const timerRoot = page.locator(".hud-layer [data-vgui-name='HudKothTimeStatus']")
  await expect(timerRoot).toBeVisible()
  await page.waitForFunction(() => !(globalThis as any).__playsrcProfile.round.kothTimers[0].paused, undefined, { timeout: 20_000 })
  await expect(timerRoot.locator("[data-vgui-name='BlueTimer'] [data-vgui-name='TimePanelValue']")).toHaveText("3:00")
  await pixels("headed-koth-red-captured")
  await command("tf_bot_add 1 blue soldier hard")
  await expect(main).toHaveAttribute("data-bot-count", "1")
  const enemy = await page.evaluate(() => (globalThis as any).__playsrcProfile.bots.find((bot: any) => bot.team === 3).identity as number)
  const scoreboard = JSON.parse(await main.getAttribute("data-scoreboard-probe") ?? "{}")
  const enemyName = scoreboard.players.find((player: any) => player.identity === enemy).name as string
  await command("setpos -1536 -130 230")
  await command(`bot_teleport "${enemyName}" -1536 0 230 0 90 0`)
  await command("ent_fire tf_logic_koth SetRedTimer 3")
  await command("setpos -1536 0 230")
  await closeConsole()
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.round.inOvertime, undefined, { timeout: 10_000 })
  await expect(timerRoot.locator("[data-vgui-name='RedTimer'] [data-vgui-name='OvertimeLabel']")).toBeVisible()
  await pixels("headed-koth-live-brush-contest-overtime")
  await command(`bot_whack "${enemyName}"`)
  await closeConsole()
  await expect(page.locator(".hud-layer [data-vgui-name='WinningTeamLabel']")).toHaveText("RED TEAM WINS!", { timeout: 10_000 })
  await pixels("headed-koth-red-victory")
  await command("tf_bot_kick all")
  await page.waitForFunction(() => (globalThis as any).__playsrcProfile.round.state === 3, undefined, { timeout: 20_000 })
  await pixels("headed-koth-round-restart")
  await command("jointeam blue")
  await command("ent_fire control_point_1 SetUnlockTime 1")
  await command("setpos -1536 0 230")
  await closeConsole()
  await page.waitForFunction(() => !(globalThis as any).__playsrcProfile.round.kothTimers[1].paused, undefined, { timeout: 25_000 })
  await expect(timerRoot.locator("[data-vgui-name='RedTimer'] [data-vgui-name='TimePanelValue']")).toHaveText("3:00")
  await pixels("headed-koth-blue-captured-after-restart")
  await command("ent_fire tf_logic_koth SetBlueTimer 1")
  await closeConsole()
  await expect(page.locator(".hud-layer [data-vgui-name='WinningTeamLabel']")).toHaveText("BLU TEAM WINS!", { timeout: 10_000 })
  await pixels("headed-koth-blue-victory")
  await testInfo.attach("koth-source-clock-samples", { body: JSON.stringify({ schema: "playsrc-koth-headed-v1", samples }), contentType: "application/json" })
})
