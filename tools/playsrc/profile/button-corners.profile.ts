import { createHash } from "node:crypto"
import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { expect, guardStartupInput, test } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { decodeScreenshot } from "./screenshot-pixels"
import { startupConsoleIdle, startupNativeReader } from "./native-startup"
import { requireStartupNative } from "./static-startup-gate"
import { profileArtifact } from "./profile-artifacts"

test("dashboard square corners and authored rounded menu controls paint independently", async ({ page }) => {
  const local = await loadLocalConfig(), directory = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!
  if (!directory) throw new Error("Use the checked button-corners profile runner")
  await mkdir(directory, { recursive: true })
  const native = await startupNativeReader(page, local.sourceCacheDir)
  const admit = async () => requireStartupNative(await native.read())
  const act = async (action: () => Promise<unknown>) => { await admit(); await action() }
  guardStartupInput(page, admit)
  const records: unknown[] = []
  await profileArtifact(async () => { await writeFile(path.join(directory, "native-admission.json"), JSON.stringify(native.records, null, 2)) })
  const selector = (name: string) => `[data-vgui-name="${name}"]`
  // Configured ClientScheme TanDark / TFOrange / GreenSolid / CreditsGreen.
  // Not sampled from the candidate's DOM, canvas, or computed styles.
  const tan = [117, 107, 94], orange = [145, 73, 59]
  const capture = async (name: string, state: string, fill: number[], side: "left" | "right", rounded: boolean) => {
    const control = page.locator(selector(name))
    await expect(control).toBeVisible()
    await admit()
    const bytes = await control.screenshot()
    await admit()
    const dpr = await page.evaluate(() => devicePixelRatio)
    const bounds = await control.boundingBox()
    const attributes = await control.evaluate(element => Object.fromEntries([...element.attributes].map(attribute => [attribute.name, attribute.value])))
    await profileArtifact(async () => {
    const file = path.join(directory, `${name}-${state}.png`)
    await writeFile(file, bytes)
    const image = decodeScreenshot(bytes)
    const pixel = (x: number, y: number) => {
      const offset = ((image.height - 1 - Math.floor(y * dpr)) * image.width + (side === "left" ? Math.floor(x * dpr) : image.width - 1 - Math.floor(x * dpr))) * image.channels
      return Array.from(image.pixels.subarray(offset, offset + 3))
    }
    const samples = Array.from({ length: 8 }, (_, y) => Array.from({ length: 8 }, (_, x) => pixel(x, y)))
    records.push({ name, state, file, bytes: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex"), dpr,
      bounds, side, rounded, samples, control: { attributes } })
    await writeFile(path.join(directory, "corners.json"), JSON.stringify(records, null, 2))
    // SDK Panel::DrawBox: each unmasked 8x8 corner is filled, not textured.
    // At fractional backing scale avoid the outer pixel's partial coverage.
    if (!rounded) {
      for (let y = 1; y < 7; y++) for (let x = 1; x < 7; x++) expect(samples[y]![x], `${name}/${state} square ${x},${y}`).toEqual(fill)
    } else {
      // Independent DXT5 alpha decode of configured 8x800corner1.vtf
      // (sha256 343e6cfd978e8d1d912ad1135dea57065485a43d62f4f6b1fd3cec19a33bea5a)
      // and its mirrored corners: at 8x8, (1,1) is discarded by alpha-test .1;
      // (6,6) is fully opaque. No CSS-radius assertion substitutes for pixels.
      expect(pixel(10, 1), `${name}/${state} authored strip fill`).toEqual(fill)
      // The authored texture's RGB is uniform. Its alpha-255 samples must all
      // equal one another, independently of the material's tint conversion.
      // This is a silhouette check, not a golden for that color conversion.
      const opaque = (rgb: number[]) => rgb.every((channel, index) => channel === samples[6]![6]![index])
      if (dpr === 1) {
        // '#' is source alpha 255; '.' includes alpha-test rejection and the
        // four partially covered edge samples (~226/255). Inspect all 64 pixels.
        // Straight filled corners and the strip above remain byte-exact.
        const coverage = ["......##", "....####", "..######", "..######", ".#######", ".#######", "########", "########"]
        for (let y = 0; y < 8; y++) for (let x = 0; x < 8; x++) expect(opaque(samples[y]![x]!), `${name}/${state} texture ${x},${y}`).toBe(coverage[y]![x] === "#")
      } else {
        expect(opaque(samples[1]![1]!), `${name}/${state} transparent corner`).toBe(false)
        expect(opaque(samples[6]![6]!), `${name}/${state} opaque corner`).toBe(true)
      }
    }
    })
  }
  try {
    if (await startupConsoleIdle(local.sourceCacheDir) < 2000) throw new Error("Corner acceptance requires native idle")
    await page.goto("/")
    await expect(page.locator("main")).toHaveAttribute("data-phase", "MainMenu")
    await act(() => page.mouse.move(600, 300))
    // Disabled local service controls are not artificially enabled for evidence.
    for (const [name, side, rounded] of [
      ["ToggleChatButton", "left", false], ["GeneralStoreButton", "left", true], ["ReportBugButton", "right", true],
    ] as const) {
      await expect(page.locator(selector(name))).toHaveAttribute("aria-disabled", "true")
      await capture(name, "disabled", tan, side, rounded)
    }
    for (const name of ["CharacterSetupButton", "SettingsButton", "FindAGameButton"]) {
      const control = page.locator(selector(name)), rounded = name !== "FindAGameButton"
      const normal = rounded ? tan : [76, 107, 34]
      const armed = rounded ? orange : [94, 150, 49]
      await capture(name, "normal", normal, "left", rounded)
      await act(() => control.hover())
      await expect(control).toHaveAttribute("data-armed", "true")
      await capture(name, "hover", armed, "left", rounded)
      await act(() => page.mouse.down())
      await expect(control).toHaveAttribute("data-depressed", "true")
      await capture(name, "pressed", orange, "left", rounded)
      await act(() => page.mouse.move(600, 300))
      await act(() => page.mouse.up()) // Cancel without opening a menu or issuing a command.
      await act(() => control.hover())
      await act(() => page.mouse.move(600, 300))
      // Dashboard FindAGameButton authors keyboardinputenabled=0. Do not
      // manufacture a key-focus state which this control cannot receive.
      await expect(control).toHaveAttribute("data-focused", rounded ? "true" : "false")
      await expect(control).toHaveAttribute("data-armed", "false")
      await capture(name, rounded ? "restored-focus" : "restored", normal, "left", rounded)
    }
    await admit()
    const bytes = await page.screenshot()
    await admit()
    await profileArtifact(async () => {
      await writeFile(path.join(directory, "main-menu.png"), bytes)
      records.push({ name: "main-menu", bytes: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex") })
      await writeFile(path.join(directory, "corners.json"), JSON.stringify(records, null, 2))
    })
    // One relevant five-second steady-paint sample, not a gameplay FPS claim.
    const cdp = await page.context().newCDPSession(page)
    await cdp.send("Performance.enable")
    const before = await cdp.send("Performance.getMetrics")
    await admit()
    const sample = await page.evaluate(async () => {
      const start = performance.now(), intervals: number[] = []
      let last = start
      await new Promise<void>(resolve => {
        const frame = (now: number) => { intervals.push(now - last); last = now; if (now - start >= 5000) resolve(); else requestAnimationFrame(frame) }
        requestAnimationFrame(frame)
      })
      intervals.sort((a, b) => a - b)
      return { milliseconds: performance.now() - start, rafFrames: intervals.length, rafP95: intervals[Math.floor(intervals.length * .95)] }
    })
    await admit()
    const after = await cdp.send("Performance.getMetrics")
    await profileArtifact(async () => { await writeFile(path.join(directory, "sample.json"), JSON.stringify({ sample, before, after }, null, 2)) })
    await cdp.detach()
  } finally {
    await native.close()
  }
})
