import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { inflateSync } from "node:zlib"
import { chromium } from "@playwright/test"
import config from "../../../../../playsrc.local.json"
import { tf2AuthoredCrosshairs } from "../../src/ui-resources"

const BACKGROUND = [9, 11, 13] as const
const OUTPUT = path.join(config.sourceCacheDir, "evidence", "tf2-crosshair")

type DecodedPng = Readonly<{ width: number; height: number; channels: 3 | 4; samples: Uint8Array }>

function decodePng(bytes: Uint8Array): DecodedPng {
  const input = Buffer.from(bytes)
  if (!input.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]))) {
    throw new Error("Headed crosshair evidence is not PNG")
  }
  const compressed: Buffer[] = []
  let width = 0
  let height = 0
  let channels: 3 | 4 = 4
  for (let offset = 8; offset + 12 <= input.length;) {
    const length = input.readUInt32BE(offset)
    const kind = input.subarray(offset + 4, offset + 8).toString("ascii")
    const value = input.subarray(offset + 8, offset + 8 + length)
    if (kind === "IHDR") {
      width = value.readUInt32BE(0)
      height = value.readUInt32BE(4)
      if (value[8] !== 8 || ![2, 6].includes(value[9]!)) throw new Error("Headed crosshair PNG encoding is unsupported")
      channels = value[9] === 2 ? 3 : 4
    } else if (kind === "IDAT") compressed.push(value)
    offset += length + 12
  }
  const inflated = inflateSync(Buffer.concat(compressed))
  const stride = width * channels
  const samples = new Uint8Array(stride * height)
  for (let row = 0; row < height; row += 1) {
    const encodedStart = row * (stride + 1)
    const filter = inflated[encodedStart]!
    for (let column = 0; column < stride; column += 1) {
      const index = row * stride + column
      const left = column >= channels ? samples[index - channels]! : 0
      const up = row > 0 ? samples[index - stride]! : 0
      const upperLeft = row > 0 && column >= channels ? samples[index - stride - channels]! : 0
      let predictor = 0
      if (filter === 1) predictor = left
      else if (filter === 2) predictor = up
      else if (filter === 3) predictor = Math.floor((left + up) / 2)
      else if (filter === 4) {
        const estimate = left + up - upperLeft
        const leftDistance = Math.abs(estimate - left)
        const upDistance = Math.abs(estimate - up)
        const cornerDistance = Math.abs(estimate - upperLeft)
        predictor = leftDistance <= upDistance && leftDistance <= cornerDistance ? left : upDistance <= cornerDistance ? up : upperLeft
      } else if (filter !== 0) throw new Error(`Headed crosshair PNG filter ${filter} is unsupported`)
      samples[index] = (inflated[encodedStart + 1 + column]! + predictor) & 255
    }
  }
  return Object.freeze({ width, height, channels, samples })
}

function opaquePixel(image: DecodedPng, colored: boolean): Readonly<{ x: number; y: number; rgb: readonly number[] }> {
  for (let y = 0; y < image.height; y += 1) {
    for (let x = 0; x < image.width; x += 1) {
      const start = (y * image.width + x) * image.channels
      const alpha = image.channels === 4 ? image.samples[start + 3]! : 255
      const rgb = [image.samples[start]!, image.samples[start + 1]!, image.samples[start + 2]!]
      if (alpha === 255 && (!colored || rgb.some((channel) => channel !== 255))) {
        return Object.freeze({ x, y, rgb: Object.freeze(rgb) })
      }
    }
  }
  throw new Error("Authored crosshair has no matching opaque comparison pixel")
}

function pixel(image: DecodedPng, x: number, y: number): readonly number[] {
  const start = (y * image.width + x) * image.channels
  return [image.samples[start]!, image.samples[start + 1]!, image.samples[start + 2]!]
}

function compare(actual: readonly number[], expected: readonly number[], subject: string): void {
  if (actual.some((value, index) => Math.abs(value - expected[index]!) > 2)) {
    throw new Error(`${subject} actual pixel ${actual.join(",")} differs from expected ${expected.join(",")}`)
  }
}

