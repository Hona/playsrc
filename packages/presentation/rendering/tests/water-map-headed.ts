import { createHash } from "node:crypto"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { chromium } from "@playwright/test"
import { loadLocalConfig } from "../../../../tools/playsrc/src/config"
import { buildTf2Wasm } from "../../../../tools/playsrc/src/tf2-wasm-build"
import { acquireHeadedProfileLock, releaseHeadedProfileLock } from "../../../../tools/playsrc/src/profile-runner"
import { buildSourceBundle } from "../../../../tools/playsrc/src/source-bundle"

const started = Date.now()
const root = path.resolve(import.meta.dir, "../../../..")
const configuration = await loadLocalConfig(root)
const evidenceDirectory = path.join(configuration.sourceCacheDir, "evidence", "tf2-water-rendering")
await mkdir(evidenceDirectory, { recursive: true })
const graphPath = (await buildSourceBundle(configuration, "jump_beef")).graphPath
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
let documentHtml = ""
const server = Bun.serve({
  hostname: "127.0.0.1",
  port: 0,
  fetch(request) {
    const pathname = new URL(request.url).pathname
    if (pathname === "/") return new Response(documentHtml, { headers: { "content-type": "text/html; charset=utf-8", "cross-origin-opener-policy": "same-origin", "cross-origin-embedder-policy": "require-corp" } })
    if (pathname === "/favicon.ico") return new Response(null, { status: 204 })
    if (pathname === "/graph") return new Response(graphBytes, { headers: { "content-type": "application/json", "access-control-allow-origin": "*" } })
    if (pathname === "/bsp") return new Response(Bun.file(bspPath), { headers: { "access-control-allow-origin": "*" } })
    if (pathname === "/wasm") return new Response(Bun.file(wasmPath), { headers: { "content-type": "application/wasm", "access-control-allow-origin": "*" } })
    if (pathname.startsWith("/chunks/")) {
      const identity = pathname.slice("/chunks/".length)
      if (!descriptors.has(identity)) return new Response("undeclared Water evidence chunk", { status: 404 })
      return new Response(Bun.file(path.join(configuration.sourceCacheDir, "browser-bundles", "jump_beef.graph", "objects", identity)), { headers: { "access-control-allow-origin": "*" } })
    }
    return new Response("undeclared Water evidence request", { status: 404 })
  },
})
documentHtml = `<!doctype html><html><head><meta charset="utf-8"><title>jump_beef Source Water headed pixel evidence</title><style>html,body{margin:0;background:#101820}canvas{display:block;width:960px;height:540px}</style></head><body><canvas width="960" height="540"></canvas><script>window.__sourceWaterEvidenceOrigin=${JSON.stringify(server.url.origin)}</script><script type="module">${source.replaceAll("</script", "<\\/script")}</script></body></html>`
await writeFile(html, documentHtml)

