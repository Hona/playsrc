import { mkdir, writeFile } from "node:fs/promises"
import path from "node:path"
import { chromium } from "@playwright/test"
import { loadLocalConfig } from "../../../../tools/playsrc/src/config"
import { acquireHeadedProfileLock, releaseHeadedProfileLock } from "../../../../tools/playsrc/src/profile-lock"
import { decodeScreenshot } from "../../../../tools/playsrc/profile/screenshot-pixels"

const config = await loadLocalConfig()
const directory = path.join(config.sourceCacheDir, "evidence", "tf2-browser-performance")
await mkdir(directory, { recursive: true })
const lockPath = path.join(directory, "chromium-profile.lock")
const lock = await acquireHeadedProfileLock(lockPath, "retained-raster-parity", 100_000)
let browser: Awaited<ReturnType<typeof chromium.launch>> | undefined
let server: ReturnType<typeof Bun.serve> | undefined
const deadline = setTimeout(() => { void browser?.close(); server?.stop(true) }, 70_000)
try {
  const bundle = await Bun.build({ entrypoints: [new URL("../tests/retained-raster-browser-fixture.ts", import.meta.url).pathname], target: "browser" })
  if (!bundle.success) throw new Error("Retained raster parity fixture failed to build")
  const script = await bundle.outputs[0]!.text()
  server = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch(request) {
    if (new URL(request.url).pathname === "/fixture.js") return new Response(script, { headers: { "content-type": "text/javascript" } })
    return new Response(`<!doctype html><html><head><title>Visible retained VGUI raster parity</title><style>
      body{margin:0;background:repeating-conic-gradient(#ae2222 0% 25%,#286797 0% 50%) 0/32px 32px}
      #panel{position:absolute;left:24px;top:20px;width:75vw;height:70vh;overflow:hidden;clip-path:inset(3px 5px 7px 9px);opacity:.75}
      .raster{position:absolute;left:0;top:0;width:100%;height:100%;pointer-events:none;z-index:1}
      #baseline{display:none}button{position:absolute;left:120px;top:60px;width:240px;height:32px;z-index:2;appearance:none;border:0;border-radius:0;outline:0;font:24px/30px Arial;background:#eeddbb;color:#222}
    </style></head><body><section id="panel"><canvas id="baseline" class="raster"></canvas><canvas id="retained" class="raster"></canvas><button aria-label="Retained foreground">Retained foreground</button></section><script type="module" src="/fixture.js"></script></body></html>`, { headers: { "content-type": "text/html" } })
  } })
  browser = await chromium.launch({ headless: false, channel: "msedge" })
  const evidence: unknown[] = []
  for (const deviceScaleFactor of [1, 1.25, 1.5, 2]) {
    const context = await browser.newContext({ viewport: { width: 1280, height: 720 }, deviceScaleFactor })
    const page = await context.newPage()
    await page.goto(server.url.href)
    await page.bringToFront()
    await page.waitForFunction(() => document.body.dataset.ready === "true")
    for (const viewport of [{ width: 1280, height: 720 }, { width: 1920, height: 1080 }]) {
      await page.setViewportSize(viewport)
      const button = page.getByRole("button", { name: "Retained foreground" })
      await button.click()
      await page.keyboard.press("Enter")
      await button.blur()
      const images = await page.screenshot()
      await page.evaluate(() => {
        document.querySelector<HTMLElement>("#retained")!.style.display = "none"
        document.querySelector<HTMLElement>("#baseline")!.style.display = "block"
      })
      const canvases = await page.screenshot()
      const before = decodeScreenshot(canvases)
      const after = decodeScreenshot(images)
      let differentChannels = 0
      if (before.width !== after.width || before.height !== after.height || before.channels !== after.channels) throw new Error("Retained raster screenshot extent differs")
      for (let index = 0; index < before.pixels.length; index += 1) if (before.pixels[index] !== after.pixels[index]) differentChannels += 1
      const state = await page.evaluate(() => ({ visible: document.visibilityState === "visible", focused: document.hasFocus(), dpr: devicePixelRatio, activations: document.body.dataset.activations, unchangedRasterWrites: Number(document.body.dataset.unchangedRasterWrites), changedRasterWrites: Number(document.body.dataset.changedRasterWrites), rasterMutations: Number(document.body.dataset.rasterMutations ?? 0),
        clip: getComputedStyle(document.querySelector("#panel")!).clipPath,
        opacity: getComputedStyle(document.querySelector("#panel")!).opacity,
        transform: getComputedStyle(document.querySelector("#panel")!).transform,
        rasterBounds: document.querySelector("#baseline")!.getBoundingClientRect().toJSON(),
      }))
      if (!state.visible || !state.focused || state.dpr !== deviceScaleFactor || Number(state.activations) < 2) throw new Error("Retained raster input/visibility parity failed")
      if (state.unchangedRasterWrites !== 0 || state.rasterMutations !== 0) throw new Error("Unchanged raster reset or repainted its retained canvas")
      if (state.changedRasterWrites !== 2) throw new Error("Changed authored raster pixels were not published exactly once")
      evidence.push({ viewport, deviceScaleFactor, physical: { width: after.width, height: after.height }, differentChannels, ...state })
      await writeFile(path.join(directory, `retained-raster-${viewport.width}-${deviceScaleFactor}.png`), images)
      if (differentChannels !== 0) await writeFile(path.join(directory, `retained-raster-${viewport.width}-${deviceScaleFactor}-baseline.png`), canvases)
      if (differentChannels !== 0) throw new Error(`Retained raster alpha/depth pixels differ at DPR ${deviceScaleFactor}: ${differentChannels} channels`)
      await page.evaluate(() => {
        document.querySelector<HTMLElement>("#retained")!.style.display = "block"
        document.querySelector<HTMLElement>("#baseline")!.style.display = "none"
      })
    }
    await context.close()
  }
  await writeFile(path.join(directory, "retained-raster-parity.json"), `${JSON.stringify(evidence, null, 2)}\n`)
  console.log(JSON.stringify(evidence))
} finally {
  clearTimeout(deadline)
  await browser?.close()
  server?.stop(true)
  await releaseHeadedProfileLock(lockPath, lock.token)
}
