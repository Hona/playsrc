import { mkdir, rm, writeFile } from "node:fs/promises"
import { createHash } from "node:crypto"
import path from "node:path"
import type { Page } from "@playwright/test"
import { expect, test } from "./application-test"
import { loadLocalConfig } from "../src/config"

const TARGET = "jump_beef"
const VIEWPORTS = Object.freeze([
  Object.freeze({ name: "initial", width: 1280, height: 720, devicePixelRatio: 1 }),
  Object.freeze({ name: "height-only", width: 1280, height: 900, devicePixelRatio: 1 }),
  Object.freeze({ name: "width-only", width: 1600, height: 900, devicePixelRatio: 1 }),
  Object.freeze({ name: "four-three", width: 1024, height: 768, devicePixelRatio: 1 }),
  Object.freeze({ name: "ultrawide", width: 2560, height: 1080, devicePixelRatio: 1 }),
  Object.freeze({ name: "portrait", width: 390, height: 844, devicePixelRatio: 1 }),
  Object.freeze({ name: "landscape", width: 844, height: 390, devicePixelRatio: 1 }),
  Object.freeze({ name: "dpr-two", width: 1280, height: 720, devicePixelRatio: 2 }),
  Object.freeze({ name: "restored", width: 1280, height: 720, devicePixelRatio: 1 }),
])
const PANELS = Object.freeze([
  "HudViewport",
  "HudPlayerStatus",
  "HudPlayerClass",
  "PlayerStatusClassImage",
  "HudPlayerHealth",
  "PlayerStatusHealthImage",
  "HudWeaponAmmo",
  "HudWeaponSelection",
  "HudCrosshair",
])

type Rect = Readonly<{ x: number; y: number; width: number; height: number }>
type DecodedPng = Readonly<{ width: number; height: number; rgb: Uint8Array; alpha: Uint8Array }>
type PixelMetric = Readonly<{
  name: "bottom-25-percent" | "bottom-10-percent" | "bottom-32-pixels" | "final-row"
  x: number
  y: number
  width: number
  height: number
  alphaOccupancy: number
  opaqueBlackOccupancy: number
  meanAlpha: number
  sha256: string
}>
type Capture = Readonly<{
  viewport: (typeof VIEWPORTS)[number]
  innerViewport: Readonly<{ width: number; height: number; devicePixelRatio: number }>
  visualViewport: Readonly<{ x: number; y: number; width: number; height: number; scale: number }> | null
  ownerViewport: string
  ownerRevision: number
  rectangles: Readonly<Record<"html" | "body" | "app" | "main" | "canvas" | "hudLayer" | "hudHost", Rect>>
  ownerRecords: Readonly<Record<"canvas" | "startup" | "loading" | "gameUi" | "hud" | "options" | "developer", string>>
  panels: Readonly<Record<string, Readonly<{ id: string; parent: string | null; local: Rect; rect: Rect }>>>
  pixels: Readonly<{ canvas: readonly PixelMetric[]; hud: readonly PixelMetric[]; composed: readonly PixelMetric[] }>
}>

const scaled = (value: number, height: number): number => Math.trunc(value * height / 480)
const rect = (x: number, y: number, width: number, height: number): Rect => Object.freeze({ x, y, width, height })

function expected(width: number, height: number): Readonly<Record<string, Rect>> {
  return Object.freeze({
    HudViewport: rect(0, 0, width, height),
    HudPlayerStatus: rect(0, 0, width, height),
    HudPlayerClass: rect(0, 0, width, height),
    PlayerStatusClassImage: rect(scaled(25, height), height - scaled(88, height), scaled(75, height), scaled(75, height)),
    HudPlayerHealth: rect(0, height - scaled(120, height), scaled(250, height), scaled(120, height)),
    PlayerStatusHealthImage: rect(scaled(75, height), scaled(35, height), scaled(51, height), scaled(51, height)),
    HudWeaponAmmo: rect(width - scaled(95, height), height - scaled(55, height), scaled(94, height), scaled(45, height)),
    HudWeaponSelection: rect(0, 0, width, height),
    HudCrosshair: rect(0, 0, scaled(640, height), height),
  })
}

