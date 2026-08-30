import { createHash } from "node:crypto"
import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, guardStartupInput, test } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { startupNativeReader } from "./native-startup"
import { requireStartupNative, startupPixelEvidence } from "./static-startup-gate"

test("game-owned repeated keydowns retain scores, focus and release", async ({ page }) => {
  const config = await loadLocalConfig()
  const output = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!
  if (!output) throw new Error("Run through the checked headed profile owner")
  await mkdir(output, { recursive: true })
  const native = await startupNativeReader(page, config.sourceCacheDir)
  const admit = async () => requireStartupNative(await native.read())
  guardStartupInput(page, admit)
  const observations: unknown[] = []
  const captures: unknown[] = []
  await page.addInitScript(() => {
    const events: unknown[] = []
    ;(globalThis as any).__inputEvents = events
    for (const type of ["keydown", "keyup", "focusin", "blur", "pointerlockchange", "contextmenu", "auxclick"]) {
      window.addEventListener(type, event => {
        const key = event as KeyboardEvent
        // A microtask checkpoint can run between native event listeners. Read
        // cancellation after the entire dispatch, not before the game handler.
        setTimeout(() => events.push({ type, code: key.code, repeat: key.repeat, trusted: event.isTrusted,
          prevented: event.defaultPrevented, focus: document.activeElement?.className, scroll: [scrollX, scrollY],
          locked: document.pointerLockElement?.className ?? null }), 0)
      }, true)
    }
  })
  const main = page.locator("main")
  const scores = page.locator(".hud-layer [data-vgui-name='scoreinfo']")
  const capture = async (name: string) => {
    await admit()
    const bytes = await page.screenshot()
    const file = `${name}.page.png`
    await writeFile(path.join(output, file), bytes)
    captures.push({ file, bytes: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex"), pixels: startupPixelEvidence(bytes) })
  }
  const state = () => page.evaluate(() => ({ focused: document.hasFocus(), active: document.activeElement?.className,
    scroll: [scrollX, scrollY], locked: document.pointerLockElement?.className ?? null,
    scores: document.querySelector<HTMLElement>("main")?.dataset.scoreboardVisible,
    outline: getComputedStyle(document.querySelector("canvas.world-canvas")!).outlineStyle }))
  try {
    await page.goto("/")
    await expect(main).toHaveAttribute("data-phase", "MainMenu")
    await admit()
    await page.keyboard.press("Backquote")
    const entry = page.locator("[aria-label='Console command']")
    const command = async (text: string) => { await entry.fill(text); await entry.press("Enter") }
    await command("map pl_upward")
    await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 90_000 })
    await command("jointeam red")
    await expect(main).toHaveAttribute("data-class-selection-visible", "true")
    await command("joinclass soldier")
    await expect(main).toHaveAttribute("data-class-selection-visible", "false")
    await expect(main).toHaveAttribute("data-phase", "Ready")
    await page.keyboard.press("Backquote")
    await expect(main).toHaveAttribute("data-console-visible", "false")
    await capture("closed")
    // Repeated down calls use Chromium's trusted autoRepeat delivery. A single
    // down plus a timer does NOT generate OS/browser repeated keydown events.
    const before = await state()
    await page.keyboard.down("Tab")
    await expect(scores).toBeVisible()
    await capture("first-down")
    for (let repeat = 0; repeat < 12; repeat++) {
      await page.keyboard.down("Tab")
      observations.push(await state())
      await page.waitForTimeout(40)
    }
    await capture("repeated-down")
    const after = await state()
    expect(after.active).toBe(before.active)
    expect(after.scroll).toEqual(before.scroll)
    expect(after.focused).toBe(true)
    expect(after.scores).toBe("true")
    const events = await page.evaluate(() => (globalThis as any).__inputEvents)
    const repeats = events.filter((event: any) => event.type === "keydown" && event.code === "Tab" && event.repeat)
    expect(repeats).toHaveLength(12)
    expect(repeats.every((event: any) => event.trusted && event.prevented)).toBe(true)
    await page.keyboard.up("Tab")
    await expect(scores).toBeHidden()
    await capture("released")

    for (const [key, attribute] of [["Comma", "data-class-selection-visible"], ["Period", "data-team-selection-visible"]]) {
      await page.keyboard.down("Tab")
      await page.keyboard.press(key!)
      await expect(main).toHaveAttribute(attribute!, "true")
      await page.keyboard.up("Tab")
      await expect(main).toHaveAttribute("data-scoreboard-visible", "false")
      await capture(key!)
      await page.keyboard.press("Escape")
      await expect(main).toHaveAttribute(attribute!, "false")
    }
    await page.keyboard.down("Tab")
    await page.keyboard.down("Backquote")
    await page.keyboard.down("Backquote")
    await page.keyboard.up("Backquote")
    await expect(main).toHaveAttribute("data-console-visible", "true")
    await page.keyboard.up("Tab")
    await expect(scores).toBeHidden()
    await entry.fill("echo browser input")
    await entry.press("Home")
    await entry.press("Shift+End")
    expect(await entry.evaluate((element: HTMLInputElement) => element.value.slice(element.selectionStart!, element.selectionEnd!))).toBe("echo browser input")
    await entry.press("ControlOrMeta+c")
    await entry.press("End")
    await entry.press("ControlOrMeta+v")
    await expect(entry).toHaveValue("echo browser inputecho browser input")
    await entry.fill("joincl")
    await entry.press("Tab")
    await expect(entry).toHaveValue("joinclass ", { timeout: 3000 })
    await capture("console-editing")
    await page.keyboard.press("Backquote")
    await page.keyboard.press("Escape")
    await page.locator("[data-vgui-name='SettingsButton']").click()
    await expect(main).toHaveAttribute("data-options-visible", "true")
    const list = page.locator("[data-vgui-name='listpanel_keybindlist']")
    const { tf2UiResources } = await import("@playsrc/game-tf2-browser/ui-resources")
    const rebind = async (action: string, key: string) => {
      const row = tf2UiResources.keyboardActions.findIndex(row => row.binding === action)
      const target = list.locator(`[data-vgui-item='${row + 1}']`)
      await list.hover()
      for (let attempt = 0; attempt < 40; attempt++) {
        const bounds = await target.boundingBox(), viewport = await list.boundingBox()
        if (!bounds || !viewport) throw new Error("Binding row missing")
        if (bounds.y >= viewport.y && bounds.y + bounds.height <= viewport.y + viewport.height) break
        await page.mouse.wheel(0, bounds.y < viewport.y ? -240 : 240)
        await page.waitForTimeout(20)
      }
      const bounds = (await target.boundingBox())!
      await page.mouse.click(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2)
      await expect(target).toHaveAttribute("aria-selected", "true")
      await page.locator("[data-vgui-name='ChangeKeyButton']").click()
      await page.keyboard.press(key)
    }
    await rebind("+showscores", "ArrowUp")
    await rebind("+jump", "Tab")
    await capture("options-bindings")
    await page.locator("[data-vgui-name='OptionsDialog'] [data-vgui-name='OKButton']").click()
    await expect(main).toHaveAttribute("data-options-visible", "false")
    await page.locator("[data-vgui-name='ResumeButton']").click()
    await expect(main).toHaveAttribute("data-gameui", "in-game")
    for (const key of ["ArrowUp", "Tab"]) {
      const initial = await state()
      await page.keyboard.down(key)
      for (let repeat = 0; repeat < 5; repeat++) await page.keyboard.down(key)
      expect(await state()).toMatchObject({ active: initial.active, scroll: initial.scroll, focused: true, scores: key === "ArrowUp" ? "true" : "false" })
      await page.keyboard.up(key)
      await expect(scores).toBeHidden()
    }
    // Native pointer lock is obtained only by the real visible game click.
    await admit()
    await page.locator("canvas.world-canvas").click()
    await expect(main).toHaveAttribute("data-pointer-locked", "true", { timeout: 3000 })
    const locked = await state()
    expect(locked.outline).toBe("none")
    const firstTick = Number(await main.getAttribute("data-snapshot-tick"))
    const start = Date.now()
    await page.keyboard.down("ArrowUp")
    while (Date.now() - start < 6000) {
      await page.keyboard.down("ArrowUp")
      expect(await state()).toMatchObject({ focused: true, active: locked.active, scroll: locked.scroll, locked: "world-canvas", scores: "true" })
      await page.waitForTimeout(100)
    }
    observations.push({ sampleMilliseconds: Date.now() - start, ticks: Number(await main.getAttribute("data-snapshot-tick")) - firstTick })
    await capture("held-locked")
    await page.keyboard.up("ArrowUp")
    await expect(scores).toBeHidden()
    const position = await main.getAttribute("data-camera-position")
    await page.keyboard.down("KeyS")
    await page.waitForTimeout(150)
    await page.keyboard.up("KeyS")
    await expect.poll(() => main.getAttribute("data-camera-position")).not.toBe(position)
    await page.mouse.down({ button: "middle" })
    await page.mouse.up({ button: "middle" })
    await page.mouse.click(640, 360, { button: "right" })
    expect((await state()).locked).toBe("world-canvas")
    await page.keyboard.down("ArrowUp")
    await page.keyboard.press("Escape")
    await expect(main).toHaveAttribute("data-pointer-locked", "false")
    await page.keyboard.up("ArrowUp")
    await expect(scores).toBeHidden()
    if (await main.getAttribute("data-gameui") !== "pause") await page.keyboard.press("Escape")
    await page.locator("[data-vgui-name='ResumeButton']").click()
    await expect(main).toHaveAttribute("data-gameui", "in-game")
    await page.keyboard.down("ArrowUp")
    await page.keyboard.press("Backquote")
    await command("map pl_upward")
    await page.keyboard.up("ArrowUp")
    await expect(main).toHaveAttribute("data-scoreboard-visible", "false")
    await expect(main).toHaveAttribute("data-phase", "Ready", { timeout: 60_000 })
    if (await main.getAttribute("data-console-visible") === "true") await page.keyboard.press("Backquote")
    await page.keyboard.press("Escape")
    await page.locator("[data-vgui-name='DisconnectButton']").click()
    await expect(main).toHaveAttribute("data-phase", "MainMenu")
    await expect(main).toHaveAttribute("data-scoreboard-visible", "false")
    await capture("disconnected")
  } finally {
    await writeFile(path.join(output, "browser-input.json"), JSON.stringify({ observations, captures,
      terminal: await main.evaluate(element => ({ ...((element as HTMLElement).dataset) })).catch(() => null),
      events: await page.evaluate(() => (globalThis as any).__inputEvents).catch(() => []), native: native.records }, null, 2))
    await native.close()
  }
})
