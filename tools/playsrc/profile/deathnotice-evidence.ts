import type { Page, TestInfo } from "@playwright/test"
import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"

// Compare the actual game screenshot's opaque weapon pixels with the configured
// VTF atlas/half-texel rectangle. Background blending and fonts require native
// captures; this assertion deliberately does not label those as pixel parity.
export async function captureDeathNotice(page: Page, testInfo: TestInfo, name: string) {
  const screenshot = await page.screenshot()
  const comparison = await page.evaluate(async encoded => {
    const svg = [...document.querySelectorAll<SVGSVGElement>("[data-vgui-name^='DeathNotice'] [data-death-icon]")].at(-1)!
    const rectangle = svg.getBoundingClientRect(), view = svg.viewBox.baseVal, dpr = devicePixelRatio
    const image = async (url: string) => { const value = new Image(); value.src = url; await value.decode(); return value }
    const [actual, atlas] = await Promise.all([image(`data:image/png;base64,${encoded}`), image(svg.querySelector("image")!.getAttribute("href")!)])
    const x = Math.floor(rectangle.x * dpr), y = Math.floor(rectangle.y * dpr)
    const width = Math.ceil(rectangle.right * dpr) - x, height = Math.ceil(rectangle.bottom * dpr) - y
    const canvas = document.createElement("canvas"); canvas.width = width; canvas.height = height
    const ctx = canvas.getContext("2d", { willReadFrequently: true })!
    ctx.drawImage(actual, -x, -y)
    const pixels = ctx.getImageData(0, 0, width, height).data
    ctx.clearRect(0, 0, width, height)
    ctx.drawImage(atlas, view.x, view.y, view.width, view.height, rectangle.x * dpr - x, rectangle.y * dpr - y, rectangle.width * dpr, rectangle.height * dpr)
    const expected = ctx.getImageData(0, 0, width, height).data
    let compared = 0, matched = 0, maximumDifference = 0
    for (let i = 0; i < expected.length; i += 4) {
      if (expected[i + 3]! !== 255) continue
      compared++
      const difference = Math.max(...[0, 1, 2].map(c => Math.abs(expected[i + c]! - pixels[i + c]!)))
      if (difference <= 3) matched++
      maximumDifference = Math.max(maximumDifference, difference)
    }
    return { icon: svg.dataset.deathIcon, sourceTextureSha256: svg.dataset.sourceTextureSha256,
      dpr, rectangle: { x: rectangle.x, y: rectangle.y, width: rectangle.width, height: rectangle.height },
      compared, matched, maximumDifference, matchedFraction: matched / compared }
  }, screenshot.toString("base64"))
  const local = await loadLocalConfig()
  const directory = path.join(local.sourceCacheDir, "evidence", "tf2-deathnotice", `${Date.now()}-${name}`)
  await mkdir(directory, { recursive: true })
  await writeFile(path.join(directory, "game.png"), screenshot)
  await writeFile(path.join(directory, "atlas-comparison.json"), JSON.stringify(comparison, null, 2))
  await testInfo.attach(name, { path: path.join(directory, "game.png"), contentType: "image/png" })
  console.log(`[deathnotice-pixels] ${JSON.stringify({ directory, ...comparison })}`)
  return comparison
}
