import { createHash } from "node:crypto"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, guardStartupInput, test } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { startupConsoleIdle, startupNativeReader } from "./native-startup"
import { requireStartupNative } from "./static-startup-gate"
import { summarizeFrameTimes } from "./profile-window"

test("authored equipment, team and class controls survive the border correction", async ({ page, context }) => {
  const local = await loadLocalConfig(), directory = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!
  if (!directory) throw new Error("Use the checked button-controls profile runner")
  await mkdir(directory, { recursive: true })
  const native = await startupNativeReader(page, local.sourceCacheDir)
  const admit = async () => requireStartupNative(await native.read())
  const act = async (action: () => Promise<unknown>) => { await admit(); await action() }
  guardStartupInput(page, admit)
  const records: unknown[] = []
  const capture = async (name: string, selector: string) => {
    const control = page.locator(selector)
    await expect(control).toBeVisible()
    await admit()
    const bytes = await control.screenshot()
    await admit()
    const file = path.join(directory, `${name}.png`)
    await writeFile(file, bytes)
    records.push({ name, path: file, bytes: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex"),
      bounds: await control.boundingBox(), controls: await control.evaluate(root => [root, ...root.querySelectorAll('[data-vgui-control]')].map(element => ({
        name: (element as HTMLElement).dataset.vguiName, control: (element as HTMLElement).dataset.vguiControl,
        armed: (element as HTMLElement).dataset.armed, selected: (element as HTMLElement).dataset.selected,
        disabled: element.getAttribute("aria-disabled"),
      }))) })
  }
  try {
    if (await startupConsoleIdle(local.sourceCacheDir) < 2000) throw new Error("Control acceptance requires genuine native idle")
    await page.goto("/")
    const main = page.locator("main")
    await expect(main).toHaveAttribute("data-phase", "MainMenu")
    await act(() => page.locator('[data-vgui-name="CharacterSetupButton"]').click())
    await capture("equipment-class-buttons", ".equipment-layer")
    await act(() => page.locator('.equipment-layer [data-vgui-name="Class3"]').click())
    await expect(page.locator(".equipment-layer")).toHaveAttribute("data-preview-model", "models/player/soldier.mdl")
    await capture("equipment-loadout", ".equipment-layer")
    await act(() => page.locator('.equipment-layer [data-vgui-name="BackpackButton"]').click())
    await capture("equipment-backpack", ".equipment-layer")
    await act(() => page.locator('.equipment-layer [data-vgui-name="NextPage"]').click())
    await capture("equipment-next-page", ".equipment-layer")
    await act(() => page.keyboard.press("Backquote"))
    const entry = page.locator('[aria-label="Console command"]')
    await act(() => entry.fill("map jump_beef"))
    await act(() => entry.press("Enter"))
    await expect(main).toHaveAttribute("data-team-selection-visible", "true", { timeout: 45000 })
    if (await main.getAttribute("data-console-visible") === "true") await act(() => page.keyboard.press("Backquote"))
    await capture("team-normal", ".team-selection-layer")
    await act(() => page.locator('.team-selection-layer [data-vgui-name="teambutton1"]').hover())
    await capture("team-armed", ".team-selection-layer")
    await act(() => page.locator('.team-selection-layer [data-vgui-name="teambutton1"]').click())
    await expect(main).toHaveAttribute("data-class-selection-visible", "true")
    await act(() => page.locator('.class-selection-layer [data-vgui-name="scout"]').hover())
    await expect.poll(async () => JSON.parse(await main.getAttribute("data-class-selection-animation") || "{}").model).toBe("models/player/scout.mdl")
    await capture("class-armed", ".class-selection-layer")
    const cdp = await context.newCDPSession(page)
    await cdp.send("Performance.enable")
    const before = await cdp.send("Performance.getMetrics")
    await admit()
    const sampling = page.evaluate(async () => {
      const frames: number[] = [], start = performance.now()
      let previous = start
      await new Promise<void>(resolve => requestAnimationFrame(function frame(now) {
        frames.push(now - previous); previous = now
        now - start >= 5000 ? resolve() : requestAnimationFrame(frame)
      }))
      return { milliseconds: performance.now() - start, frames }
    })
    for (let second = 0; second < 5; second += 1) {
      await page.waitForTimeout(1000)
      await admit()
    }
    const sample = await sampling
    await admit()
    const after = await cdp.send("Performance.getMetrics")
    await cdp.detach()
    await writeFile(path.join(directory, "control-sample.json"), JSON.stringify({ milliseconds: sample.milliseconds, frames: summarizeFrameTimes(sample.frames), before, after }))
    await act(() => page.keyboard.press("Digit1"))
    await expect(main).toHaveAttribute("data-class-selection-visible", "false")
    await act(() => page.keyboard.press("Backquote"))
    await act(() => entry.fill("disconnect"))
    await act(() => entry.press("Enter"))
    await expect(main).toHaveAttribute("data-phase", "MainMenu")
    const generated = await readFile(new URL("../../../games/tf2/browser/src/ui-resources/configured.generated.ts", import.meta.url), "utf8")
    const input = JSON.parse(generated.split("export const configuredTf2UiResourceInput: unknown = ")[1]!)
    await writeFile(path.join(directory, "control-sources.json"), JSON.stringify({ contentBuild: input.contentBuild, sourceLedgerSha256: input.sourceLedgerSha256 }))
  } finally {
    await writeFile(path.join(directory, "control-captures.json"), JSON.stringify(records, null, 2))
    await writeFile(path.join(directory, "control-native-admission.json"), JSON.stringify(native.records, null, 2))
    await native.close()
  }
})
