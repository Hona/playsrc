import path from "node:path"
import { mkdir, writeFile } from "node:fs/promises"
import { randomUUID } from "node:crypto"
import { chromium } from "@playwright/test"
import { loadLocalConfig } from "../../../../tools/playsrc/src/config"
import { acquireHeadedProfileLock, releaseHeadedProfileLock } from "../../../../tools/playsrc/src/profile-lock"
import { macPageAdmission, requireMacPageAdmission } from "../../../../tools/playsrc/profile/macos-page-admission"
import { startupConsoleIdle } from "../../../../tools/playsrc/profile/native-startup"

const root = path.resolve(import.meta.dir, "../../../..")
const config = await loadLocalConfig(root)
const basePlanes = process.env.PLAYSRC_MODEL_BASE_PLANES === "1"
const output = path.join(config.sourceCacheDir, "evidence/tf2-browser-performance/dynamic-model-graphs", basePlanes ? "base-planes" : "eyes", `${Date.now()}-${randomUUID()}`)
await mkdir(output, { recursive: true })
console.error(`[evidence] model-graph=${output}`)
const build = await Bun.build({ entrypoints: [path.join(root, "packages/presentation/rendering/tests/fixtures/model-graph-probe.ts")], target: "browser", format: "esm" })
if (!build.success) throw Error(build.logs.map(String).join("\n"))
const source = await build.outputs[0]!.text()
const server = Bun.serve({ port: 4190, hostname: "127.0.0.1", fetch(request) {
  return new URL(request.url).pathname === "/probe.js" ? new Response(source, { headers: { "Content-Type": "text/javascript" } })
    : new Response(`<!doctype html><title>Dynamic model graphs pixel/depth equivalence</title><style>body{margin:0;background:#111;color:white}</style><h3>Dedicated vs shared graph: identical multi-actor pixels and depth</h3><script type="module">import {createModelGraphProbe} from '/probe.js';window.probe=await createModelGraphProbe(false,${basePlanes});</script>`, { headers: { "Content-Type": "text/html" } })
} })
const lockPath = path.join(config.sourceCacheDir, "evidence/tf2-browser-performance/chromium-profile.lock")
const started = Date.now()
let lock: Awaited<ReturnType<typeof acquireHeadedProfileLock>> | undefined
let browser: Awaited<ReturnType<typeof chromium.launch>> | undefined
let native: Awaited<ReturnType<typeof macPageAdmission>>
const deadline = setTimeout(() => { void browser?.close() }, 100_000)
try {
  lock = await acquireHeadedProfileLock(lockPath, "dynamic-model-graph-pixels", 60_000)
  if (await startupConsoleIdle(config.sourceCacheDir) < 2000) throw Error("Native model graph launch requires genuine input idle")
  browser = await chromium.launch({ channel: "msedge", headless: false })
  const page = await browser.newPage({ viewport: { width: 640, height: 540 }, deviceScaleFactor: 1 })
  const errors: string[] = []
  page.on("pageerror", error => errors.push(error.message))
  await page.goto("http://127.0.0.1:4190/")
  await page.waitForFunction(() => (window as any).probe, undefined, { timeout: 20_000 })
  native = await macPageAdmission(page, config.sourceCacheDir)
  if (!native) throw Error("Model graph pixel verification requires native page/window admission")
  await page.waitForTimeout(2100)
  if (await startupConsoleIdle(config.sourceCacheDir) < 2000) throw Error("Native model graph sample is not input-idle")
  const admissionBefore = await native.read(path.join(output, "before.desktop.png"))
  await writeFile(path.join(output, "native-admission.json"), JSON.stringify([admissionBefore]))
  requireMacPageAdmission(admissionBefore)
  const results = []
  for (let phase = 0; phase < 4; phase++) {
    const { beforePixels, afterPixels, ...result } = await page.evaluate(phase => (window as any).probe.compare(phase), phase)
    for (const [label, data] of [["before", beforePixels], ["after", afterPixels]]) await writeFile(path.join(output, `phase-${phase}-${label}.png`), Buffer.from(data.split(",")[1], "base64"))
    results.push(result)
  }
  await page.screenshot({ path: path.join(output, "visible.png") })
  const admissionAfter = await native.read(path.join(output, "after.desktop.png"))
  await writeFile(path.join(output, "native-admission.json"), JSON.stringify([admissionBefore, admissionAfter]))
  requireMacPageAdmission(admissionAfter)
  await page.evaluate(() => (window as any).probe.dispose())
  if (errors.length) throw Error(errors.join("\n"))
  const report = { headed: true, basePlanes, results, errors, milliseconds: Date.now() - started }
  await writeFile(path.join(output, "result.json"), JSON.stringify(report, null, 2))
  console.log(JSON.stringify(report))
  if (results.some(result => result.colorMismatches || result.depthMismatches)) throw Error("Multi-actor graph pixel/depth comparison failed; raw captures retained")
} finally {
  clearTimeout(deadline)
  await native?.close()
  await browser?.close()
  server.stop()
  if (lock) await releaseHeadedProfileLock(lockPath, lock.token)
}
