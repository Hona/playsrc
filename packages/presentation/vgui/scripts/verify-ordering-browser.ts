import { createHash } from "node:crypto"
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import path from "node:path"
import { fileURLToPath } from "node:url"
import { chromium } from "@playwright/test"
import { loadLocalConfig } from "../../../../tools/playsrc/src/config"
import { createEvidenceDirectory } from "../../../../tools/playsrc/src/evidence-directory"

const packageRoot = fileURLToPath(new URL("../", import.meta.url))
const publicRoot = path.resolve(packageRoot, "../../..")
const config = await loadLocalConfig(publicRoot)
const evidenceRoot = await createEvidenceDirectory(config, "vgui-ordering")
console.log(`Evidence: ${evidenceRoot}`)
const temporaryRoot = path.join(config.sourceCacheDir, "jobs")
await mkdir(temporaryRoot, { recursive: true })
const temporary = await mkdtemp(path.join(temporaryRoot, "vgui-ordering-"))
const fontSources = Object.freeze([
  ["tf2build.ttf", 61_696, "23faa58a08c929c0b6638f581488e49399cd7a390c70cb9debdaf8371a95e0c6"],
  ["tf2.ttf", 68_828, "1c36e9e8f8e305fb0a889889bf55a06d0ab9aba13f88d5188ddf87122d5c1af1"],
] as const)
const fonts = new Map<string, Uint8Array>()
for (const [name, size, hash] of fontSources) {
  const location = path.join(config.tf2Dir, "resource", name)
  const bytes = new Uint8Array(await readFile(location))
  if (bytes.byteLength !== size || createHash("sha256").update(bytes).digest("hex") !== hash) {
    throw new Error(`Configured font identity differs: resource/${name}`)
  }
  fonts.set(name, bytes)
}

const bundle = await Bun.build({
  entrypoints: [path.join(packageRoot, "tests", "ordering-browser-fixture.ts")],
  outdir: temporary,
  target: "browser",
  minify: false,
  plugins: [{
    name: "ordering-workspace-aliases",
    setup(build) {
      build.onResolve({ filter: /^@playsrc\/vgui$/ }, () => ({ path: path.join(packageRoot, "src", "index.ts") }))
      build.onResolve({ filter: /^@playsrc\/game-tf2-browser\/content-build$/ }, () => ({ path: path.join(publicRoot, "games", "tf2", "browser", "src", "content-build.ts") }))
    },
  }],
})
if (!bundle.success) throw new Error("VGUI ordering browser fixture bundle failed")
const html = `<!doctype html><html><head><meta charset="utf-8"><title>Visible VGUI ordering verification</title><style>
html,body,#workspace{width:1280px;height:720px;margin:0;overflow:hidden}body{background:#171a1b}#workspace{position:relative}
.layer{position:absolute;inset:0;pointer-events:none}.ammo-layer{z-index:20}.options-layer{z-index:30}.developer-layer{z-index:40}
</style></head><body><main id="workspace"><section id="ammo" class="layer ammo-layer"></section><section id="options" class="layer options-layer"></section><section id="developer" class="layer developer-layer"></section></main><script type="module" src="/ordering-browser-fixture.js"></script></body></html>`
await writeFile(path.join(temporary, "index.html"), html)
const server = Bun.serve({
  hostname: "127.0.0.1",
  port: 4176,
  async fetch(request) {
    const url = new URL(request.url)
    const font = fonts.get(url.pathname.slice("/fonts/".length))
    if (url.pathname.startsWith("/fonts/") && font) return new Response(font, { headers: { "content-type": "font/ttf" } })
    const file = url.pathname === "/" ? "index.html" : url.pathname.slice(1)
    if (file !== "index.html" && file !== "ordering-browser-fixture.js") return new Response("not found", { status: 404 })
    return new Response(await readFile(path.join(temporary, file)), { headers: { "content-type": file.endsWith(".js") ? "text/javascript" : "text/html" } })
  },
})

