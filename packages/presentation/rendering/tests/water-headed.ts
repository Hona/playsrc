import { createHash } from "node:crypto"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { chromium } from "@playwright/test"
import { loadLocalConfig } from "../../../../tools/playsrc/src/config"

const root = path.resolve(import.meta.dir, "../../../..")
const configuration = await loadLocalConfig(root)
const evidenceDirectory = path.join(configuration.sourceCacheDir, "evidence", "tf2-water-rendering")
await mkdir(evidenceDirectory, { recursive: true })

const build = await Bun.build({
  entrypoints: [path.join(import.meta.dir, "water-headed-entry.ts")],
  target: "browser",
  format: "esm",
  minify: false,
})
if (!build.success || build.outputs.length !== 1) {
  throw new Error(`Water evidence browser bundle failed: ${build.logs.map(String).join("; ")}`)
}
const source = await build.outputs[0]!.text()
const authoredFrames = await Promise.all([0, 30].map(async (frame) => {
  const bytes = await readFile(path.join(evidenceDirectory, `normal-frame-${frame}.rgb`))
  if (bytes.byteLength !== 256 * 256 * 3) throw new Error(`Authored Water frame ${frame} is incomplete`)
  return bytes.toString("base64")
}))
const html = path.join(evidenceDirectory, "water-headed.html")
await writeFile(html, `<!doctype html><html><head><meta charset="utf-8"><title>Source Water headed pixel evidence</title><style>html,body{margin:0;background:#101820}canvas{display:block;width:640px;height:480px}</style></head><body><canvas width="640" height="480"></canvas><script>window.__sourceWaterAuthoredFrames=${JSON.stringify(authoredFrames)}</script><script type="module">${source.replaceAll("</script", "<\\/script")}</script></body></html>`)

const browser = await chromium.launch({
  channel: "msedge",
  headless: false,
  args: ["--enable-unsafe-webgpu"],
})
try {
  const page = await browser.newPage({ viewport: { width: 640, height: 480 }, deviceScaleFactor: 1 })
  page.on("pageerror", (error) => console.error(`[water-headed] ${error.stack ?? error.message}`))
  await page.bringToFront()
  await page.goto(`file://${html}`, { waitUntil: "load", timeout: 30_000 })
  await page.waitForFunction(() => (window as any).__sourceWaterEvidenceReady === true, undefined, { timeout: 30_000 })

  const captures = []
  for (const name of ["refraction", "reflection-left", "reflection-right", "underwater-blur", "authored-frame-0", "authored-frame-30"] as const) {
    const result = await page.evaluate((scenario) => (window as any).__sourceWaterEvidenceScenario(scenario), name) as {
      scenario: string
      expected: readonly [number, number, number, number]
      water: Readonly<{ x: number; y: number }>
      wall: Readonly<{ x: number; y: number }>
      reference: Record<string, unknown>
      retainedMaterialAnimation: boolean
      depthWrite: boolean
      transparent: boolean
    }
    await page.waitForTimeout(250)
    const screenshot = await page.screenshot({ animations: "disabled", caret: "hide" })
    const capturePath = path.join(evidenceDirectory, `${name}.png`)
    await writeFile(capturePath, screenshot)
    const observed = await page.evaluate(async ({ png, points }) => {
      const bytes = Uint8Array.from(atob(png), (value) => value.charCodeAt(0))
      const bitmap = await createImageBitmap(new Blob([bytes], { type: "image/png" }))
      const canvas = new OffscreenCanvas(bitmap.width, bitmap.height)
      const context = canvas.getContext("2d", { willReadFrequently: true })
      if (!context) throw new Error("Water evidence screenshot decoder is unavailable")
      context.drawImage(bitmap, 0, 0)
      return points.map(({ x, y }) => Array.from(context.getImageData(x, y, 1, 1).data))
    }, {
      png: screenshot.toString("base64"),
      points: [result.water, result.wall],
    }) as number[][]

    const water = observed[0]!
    const foregroundWall = observed[1]!
    const channelErrors = water.map((value, channel) => Math.abs(value - result.expected[channel]!))
    if (channelErrors.some((error) => error > 2)) {
      throw new Error(`${name} visible water pixel differs: expected ${result.expected.join(",")}, observed ${water.join(",")}`)
    }
    if (foregroundWall[1]! <= foregroundWall[0]! || foregroundWall[1]! <= foregroundWall[2]!) {
      throw new Error(`${name} opaque foreground wall lost its depth relationship: ${foregroundWall.join(",")}`)
    }
    if (!result.depthWrite || result.transparent) {
      throw new Error(`${name} expensive Water lost opaque depth-write state`)
    }
    if (name === "authored-frame-30" && !result.retainedMaterialAnimation) {
      throw new Error("Authored normal-frame animation replaced the retained Water material")
    }
    captures.push(Object.freeze({
      ...result,
      observedWater: Object.freeze(water),
      observedForegroundWall: Object.freeze(foregroundWall),
      channelErrors: Object.freeze(channelErrors),
      capture: path.basename(capturePath),
      captureSha256: createHash("sha256").update(screenshot).digest("hex"),
      captureBytes: screenshot.byteLength,
    }))
  }

  const report = Object.freeze({
    schema: "playsrc-source-water-headed-pixel-evidence-v1",
    headed: true,
    runtime: "Chromium WebGPU",
    viewport: Object.freeze({ width: 640, height: 480, deviceScaleFactor: 1 }),
    captures: Object.freeze(captures),
  })
  const reportPath = path.join(evidenceDirectory, "water-headed.json")
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`)
  console.log(`SOURCE_WATER_HEADED ${JSON.stringify(report)}`)
} finally {
  await browser.close()
}