const lockDirectory = path.join(configuration.sourceCacheDir, "evidence", "tf2-browser-performance")
await mkdir(lockDirectory, { recursive: true })
const lockPath = path.join(lockDirectory, "chromium-profile.lock")
const remaining = () => 175_000 - (Date.now() - started)
if (remaining() < 1) throw new Error("Headed Water evidence exhausted its bounded deadline before acquiring the machine-wide lock")
const lock = await acquireHeadedProfileLock(lockPath, "jump-beef-water-map-headed", Math.min(120_000, remaining()))
const browser = await chromium.launch({
  channel: "msedge",
  headless: false,
  args: ["--enable-unsafe-webgpu"],
})
const deadline = setTimeout(() => { void browser.close() }, Math.max(1, remaining()))
try {
  const page = await browser.newPage({ viewport: { width: 960, height: 540 }, deviceScaleFactor: 1 })
  page.on("pageerror", (error) => console.error(`[water-map-headed] page error: ${error.stack || error.message || String(error)}`))
  page.on("console", (message) => console.error(`[water-map-headed] ${message.type()}: ${message.text()}`))
  page.on("requestfailed", (request) => console.error(`[water-map-headed] request failed: ${request.url()} ${request.failure()?.errorText ?? ""}`))
  await page.bringToFront()
  await page.goto(server.url.href, { waitUntil: "load", timeout: 30_000 })
  await Promise.race([
    page.waitForFunction(() => (window as any).__sourceWaterMapReady === true, undefined, { timeout: Math.max(1, remaining()) }),
    page.waitForEvent("pageerror", { timeout: Math.max(1, remaining()) }).then((error) => { throw error }),
  ])
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

  const overhead = []
  const overheadCases = [
    ...[-2100, -1900, -1500].flatMap((height) => [45, 75, 89, 90].map((pitch) => ({ position: [-4800, 3000, height] as const, pitch }))),
    ...[-5200, -4400].map((x) => ({ position: [x, 3000, -1500] as const, pitch: 90 })),
    { position: [-4400, 3000, -1000] as const, pitch: 90 },
  ]
  for (const scenario of overheadCases) {
    const present = await page.evaluate(({ position, pitch }) => (window as any).__sourceWaterMapOverhead(position, pitch, false), scenario)
    if (!present.stateRestored || present.passes.join(",") !== "reflection,refraction,main") {
      throw new Error(`Overhead Water ${scenario.position.join(",")}/${scenario.pitch} lost its authored view plan`)
    }
    const visible = await page.screenshot({ animations: "disabled", caret: "hide" })
    const absent = await page.evaluate(({ position, pitch }) => (window as any).__sourceWaterMapOverhead(position, pitch, true), scenario)
    if (!absent.stateRestored || absent.passes.join(",") !== "main") {
      throw new Error(`Overhead Water ${scenario.position.join(",")}/${scenario.pitch} did not isolate its opaque depth comparison`)
    }
    const opaque = await page.screenshot({ animations: "disabled", caret: "hide" })
    const comparison = await page.evaluate(async ([first, second]) => {
      const decode = async (encoded: string) => {
        const bitmap = await createImageBitmap(new Blob([Uint8Array.from(atob(encoded), (value) => value.charCodeAt(0))], { type: "image/png" }))
        const canvas = new OffscreenCanvas(bitmap.width, bitmap.height)
        const context = canvas.getContext("2d", { willReadFrequently: true })!
        context.drawImage(bitmap, 0, 0)
        return context.getImageData(0, 0, bitmap.width, bitmap.height).data
      }
      const [water, walls] = await Promise.all([decode(first!), decode(second!)])
      let changed = 0, unchanged = 0
      for (let index = 0; index < water.length; index += 4) {
        if (water[index] !== walls[index] || water[index + 1] !== walls[index + 1] || water[index + 2] !== walls[index + 2]) changed += 1
        else unchanged += 1
      }
      const center = (270 * 960 + 480) * 4
      return { changedPixels: changed, unchangedPixels: unchanged, waterCenter: Array.from(water.subarray(center, center + 4)), opaqueCenter: Array.from(walls.subarray(center, center + 4)) }
    }, [visible.toString("base64"), opaque.toString("base64")])
    if (comparison.changedPixels < 64 || comparison.opaqueCenter[3] !== 255) {
      throw new Error(`Overhead Water ${scenario.position.join(",")}/${scenario.pitch} lost authored pixels or opaque wall occlusion: ${JSON.stringify(comparison)}`)
    }
    if (scenario.pitch >= 89 && scenario.position[0] !== -4400 && comparison.waterCenter.join(",") === comparison.opaqueCenter.join(",")) {
      throw new Error(`Overhead Water ${scenario.position.join(",")}/${scenario.pitch} disappeared beneath the centered straight-down ray`)
    }
    if (scenario.position[0] === -4400 && comparison.unchangedPixels < 64) {
      throw new Error(`Overhead Water ${scenario.position.join(",")}/${scenario.pitch} did not preserve its opaque lateral wall pixels`)
    }
    const filename = `jump-beef-overhead-${scenario.position[0]}-${scenario.position[2]}-${scenario.pitch}.png`
    await writeFile(path.join(evidenceDirectory, filename), visible)
    overhead.push(Object.freeze({ ...present, ...comparison, capture: filename, captureSha256: createHash("sha256").update(visible).digest("hex") }))
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
    overhead: Object.freeze(overhead),
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
  clearTimeout(deadline)
  await browser.close()
  await releaseHeadedProfileLock(lockPath, lock.token)
  server.stop(true)
}
