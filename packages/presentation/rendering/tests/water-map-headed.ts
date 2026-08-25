import { createHash } from "node:crypto"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { chromium } from "@playwright/test"
import { loadLocalConfig } from "../../../../tools/playsrc/src/config"
import { buildTf2Wasm } from "../../../../tools/playsrc/src/tf2-wasm-build"

const root = path.resolve(import.meta.dir, "../../../..")
const configuration = await loadLocalConfig(root)
const evidenceDirectory = path.join(configuration.sourceCacheDir, "evidence", "tf2-water-rendering")
await mkdir(evidenceDirectory, { recursive: true })
const graphPath = path.join(configuration.sourceCacheDir, "browser-bundles", "jump_beef.graph.json")
const graphBytes = await readFile(graphPath)
const graph = JSON.parse(graphBytes.toString("utf8")) as {
  target: string
  contentBuild: string
  chunks: readonly { encodedSha256: string; encodedByteLength: string }[]
}
if (graph.target !== "jump_beef" || graph.contentBuild !== "24245096") {
  throw new Error("Headed Water map evidence requires the exact configured jump_beef graph")
}
const wasmPath = await buildTf2Wasm(configuration, false)
const bspSha256 = "b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959"
const bspPath = path.join(configuration.sourceCacheDir, "objects", "sha256", bspSha256.slice(0, 2), bspSha256)
const descriptors = new Map(graph.chunks.map((chunk) => [chunk.encodedSha256, chunk]))

const build = await Bun.build({
  entrypoints: [path.join(import.meta.dir, "water-map-headed-entry.ts")],
  target: "browser",
  format: "esm",
  minify: false,
})
if (!build.success || build.outputs.length !== 1) {
  throw new Error(`Headed Water map browser bundle failed: ${build.logs.map(String).join("; ")}`)
}
const source = await build.outputs[0]!.text()
const html = path.join(evidenceDirectory, "water-map-headed.html")
await writeFile(html, `<!doctype html><html><head><meta charset="utf-8"><title>jump_beef Source Water headed pixel evidence</title><style>html,body{margin:0;background:#101820}canvas{display:block;width:960px;height:540px}</style></head><body><canvas width="960" height="540"></canvas><script type="module">${source.replaceAll("</script", "<\\/script")}</script></body></html>`)