async function settle(page: Page): Promise<void> {
  await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))))
}

async function isolatedScreenshot(page: Page, selector: string): Promise<Uint8Array> {
  await page.evaluate((selected) => {
    for (const element of [document.documentElement, document.body, document.getElementById("app"), document.querySelector("main")]) {
      if (!(element instanceof HTMLElement)) continue
      element.dataset.viewportEvidenceStyle = element.getAttribute("style") ?? ""
      element.style.setProperty("background", "transparent", "important")
    }
    const selectedElement = document.querySelector(selected)
    for (const child of document.querySelector("main")?.children ?? []) {
      if (child === selectedElement) continue
      const element = child as HTMLElement
      element.dataset.viewportEvidenceStyle = element.getAttribute("style") ?? ""
      element.style.setProperty("visibility", "hidden", "important")
    }
  }, selector)
  try {
    return new Uint8Array(await page.screenshot({ omitBackground: true }))
  } finally {
    await page.evaluate(() => {
      for (const element of document.querySelectorAll<HTMLElement>("[data-viewport-evidence-style]")) {
        const style = element.dataset.viewportEvidenceStyle ?? ""
        if (style) element.setAttribute("style", style)
        else element.removeAttribute("style")
        delete element.dataset.viewportEvidenceStyle
      }
    })
  }
}

function invariant(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message)
}

function readUint32(bytes: Uint8Array, offset: number): number {
  return new DataView(bytes.buffer, bytes.byteOffset + offset, 4).getUint32(0, false)
}

function paeth(left: number, above: number, upperLeft: number): number {
  const estimate = left + above - upperLeft
  const leftDistance = Math.abs(estimate - left)
  const aboveDistance = Math.abs(estimate - above)
  const upperLeftDistance = Math.abs(estimate - upperLeft)
  if (leftDistance <= aboveDistance && leftDistance <= upperLeftDistance) return left
  return aboveDistance <= upperLeftDistance ? above : upperLeft
}

