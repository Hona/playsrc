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
        queueMicrotask(() => events.push({ type, code: key.code, repeat: key.repeat, trusted: event.isTrusted,
          prevented: event.defaultPrevented, focus: document.activeElement?.className, scroll: [scrollX, scrollY],
          locked: document.pointerLockElement?.className ?? null }))
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
    expect.soft(after.active).toBe(before.active)
    expect.soft(after.scroll).toEqual(before.scroll)
    expect.soft(after.focused).toBe(true)
    expect.soft(after.scores).toBe("true")
    const events = await page.evaluate(() => (globalThis as any).__inputEvents)
    const repeats = events.filter((event: any) => event.type === "keydown" && event.code === "Tab" && event.repeat)
    expect.soft(repeats).toHaveLength(12)
    expect.soft(repeats.every((event: any) => event.trusted && event.prevented)).toBe(true)
    await page.keyboard.up("Tab")
    await expect(scores).toBeHidden()
    await capture("released")
  } finally {
    await writeFile(path.join(output, "browser-input.json"), JSON.stringify({ observations, captures,
      events: await page.evaluate(() => (globalThis as any).__inputEvents).catch(() => []), native: native.records }, null, 2))
    await native.close()
  }
})