const browser = await chromium.launch({
  channel: "msedge",
  headless: false,
  args: ["--enable-unsafe-webgpu"],
})
try {
  const page = await browser.newPage({ viewport: { width: 960, height: 540 }, deviceScaleFactor: 1 })
  page.on("pageerror", (error) => console.error(`[water-map-headed] page error: ${error.stack || error.message || String(error)}`))
  page.on("console", (message) => console.error(`[water-map-headed] ${message.type()}: ${message.text()}`))
  page.on("requestfailed", (request) => console.error(`[water-map-headed] request failed: ${request.url()} ${request.failure()?.errorText ?? ""}`))
  await page.route("https://source-water-evidence.invalid/**", async (route) => {
    const request = new URL(route.request().url())
    let bytes: Buffer
    let contentType = "application/octet-stream"
    if (request.pathname === "/graph") {
      bytes = graphBytes
      contentType = "application/json"
    } else if (request.pathname === "/bsp") {
      bytes = await readFile(bspPath)
      if (bytes.byteLength !== 33_379_388 || createHash("sha256").update(bytes).digest("hex") !== bspSha256) {
        throw new Error("Headed Water BSP does not match the configured fixed source")
      }
    } else if (request.pathname === "/wasm") {
      bytes = await readFile(wasmPath)
      contentType = "application/wasm"
    } else if (request.pathname.startsWith("/chunks/")) {
      const identity = request.pathname.slice("/chunks/".length)
      const descriptor = descriptors.get(identity)
      if (!descriptor) throw new Error(`Undeclared Water evidence chunk ${identity}`)
      bytes = await readFile(path.join(configuration.sourceCacheDir, "browser-bundles", "jump_beef.graph", "objects", identity))
      if (bytes.byteLength !== Number(descriptor.encodedByteLength) || createHash("sha256").update(bytes).digest("hex") !== identity) {
        throw new Error(`Headed Water evidence chunk ${identity} failed integrity verification`)
      }
    } else {
      throw new Error(`Undeclared Water evidence request ${request.pathname}`)
    }
    await route.fulfill({
      status: 200,
      body: bytes,
      contentType,
      headers: { "access-control-allow-origin": "*" },
    })
  })
  await page.bringToFront()
  await page.goto(`file://${html}`, { waitUntil: "load", timeout: 30_000 })
  await page.waitForFunction(() => (window as any).__sourceWaterMapReady === true, undefined, { timeout: 180_000 })
  const spawn = await page.evaluate(async () => (window as any).__sourceWaterMapWarmSpawn()) as {
    milliseconds: number
    passes: readonly string[]
    drawableSurfaces: number
  }
  if (spawn.passes.join(",") !== "main") throw new Error("Configured Water evidence spawn did not execute its ordinary main view")
  await page.waitForTimeout(250)

  const captures = []
  for (const name of ["above-frame-0", "above-frame-30", "below", "crossing"] as const) {
    const result = await page.evaluate(async (scenario) => (window as any).__sourceWaterMapScenario(scenario), name) as {
      name: string
      normalFrame: number
      eyeInVolume: boolean
      plannedPasses: readonly string[]
      renderedPasses: readonly string[]
      stateRestored: boolean
      nearPlaneIntersects: boolean
      targets: Readonly<{ reflection: readonly number[]; refraction: readonly number[] }>
      targetSamples: readonly Readonly<{ x: number; y: number; reflection: readonly number[]; refraction: readonly number[] }>[]
      [key: string]: unknown
    }
    await page.waitForTimeout(250)
    const screenshot = await page.screenshot({ animations: "disabled", caret: "hide" })
    const filename = `jump-beef-${name}.png`
    await writeFile(path.join(evidenceDirectory, filename), screenshot)
    const pixels = await page.evaluate(async (encoded) => {
      const bytes = Uint8Array.from(atob(encoded), (value) => value.charCodeAt(0))
      const bitmap = await createImageBitmap(new Blob([bytes], { type: "image/png" }))
      const canvas = new OffscreenCanvas(bitmap.width, bitmap.height)
      const context = canvas.getContext("2d", { willReadFrequently: true })
      if (!context) throw new Error("Water map screenshot decoder is unavailable")
      context.drawImage(bitmap, 0, 0)
      const samples = [
        { name: "center", x: 480, y: 270 },
        { name: "upper", x: 480, y: 135 },
        { name: "lower", x: 480, y: 405 },
        { name: "left", x: 240, y: 270 },
        { name: "right", x: 720, y: 270 },
      ].map(({ name, x, y }) => ({ name, x, y, rgba: Array.from(context.getImageData(x, y, 1, 1).data) }))
      return samples
    }, screenshot.toString("base64"))

    if (result.plannedPasses.join(",") !== result.renderedPasses.join(",")) {
      throw new Error(`${name} Water pass order differs: ${result.plannedPasses.join(",")} / ${result.renderedPasses.join(",")}`)
    }
    if (!result.stateRestored) throw new Error(`${name} Water view did not restore scene state`)
    if (name.startsWith("above") && result.renderedPasses.join(",") !== "reflection,refraction,main") {
      throw new Error(`${name} omitted its real Source reflection/refraction auxiliary views`)
    }
    if (name === "below" && (!result.eyeInVolume || result.renderedPasses.join(",") !== "refraction,main")) {
      throw new Error("Below-water transition did not select Source underwater refraction/main views")
    }
    if (name === "crossing" && (!result.nearPlaneIntersects || !result.renderedPasses.includes("intersection"))) {
      throw new Error("Water near-plane transition omitted its Source intersection view")
    }
    if (name === "above-frame-0" && result.normalFrame !== 0) throw new Error("Authored Water frame zero was not selected")
    if (name === "above-frame-30" && result.normalFrame !== 30) throw new Error("Authored Water frame thirty was not selected")
    if (name.startsWith("above")) {
      const expectedDepth = [255, 125, 202]
      for (let index = 0; index < expectedDepth.length; index += 1) {
        const observed = result.targetSamples[index]?.refraction[3]
        if (observed === undefined || Math.abs(observed - expectedDepth[index]!) > 1) {
          throw new Error(`${name} authored refraction depth-alpha differs at sample ${index}: expected ${expectedDepth[index]}, observed ${observed}`)
        }
      }
      if (result.targetSamples.some((sample) => sample.reflection[3] !== 255)) {
        throw new Error(`${name} reflection target did not retain opaque alpha`)
      }
    }
    captures.push(Object.freeze({
      ...result,
      pixels: Object.freeze(pixels),
      capture: filename,
      captureSha256: createHash("sha256").update(screenshot).digest("hex"),
      captureBytes: screenshot.byteLength,
    }))
  }

  const first = captures.find((capture) => capture.name === "above-frame-0")
  const animated = captures.find((capture) => capture.name === "above-frame-30")
  if (!first || !animated || first.captureSha256 === animated.captureSha256) {
    throw new Error("Authored Water frame animation did not change the actual visible map pixels")
  }

  const frameTimings = await page.evaluate(async () => (window as any).__sourceWaterMapBenchmark()) as readonly {
    name: string
    frames: number
    p50Milliseconds: number
    p95Milliseconds: number
    p99Milliseconds: number
    maximumMilliseconds: number
    samples: readonly unknown[]
  }[]
  await page.evaluate(async () => (window as any).__sourceWaterMapDispose())
  const report = Object.freeze({
    schema: "playsrc-source-water-map-headed-pixel-evidence-v2",
    headed: true,
    runtime: "Chromium WebGPU",
    target: "jump_beef",
    bspSha256,
    viewport: Object.freeze({ width: 960, height: 540, deviceScaleFactor: 1 }),
    spawn,
    captures: Object.freeze(captures),
    frameTimings: Object.freeze(frameTimings),
  })
  await writeFile(path.join(evidenceDirectory, "water-map-headed.json"), `${JSON.stringify(report, null, 2)}\n`)
  console.log(`SOURCE_WATER_MAP_HEADED ${JSON.stringify({
    ...report,
    frameTimings: frameTimings.map(({ samples, ...distribution }) => distribution),
  })}`)
  const budgetMilliseconds = 1000 / 120
  const rejected = frameTimings.filter((timing) => timing.p95Milliseconds > budgetMilliseconds)
  if (rejected.length > 0) {
    throw new Error(`Water transitions exceed the ${budgetMilliseconds.toFixed(3)} ms frame-work budget: ${rejected.map((timing) => `${timing.name} p95=${timing.p95Milliseconds.toFixed(3)}ms`).join(", ")}`)
  }
} finally {
  await browser.close()
}