async function decodePng(bytes: Uint8Array): Promise<DecodedPng> {
  const signature = [137, 80, 78, 71, 13, 10, 26, 10]
  invariant(bytes.byteLength >= 33 && signature.every((value, index) => bytes[index] === value), "viewport PNG signature is invalid")
  let offset = 8
  let width = 0
  let height = 0
  let channels = 0
  const compressedParts: Uint8Array[] = []
  while (offset < bytes.byteLength) {
    invariant(offset + 12 <= bytes.byteLength, "viewport PNG chunk is truncated")
    const length = readUint32(bytes, offset)
    const type = new TextDecoder().decode(bytes.subarray(offset + 4, offset + 8))
    const dataStart = offset + 8
    const dataEnd = dataStart + length
    invariant(dataEnd + 4 <= bytes.byteLength, "viewport PNG chunk range is invalid")
    if (type === "IHDR") {
      invariant(length === 13 && width === 0, "viewport PNG IHDR is invalid")
      width = readUint32(bytes, dataStart)
      height = readUint32(bytes, dataStart + 4)
      const bitDepth = bytes[dataStart + 8]
      const colorType = bytes[dataStart + 9]
      invariant(bitDepth === 8 && (colorType === 2 || colorType === 6), "viewport PNG color profile is unsupported")
      invariant(bytes[dataStart + 10] === 0 && bytes[dataStart + 11] === 0 && bytes[dataStart + 12] === 0, "viewport PNG encoding profile is unsupported")
      channels = colorType === 2 ? 3 : 4
    } else if (type === "IDAT") compressedParts.push(bytes.slice(dataStart, dataEnd))
    else if (type === "IEND") {
      invariant(length === 0 && dataEnd + 4 === bytes.byteLength, "viewport PNG IEND is invalid")
      offset = bytes.byteLength
      break
    }
    offset = dataEnd + 4
  }
  invariant(width > 0 && height > 0 && channels > 0 && compressedParts.length > 0, "viewport PNG structure is incomplete")
  invariant(width <= 5120 && height <= 4096, "viewport PNG dimensions exceed the evidence bound")
  const compressedLength = compressedParts.reduce((sum, part) => sum + part.byteLength, 0)
  const compressed = new Uint8Array(compressedLength)
  let compressedOffset = 0
  for (const part of compressedParts) { compressed.set(part, compressedOffset); compressedOffset += part.byteLength }
  const inflated = new Uint8Array(await new Response(new Blob([compressed]).stream().pipeThrough(new DecompressionStream("deflate"))).arrayBuffer())
  const stride = width * channels
  invariant(inflated.byteLength === height * (stride + 1), "viewport PNG scanline length is invalid")
  const samples = new Uint8Array(height * stride)
  for (let y = 0; y < height; y += 1) {
    const filter = inflated[y * (stride + 1)]
    invariant(filter !== undefined && filter <= 4, "viewport PNG filter is unsupported")
    const encodedStart = y * (stride + 1) + 1
    const outputStart = y * stride
    for (let x = 0; x < stride; x += 1) {
      const encoded = inflated[encodedStart + x] ?? 0
      const left = x >= channels ? samples[outputStart + x - channels] ?? 0 : 0
      const above = y > 0 ? samples[outputStart - stride + x] ?? 0 : 0
      const upperLeft = y > 0 && x >= channels ? samples[outputStart - stride + x - channels] ?? 0 : 0
      const predictor = filter === 0 ? 0 : filter === 1 ? left : filter === 2 ? above : filter === 3 ? Math.floor((left + above) / 2) : paeth(left, above, upperLeft)
      samples[outputStart + x] = (encoded + predictor) & 0xff
    }
  }
  const rgb = new Uint8Array(width * height * 3)
  const alpha = new Uint8Array(width * height)
  for (let source = 0, destination = 0, pixel = 0; source < samples.byteLength; source += channels, destination += 3, pixel += 1) {
    rgb[destination] = samples[source] ?? 0
    rgb[destination + 1] = samples[source + 1] ?? 0
    rgb[destination + 2] = samples[source + 2] ?? 0
    alpha[pixel] = channels === 4 ? samples[source + 3] ?? 0 : 255
  }
  return Object.freeze({ width, height, rgb, alpha })
}

function bottomMetrics(image: DecodedPng): readonly PixelMetric[] {
  const regions = [
    { name: "bottom-25-percent" as const, height: Math.max(1, Math.ceil(image.height * 0.25)) },
    { name: "bottom-10-percent" as const, height: Math.max(1, Math.ceil(image.height * 0.1)) },
    { name: "bottom-32-pixels" as const, height: Math.min(32, image.height) },
    { name: "final-row" as const, height: 1 },
  ]
  return Object.freeze(regions.map((region) => {
    const y = image.height - region.height
    const pixels = image.width * region.height
    let alphaPixels = 0
    let opaqueBlackPixels = 0
    let alphaTotal = 0
    const samples = new Uint8Array(pixels * 4)
    let sample = 0
    for (let row = y; row < image.height; row += 1) {
      for (let x = 0; x < image.width; x += 1) {
        const pixel = row * image.width + x
        const rgb = pixel * 3
        const alpha = image.alpha[pixel] ?? 0
        const red = image.rgb[rgb] ?? 0
        const green = image.rgb[rgb + 1] ?? 0
        const blue = image.rgb[rgb + 2] ?? 0
        samples[sample++] = red
        samples[sample++] = green
        samples[sample++] = blue
        samples[sample++] = alpha
        if (alpha > 0) alphaPixels += 1
        if (alpha === 255 && red <= 2 && green <= 2 && blue <= 2) opaqueBlackPixels += 1
        alphaTotal += alpha
      }
    }
    return Object.freeze({
      name: region.name,
      x: 0,
      y,
      width: image.width,
      height: region.height,
      alphaOccupancy: Number((alphaPixels / pixels).toFixed(6)),
      opaqueBlackOccupancy: Number((opaqueBlackPixels / pixels).toFixed(6)),
      meanAlpha: Number((alphaTotal / pixels).toFixed(3)),
      sha256: createHash("sha256").update(samples).digest("hex"),
    })
  }))
}