const built = await Bun.build({
  entrypoints: [path.join(import.meta.dir, "crosshair-browser.ts")],
  target: "browser",
  format: "esm",
})
if (!built.success || built.outputs.length !== 1) throw new Error("Headed TF2 crosshair production adapter did not bundle")
await mkdir(OUTPUT, { recursive: true })
const browser = await chromium.launch({ headless: false })
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 })
  await page.bringToFront()
  await page.setContent(`<html><head><style>html,body{margin:0;width:100%;height:100%;background:rgb(${BACKGROUND.join(",")})}#tf2-crosshair-headed-root{position:absolute;inset:0}</style></head><body><div id="tf2-crosshair-headed-root"></div></body></html>`)
  await page.addScriptTag({ type: "module", content: await built.outputs[0]!.text() })
  await page.waitForFunction(() => "__playsrcTf2Crosshair" in window)

  const stock = await page.evaluate(() => (window as any).__playsrcTf2Crosshair.show("", [200, 200, 200], 32))
  if (stock.left !== 624 || stock.top !== 344 || stock.width !== 32 || stock.height !== 32 || stock.style !== "stock") {
    throw new Error(`Headed stock crosshair geometry differs: ${JSON.stringify(stock)}`)
  }
  await page.waitForTimeout(650)
  const stockBytes = await page.screenshot({ path: path.join(OUTPUT, "stock-crosshair.png") })
  const stockImage = decodePng(stockBytes)
  const stockSource = decodePng(Buffer.from(tf2AuthoredCrosshairs.weapons[0]!.crosshair.frames[0]!.pngDataUrl.split(",")[1]!, "base64"))
  const stockPoint = opaquePixel(stockSource, false)
  const stockPixel = pixel(stockImage, stock.left + stockPoint.x, stock.top + stockPoint.y)
  const stockExpected = stockPoint.rgb.map((channel) => Math.round(channel * 200 / 255))
  compare(stockPixel, stockExpected, "stock authored/tinted")
  compare(pixel(stockImage, stock.left - 1, stock.top - 1), BACKGROUND, "stock surrounding transparency")

  const customTint = [48, 120, 240] as const
  const custom = await page.evaluate((color) => (window as any).__playsrcTf2Crosshair.show("crosshair5", color, 32), customTint)
  if (custom.left !== 608 || custom.top !== 328 || custom.width !== 64 || custom.height !== 64 || custom.style !== "crosshair5") {
    throw new Error(`Headed custom crosshair geometry differs: ${JSON.stringify(custom)}`)
  }
  await page.waitForTimeout(650)
  const customBytes = await page.screenshot({ path: path.join(OUTPUT, "crosshair5-tinted.png") })
  const customImage = decodePng(customBytes)
  const customSource = decodePng(Buffer.from(tf2AuthoredCrosshairs.styles.find((item) => item.file === "crosshair5")!.frames[0]!.pngDataUrl.split(",")[1]!, "base64"))
  const customPoint = opaquePixel(customSource, true)
  const customPixel = pixel(customImage, custom.left + customPoint.x, custom.top + customPoint.y)
  const customExpected = customPoint.rgb.map((channel, index) => Math.round(channel * customTint[index]! / 255))
  compare(customPixel, customExpected, "custom authored-color multiplication")

  const resized = await page.evaluate(() => (window as any).__playsrcTf2Crosshair.show("crosshair7", [255, 0, 64], 48))
  if (resized.left !== 592 || resized.top !== 312 || resized.width !== 96 || resized.height !== 96) {
    throw new Error(`Headed scaled crosshair geometry differs: ${JSON.stringify(resized)}`)
  }
  await page.waitForTimeout(650)
  await page.screenshot({ path: path.join(OUTPUT, "crosshair7-scaled.png") })
  const hidden = await page.evaluate(() => (window as any).__playsrcTf2Crosshair.show("crosshair7", [255, 0, 64], 48, 1, false))
  if (hidden.display !== "none") throw new Error("Headed crosshair suppression left authored pixels visible")
  await page.waitForTimeout(350)
  const suppressed = decodePng(await page.screenshot({ path: path.join(OUTPUT, "crosshair-suppressed.png") }))
  compare(pixel(suppressed, 640, 360), BACKGROUND, "crosshair suppression")

  const report = Object.freeze({
    schema: "playsrc-tf2-crosshair-headed-evidence-v1",
    headed: true,
    viewport: { width: 1280, height: 720, devicePixelRatio: 1 },
    stock: { ...stock, comparedPixel: stockPoint, actual: stockPixel, expected: stockExpected },
    custom: { ...custom, comparedPixel: customPoint, actual: customPixel, expected: customExpected },
    resized,
    suppressed: true,
    captures: ["stock-crosshair.png", "crosshair5-tinted.png", "crosshair7-scaled.png", "crosshair-suppressed.png"],
  })
  await writeFile(path.join(OUTPUT, "report.json"), `${JSON.stringify(report, null, 2)}\n`)
  console.log(JSON.stringify(report, null, 2))
} finally {
  await browser.close()
}
