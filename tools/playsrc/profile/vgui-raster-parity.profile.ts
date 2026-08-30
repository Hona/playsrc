import { readFile, writeFile } from "node:fs/promises"
import { createHash } from "node:crypto"
import path from "node:path"
import { test, expect } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { startupConsoleIdle, startupNativeReader } from "./native-startup"
import { requireStartupNative } from "./static-startup-gate"
import { profileArtifact } from "./profile-artifacts"
import { decodeScreenshot } from "./screenshot-pixels"

test.use({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 1 })
test("shared raster preserves native alpha, clipping, order and input", async ({ page, baseURL }) => {
  test.setTimeout(45_000)
  const directory = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!, { sourceCacheDir } = await loadLocalConfig()
  if (!directory || !baseURL) throw new Error("Use the checked native raster profile")
  const reader = await startupNativeReader(page, sourceCacheDir), records: unknown[] = [], media: string[] = []
  try {
    await page.goto(new URL(`/@fs/${process.cwd().replaceAll("\\", "/")}/packages/presentation/vgui/tests/retained-raster.html`, baseURL).href)
    await page.waitForFunction(() => document.body.dataset.ready === "true")
    await page.waitForTimeout(2100)
    if (await startupConsoleIdle(sourceCacheDir) < 2000) throw new Error("Raster parity requires genuine native idle")
    requireStartupNative(await reader.read())
    const button = page.getByRole("button", { name: "Retained foreground" })
    await button.click(); await page.keyboard.press("Enter"); await button.blur()
    const state = await page.evaluate(() => ({ activations: Number(document.body.dataset.activations), unchangedWrites: Number(document.body.dataset.unchangedRasterWrites), changedWrites: Number(document.body.dataset.changedRasterWrites), mutations: Number(document.body.dataset.rasterMutations ?? 0) }))
    expect(state).toEqual({ activations: 2, unchangedWrites: 0, changedWrites: 2, mutations: 0 })
    for (const name of ["shared", "canvas"]) {
      if (name === "canvas") await page.evaluate(() => {
        document.querySelector<HTMLElement>("#retained")!.style.display = "none"
        document.querySelector<HTMLElement>("#baseline")!.style.display = "block"
      })
      await page.screenshot({ path: path.join(directory, `${name}.page.png`) })
      const native = await reader.read(path.join(directory, `${name}.desktop.png`), "window")
      requireStartupNative(native); requireStartupNative(await reader.read())
      records.push({ name, native })
      media.push(`${name}.page.png`, `${name}.desktop.png`)
    }
    await profileArtifact(async () => {
      const files = await Promise.all(media.map(async file => {
        const bytes = await readFile(path.join(directory, file))
        return { file, path: path.join(directory, file), byteLength: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex"), image: decodeScreenshot(bytes) }
      }))
      const comparisons = ["page", "desktop"].map(kind => {
        const a = files.find(file => file.file === `shared.${kind}.png`)!.image, b = files.find(file => file.file === `canvas.${kind}.png`)!.image
        expect([a.width, a.height, a.channels]).toEqual([b.width, b.height, b.channels])
        let differentChannels = 0
        for (let i = 0; i < a.pixels.length; i++) if (a.pixels[i] !== b.pixels[i]) differentChannels++
        return { kind, differentChannels }
      })
      await writeFile(path.join(directory, "raster-parity.json"), JSON.stringify({ state, records, comparisons, media: files.map(({ image, ...record }) => record) }, null, 2))
      for (const comparison of comparisons) expect(comparison.differentChannels).toBe(0)
    })
  } finally { await reader.close() }
})