test("profile TF2 HUD layout and composed viewport ownership", async ({ page }) => {
  const local = await loadLocalConfig()
  const outputDirectory = path.join(local.sourceCacheDir, "profiles", "hud", TARGET)
  await rm(outputDirectory, { recursive: true, force: true })
  await mkdir(outputDirectory, { recursive: true })

  await page.goto("/", { waitUntil: "domcontentloaded", timeout: 30_000 })
  await page.waitForFunction(() => {
    const phase = document.querySelector("main")?.getAttribute("data-phase")
    return phase === "MainMenu" || phase === "Failed"
  }, undefined, { timeout: 180_000, polling: 50 })
  expect(await page.locator("main").getAttribute("data-phase")).toBe("MainMenu")

  await page.keyboard.press("Backquote")
  const consoleEntry = page.locator("[aria-label='Console command']")
  await expect(consoleEntry).toBeVisible()
  await consoleEntry.fill(`map ${TARGET}`)
  await page.keyboard.press("Enter")
  await page.waitForFunction(() => {
    const main = document.querySelector<HTMLElement>("main")
    return main?.dataset.phase === "Ready" && main.dataset.gameui === "in-game"
  }, undefined, { timeout: 600_000, polling: 50 })
  await page.keyboard.press("Backquote")

  const client = await page.context().newCDPSession(page)
  const captures: Capture[] = []
  for (let index = 0; index < VIEWPORTS.length; index += 1) {
    const viewport = VIEWPORTS[index]!
    await page.setViewportSize({ width: viewport.width, height: viewport.height })
    await client.send("Emulation.setDeviceMetricsOverride", {
      width: viewport.width,
      height: viewport.height,
      deviceScaleFactor: viewport.devicePixelRatio,
      mobile: false,
      screenWidth: viewport.width,
      screenHeight: viewport.height,
    })
    await page.waitForFunction((expected) => devicePixelRatio === expected, viewport.devicePixelRatio)
    await page.evaluate(() => window.visualViewport?.dispatchEvent(new Event("resize")))
    await settle(page)
    const geometry = await page.evaluate(({ names, viewport }) => {
      const readRect = (element: Element) => {
        const value = element.getBoundingClientRect()
        return { x: value.x, y: value.y, width: value.width, height: value.height }
      }
      const required = <T extends Element>(selector: string): T => {
        const value = document.querySelector<T>(selector)
        if (!value) throw new Error(`Viewport evidence element ${selector} is unavailable`)
        return value
      }
      const host = required<HTMLElement>("[data-vgui-runtime='tf2-hud']")
      const panels: Record<string, { id: string; parent: string | null; local: ReturnType<typeof readRect>; rect: ReturnType<typeof readRect> }> = {}
      for (const name of names) {
        const element = host.querySelector<HTMLElement>(`[data-vgui-name="${name}"]`)
        if (!element) throw new Error(`TF2 HUD panel ${name} is unavailable`)
        panels[name] = {
          id: element.id,
          parent: element.parentElement?.dataset.vguiName ?? null,
          local: {
            x: Number.parseFloat(element.style.left),
            y: Number.parseFloat(element.style.top),
            width: Number.parseFloat(element.style.width),
            height: Number.parseFloat(element.style.height),
          },
          rect: readRect(element),
        }
      }
      const main = required<HTMLElement>("main")
      const ownerRecord = (selector: string) => required<HTMLElement>(selector).dataset.presentationViewport ?? ""
      const visual = window.visualViewport
      return {
        viewport,
        innerViewport: { width: innerWidth, height: innerHeight, devicePixelRatio },
        visualViewport: visual ? { x: visual.offsetLeft, y: visual.offsetTop, width: visual.width, height: visual.height, scale: visual.scale } : null,
        ownerViewport: main.dataset.presentationViewport ?? "",
        ownerRevision: Number(main.dataset.presentationViewportRevision),
        rectangles: {
          html: readRect(document.documentElement), body: readRect(document.body), app: readRect(required("#app")), main: readRect(main),
          canvas: readRect(required(".world-canvas")), hudLayer: readRect(required(".hud-layer")), hudHost: readRect(host),
        },
        ownerRecords: {
          canvas: ownerRecord(".world-canvas"), startup: ownerRecord(".startup-layer"), loading: ownerRecord(".loading-layer"),
          gameUi: ownerRecord(".gameui-layer"), hud: ownerRecord(".hud-layer"), options: ownerRecord(".options-layer"), developer: ownerRecord(".developer-layer"),
        },
        panels,
      }
    }, { names: PANELS, viewport })
    const canvas = await decodePng(await isolatedScreenshot(page, ".world-canvas"))
    const hud = await decodePng(await isolatedScreenshot(page, ".hud-layer"))
    const composed = await decodePng(new Uint8Array(await page.screenshot({
      omitBackground: false,
      path: path.join(outputDirectory, `hud-${index + 1}-${viewport.name}-${viewport.width}x${viewport.height}-dpr${viewport.devicePixelRatio}.png`),
    })))
    expect([composed.width, composed.height]).toEqual([viewport.width, viewport.height])
    expect([canvas.width, canvas.height]).toEqual([composed.width, composed.height])
    expect([hud.width, hud.height]).toEqual([composed.width, composed.height])
    captures.push(Object.freeze({ ...geometry, pixels: Object.freeze({ canvas: bottomMetrics(canvas), hud: bottomMetrics(hud), composed: bottomMetrics(composed) }) }))
  }

  const duplicateRevision = captures.at(-1)!.ownerRevision
  await page.evaluate(() => {
    window.visualViewport?.dispatchEvent(new Event("resize"))
    document.dispatchEvent(new Event("fullscreenchange"))
    document.dispatchEvent(new Event("pointerlockchange"))
  })
  await settle(page)
  expect(Number(await page.locator("main").getAttribute("data-presentation-viewport-revision"))).toBe(duplicateRevision)

  const report = Object.freeze({ schema: "playsrc-tf2-hud-layout-profile-v2", target: TARGET, captures: Object.freeze(captures) })
  await writeFile(path.join(outputDirectory, "report.json"), `${JSON.stringify(report, null, 2)}\n`)

  for (const capture of captures) {
    const expectedViewport = rect(0, 0, capture.viewport.width, capture.viewport.height)
    expect(capture.innerViewport).toEqual({ width: capture.viewport.width, height: capture.viewport.height, devicePixelRatio: capture.viewport.devicePixelRatio })
    if (capture.visualViewport) expect(capture.visualViewport).toMatchObject({ x: 0, y: 0, width: capture.viewport.width, height: capture.viewport.height, scale: 1 })
    for (const rectangle of Object.values(capture.rectangles)) expect(rectangle).toEqual(expectedViewport)
    expect(new Set(Object.values(capture.ownerRecords))).toEqual(new Set([`${capture.ownerRevision}:${capture.viewport.width}x${capture.viewport.height}@${capture.viewport.devicePixelRatio}`]))
    expect(capture.ownerViewport).toBe(`${capture.viewport.width}x${capture.viewport.height}@${capture.viewport.devicePixelRatio}`)
    const expectedPanels = expected(capture.viewport.width, capture.viewport.height)
    for (const name of PANELS) expect(capture.panels[name]!.local, `${name} at ${capture.viewport.name}`).toEqual(expectedPanels[name])
    for (const metric of capture.pixels.canvas) expect(metric.alphaOccupancy, `canvas ${metric.name} at ${capture.viewport.name}`).toBeGreaterThan(0)
    for (const metric of capture.pixels.composed) {
      expect(metric.alphaOccupancy, `composed ${metric.name} at ${capture.viewport.name}`).toBe(1)
      expect(metric.opaqueBlackOccupancy, `composed ${metric.name} at ${capture.viewport.name}`).toBeLessThan(1)
    }
  }
  const stableGeometry = (capture: Capture | undefined) => Object.fromEntries(Object.entries(capture?.panels ?? {}).map(([name, panel]) => [name, { id: panel.id, parent: panel.parent, local: panel.local }]))
  expect(stableGeometry(captures.at(-1))).toEqual(stableGeometry(captures[0]))
})
