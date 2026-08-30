import path from "node:path"
import { createHash } from "node:crypto"
import { readFile, writeFile } from "node:fs/promises"
import { test, expect } from "./application-test"
import { profileArtifact } from "./profile-artifacts"
import { startupNativeReader } from "./native-startup"
import { requireStartupNative } from "./static-startup-gate"
import { loadLocalConfig } from "../src/config"

// A short real headed lifecycle check, not a gameplay/performance benchmark.
test("prepared runner hands off a visible browser and releases before extraction", async ({ page }, testInfo) => {
  const directory = process.env.PLAYSRC_PROFILE_RUN_DIRECTORY!
  await page.setContent('<!doctype html><title>playsrc runner handoff</title><style>html,body{margin:0;background:#183c60}canvas{display:block}</style><canvas width="640" height="360"></canvas><script>window.phase=0;const c=document.querySelector("canvas"),g=c.getContext("2d");function draw(){g.fillStyle=window.phase?"#ffb347":"#41bf8d";g.fillRect(0,0,640,360);window.drawn=window.phase;requestAnimationFrame(draw)}draw()</script>')
  const reader = await startupNativeReader(page, (await loadLocalConfig()).sourceCacheDir)
  const records: unknown[] = []
  let before: Buffer, after: Buffer
  try {
    requireStartupNative(await reader.read())
    records.push(await reader.diagnoseOwnedWindow(path.join(directory, "handoff-before.native.png")))
    before = await page.locator("canvas").screenshot()
    await page.evaluate(() => { (window as any).phase = 1 })
    await page.waitForFunction(() => (window as any).drawn === 1)
    requireStartupNative(await reader.read())
    records.push(await reader.diagnoseOwnedWindow(path.join(directory, "handoff-after.native.png")))
    after = await page.locator("canvas").screenshot()
  } finally { await reader.close() }
  await profileArtifact(async () => {
    const owner = JSON.parse(await readFile(process.env.PLAYSRC_LOCAL_JOB_OWNER!, "utf8"))
    const released = JSON.parse(await readFile(path.join(owner.run, "desktop-released.json"), "utf8"))
    const extractionStartedAt = Date.now()
    expect(released.desktopReleasedAt).toBeGreaterThan(released.desktopStartedAt)
    expect(extractionStartedAt).toBeGreaterThanOrEqual(released.desktopReleasedAt)
    // Deliberately slow background retention proves that desktop release is not
    // tied to command exit. This is not included in any gameplay sample metric.
    await new Promise(resolve => setTimeout(resolve, 1500))
    const hash = (bytes: Buffer) => createHash("sha256").update(bytes).digest("hex")
    expect(hash(before)).not.toBe(hash(after))
    await writeFile(path.join(directory, "handoff.json"), JSON.stringify({ performanceSample: false, extractionStartedAt, extractionFinishedAt: Date.now(), released, records, before: hash(before), after: hash(after) }, null, 2))
    await writeFile(path.join(directory, "handoff-before.png"), before)
    await writeFile(path.join(directory, "handoff-after.png"), after)
    await testInfo.attach("post-desktop-artifacts", { body: JSON.stringify({ extractionStartedAt, desktopReleasedAt: released.desktopReleasedAt }), contentType: "application/json" })
  })
})