let browser: Awaited<ReturnType<typeof chromium.launch>> | undefined
try {
  await mkdir(evidenceRoot, { recursive: true })
  browser = await chromium.launch({ headless: false })
  const page = await browser.newPage({ viewport: { width: 1_280, height: 720 }, deviceScaleFactor: 1 })
  await page.goto("http://127.0.0.1:4176/", { waitUntil: "load" })
  await page.waitForFunction(() => {
    const state = window.vguiOrderingEvidence?.status()
    return state?.ready || state?.error
  }, undefined, { timeout: 30_000 })
  const status = await page.evaluate(() => window.vguiOrderingEvidence.status())
  if (!status.ready) throw new Error(status.error ?? "VGUI ordering browser fixture failed")

  const captureGlyphs = async (names: readonly string[], captureName: string) => {
    await page.evaluate(() => window.vguiOrderingEvidence.setPairVisibility(true, false))
    const foreground = await page.screenshot({ animations: "disabled" })
    await page.evaluate(() => window.vguiOrderingEvidence.setPairVisibility(false, true))
    const shadow = await page.screenshot({ animations: "disabled" })
    await page.evaluate(() => window.vguiOrderingEvidence.setPairVisibility(true, true))
    const composed = await page.screenshot({ path: path.join(evidenceRoot, captureName), animations: "disabled" })
    return page.evaluate(async ({ foreground, shadow, composed, names }) => {
      const decode = async (bytes: number[]) => {
        const image = await createImageBitmap(new Blob([new Uint8Array(bytes)], { type: "image/png" }))
        const canvas = new OffscreenCanvas(image.width, image.height)
        const context = canvas.getContext("2d", { willReadFrequently: true })!
        context.drawImage(image, 0, 0)
        return context.getImageData(0, 0, image.width, image.height)
      }
      const [front, back, result] = await Promise.all([decode(foreground), decode(shadow), decode(composed)])
      const background = [23, 26, 27, 255]
      const tan = [235, 226, 202, 255]
      const opaqueBlack = [46, 43, 42, 255]
      const translucentBlack = [5, 6, 6, 255]
      const same = (image: ImageData, offset: number, color: readonly number[]) => color.every((value, index) => image.data[offset + index] === value)
      const pixel = (image: ImageData, offset: number) => Array.from(image.data.slice(offset, offset + 4))
      const probe = (name: string) => {
        const element = document.querySelector<HTMLElement>(`[data-vgui-name="${name}"]`)!
        const bounds = element.getBoundingClientRect()
        const translucent = name === "AmmoInReserve"
        const isShadow = (offset: number) => same(back, offset, translucent ? translucentBlack : opaqueBlack)
        let overlap: { x: number; y: number; foreground: number[]; shadow: number[]; composed: number[] } | null = null
        let offsetShadow: { x: number; y: number; foreground: number[]; shadow: number[]; composed: number[] } | null = null
        let foregroundSolid = 0
        let shadowSolid = 0
        for (let y = Math.floor(bounds.y); y < Math.ceil(bounds.bottom); y += 1) {
          for (let x = Math.floor(bounds.x); x < Math.ceil(bounds.right); x += 1) {
            const index = (y * result.width + x) * 4
            if (same(front, index, tan)) foregroundSolid += 1
            if (isShadow(index)) shadowSolid += 1
            if (!overlap && same(front, index, tan) && isShadow(index)) {
              overlap = { x, y, foreground: pixel(front, index), shadow: pixel(back, index), composed: pixel(result, index) }
            }
            if (!offsetShadow && same(front, index, background) && isShadow(index)) {
              offsetShadow = { x, y, foreground: pixel(front, index), shadow: pixel(back, index), composed: pixel(result, index) }
            }
          }
        }
        return { bounds: bounds.toJSON(), foregroundSolid, shadowSolid, overlap, offsetShadow }
      }
      return Object.fromEntries(names.map((name) => [name, probe(name)]))
    }, { foreground: [...foreground], shadow: [...shadow], composed: [...composed], names: [...names] })
  }

  const clip = await captureGlyphs(["AmmoInClip", "AmmoInReserve"], "ordering.png")
  await page.evaluate(() => window.vguiOrderingEvidence.setAmmoMode("no-clip"))
  const noClip = await captureGlyphs(["AmmoNoClip"], "ordering-no-clip.png")
  await page.evaluate(() => window.vguiOrderingEvidence.setAmmoMode("clip"))
  const pixels = { clip: clip.AmmoInClip!, reserve: clip.AmmoInReserve!, noClip: noClip.AmmoNoClip! }
  const ordering = await page.evaluate(() => {
    const snapshot = window.vguiOrderingEvidence.ammoSnapshot() as {
      panels: readonly { id: number; name: string; z: number; children: readonly number[] }[]
    }
    const parent = snapshot.panels.find((panel) => panel.name === "HudWeaponAmmo")!
    const model = parent.children.map((identity) => {
      const panel = snapshot.panels.find((candidate) => candidate.id === identity)!
      return { name: panel.name, z: panel.z }
    })
    const element = document.querySelector<HTMLElement>('[data-vgui-name="HudWeaponAmmo"]')!
    const dom = [...element.children].map((child) => (child as HTMLElement).dataset.vguiName).filter(Boolean)
    return { model, dom }
  })

  await page.evaluate(() => window.vguiOrderingEvidence.showConsole())
  await page.evaluate(() => window.vguiOrderingEvidence.showOptions())
  const optionsForeground = await page.evaluate(() => window.vguiOrderingEvidence.windowState())
  const windowCapture = await page.screenshot({ path: path.join(evidenceRoot, "ordering-windows.png"), animations: "disabled" })
  await page.click('[data-vgui-name="ApplyButton"]')
  const optionRequests = await page.evaluate(() => window.vguiOrderingEvidence.optionRequests())
  await page.evaluate(() => window.vguiOrderingEvidence.setOptionsModal(true))
  await page.evaluate(() => window.vguiOrderingEvidence.showConsole())
  const modalForeground = await page.evaluate(() => window.vguiOrderingEvidence.windowState())
  await page.evaluate(() => window.vguiOrderingEvidence.setOptionsModal(false))
  await page.evaluate(() => window.vguiOrderingEvidence.showConsole())
  const consoleForeground = await page.evaluate(() => window.vguiOrderingEvidence.windowState())
  const consoleCapture = await page.screenshot({ animations: "disabled" })
  await page.evaluate(() => window.vguiOrderingEvidence.hideOptions())
  const hiddenOptions = await page.evaluate(() => window.vguiOrderingEvidence.windowState())
  await page.evaluate(() => window.vguiOrderingEvidence.showOptions())
  const resumedOptions = await page.evaluate(() => window.vguiOrderingEvidence.windowState())
  const windowPixels = await page.evaluate(async ({ options, developer, x, y }) => {
    const sample = async (bytes: number[]) => {
      const image = await createImageBitmap(new Blob([new Uint8Array(bytes)], { type: "image/png" }))
      const canvas = new OffscreenCanvas(image.width, image.height)
      const context = canvas.getContext("2d", { willReadFrequently: true })!
      context.drawImage(image, 0, 0)
      return Array.from(context.getImageData(x, y, 1, 1).data)
    }
    return { x, y, options: await sample(options), developer: await sample(developer) }
  }, {
    options: [...windowCapture],
    developer: [...consoleCapture],
    x: (optionsForeground as { x: number }).x,
    y: (optionsForeground as { y: number }).y,
  })
  const report = {
    schema: "playsrc-vgui-ordering-browser-evidence-v1",
    contentBuild: "24245096",
    patch: "10828683",
    sdkRevision: "88fa198fba3fb85d46d4c95018254693fdc3af0a",
    ammoResource: { logicalPath: "resource/ui/hudammoweapons.res", sha256: "a23a98f009dd34ac8c94e7149b1ded56eb9ed66e03d583fcd9c2ab68c3cb7734" },
    fonts: fontSources.map(([name, byteLength, sha256]) => ({ logicalPath: `resource/${name}`, byteLength, sha256 })),
    viewport: { width: 1_280, height: 720, devicePixelRatio: 1 },
    ordering,
    pixels,
    windows: { optionsForeground, modalForeground, consoleForeground, hiddenOptions, resumedOptions, optionRequests, pixels: windowPixels, screenshotBytes: windowCapture.byteLength },
    userAgent: await page.evaluate(() => navigator.userAgent),
  }
  await writeFile(path.join(evidenceRoot, "ordering-browser-evidence.json"), `${JSON.stringify(report, null, 2)}\n`)
  console.log(JSON.stringify(report, null, 2))
  if (ordering.model.some((panel, index) => panel.name !== ordering.dom[index])) {
    throw new Error("Configured DOM sibling paint order differs from the runtime model")
  }
  for (const [foreground, shadow] of [["AmmoInClip", "AmmoInClipShadow"], ["AmmoInReserve", "AmmoInReserveShadow"], ["AmmoNoClip", "AmmoNoClipShadow"]] as const) {
    if (ordering.dom.indexOf(shadow) >= ordering.dom.indexOf(foreground)) {
      throw new Error(`Configured equal-z ${shadow} does not paint behind ${foreground}`)
    }
  }
  for (const [name, pair] of Object.entries(pixels)) {
    if (!pair.overlap) throw new Error(`Configured ${name} foreground and shadow have no shared solid glyph-interior pixel`)
    if (!pair.offsetShadow) throw new Error(`Configured ${name} shadow has no visible authored one-pixel dark edge`)
    if (pair.overlap.composed.some((channel, index) => channel !== pair.overlap!.foreground[index])) {
      throw new Error(`Configured ${name} shadow occludes the bright glyph interior at (${pair.overlap.x},${pair.overlap.y}): ${JSON.stringify(pair.overlap.composed)}`)
    }
    if (pair.offsetShadow.composed.some((channel, index) => channel !== pair.offsetShadow!.shadow[index])) {
      throw new Error(`Configured ${name} authored one-pixel shadow offset is not visible behind the foreground`)
    }
  }
  if ((optionsForeground as { target: string | null }).target !== "ApplyButton") {
    throw new Error(`Active Options ApplyButton is covered by ${(optionsForeground as { target: string | null }).target ?? "no panel"}`)
  }
  if (!optionRequests.some((request) => request.kind === "command" && request.command === "Apply")) {
    throw new Error("A real browser pointer click did not dispatch the active Options Apply command")
  }
  if ((modalForeground as { target: string | null; developerInert: boolean }).target !== "ApplyButton"
    || !(modalForeground as { target: string | null; developerInert: boolean }).developerInert) {
    throw new Error("The application-modal Options window did not retain foreground and make the visible console inert")
  }
  if ((consoleForeground as { target: string | null }).target === "ApplyButton") {
    throw new Error("Reactivated developer console did not return above the Options window")
  }
  if ((hiddenOptions as { optionsVisible: boolean; optionsLayer: string; developerLayer: string }).optionsVisible
    || (hiddenOptions as { optionsLayer: string }).optionsLayer !== "30"
    || (hiddenOptions as { developerLayer: string }).developerLayer !== "40") {
    throw new Error("Hiding Options did not restore the original browser stacking contexts")
  }
  if ((resumedOptions as { target: string | null }).target !== "ApplyButton") {
    throw new Error("Reactivating a hidden Options root did not restore its window foreground")
  }
  if (windowPixels.options.every((channel, index) => channel === windowPixels.developer[index])) {
    throw new Error("Options and developer-console foreground transitions did not change the actual overlap pixel")
  }
} finally {
  await browser?.close()
  server.stop(true)
  await rm(temporary, { recursive: true, force: true })
}
