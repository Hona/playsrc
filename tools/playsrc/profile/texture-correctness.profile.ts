import { test, expect, chromium } from "@playwright/test"
import { createServer } from "node:http"
import { readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
import { acquireHeadedProfileLock, releaseHeadedProfileLock } from "../src/profile-lock"
import { startupConsoleIdle, startupNativeReader, closeStartupNativeProbe } from "./native-startup"
import { requireStartupNative } from "./static-startup-gate"

test("current-format authored particle image correctness", async () => {
  const { sourceCacheDir } = await loadLocalConfig(), directory = path.join(sourceCacheDir, "evidence/tf2-browser-performance/texture-replacement/current-format/native-particles")
  const lockPath = path.join(sourceCacheDir, "evidence/tf2-browser-performance/chromium-profile.lock")
  const lock = await acquireHeadedProfileLock(lockPath, "texture-correctness", 2000)
  let browser: Awaited<ReturnType<typeof chromium.launch>> | undefined, reader: Awaited<ReturnType<typeof startupNativeReader>> | undefined
  const records: unknown[] = []
  const server = createServer(async (request, response) => {
    try {
      const name = request.url === "/fixture.js" ? "fixture.js" : request.url === "/inputs.json" ? "inputs.json" : null
      response.setHeader("Content-Type", name === "fixture.js" ? "text/javascript" : name ? "application/json" : "text/html")
      response.end(name ? await readFile(path.join(directory, name)) : '<!doctype html><title>Source texture correctness</title><style>body{margin:0;background:#101820}canvas{width:1280px;height:720px}</style><canvas></canvas><script type="module" src="/fixture.js"></script>')
    } catch { response.writeHead(500); response.end() }
  })
  try {
    expect(await startupConsoleIdle(sourceCacheDir)).toBeGreaterThanOrEqual(2000)
    await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve))
    // Keep the fresh tab strip away from the stationary desktop pointer that
    // produced the privately identified tab-hover card. Viewport is unchanged.
    browser = await chromium.launch({ headless: false, args: ["--window-size=1296,808", "--window-position=10,140"] })
    const page = await browser.newPage({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 })
    reader = await startupNativeReader(page, sourceCacheDir)
    // Native window-opening animation can briefly differ from CDP's final
    // bounds. Wait for exact linkage, never activate or relax the guard.
    await expect.poll(async () => {
      try { requireStartupNative(await reader!.read()); return true } catch { return false }
    }, { timeout: 3000 }).toBe(true)
    await page.goto(`http://127.0.0.1:${(server.address() as any).port}/`)
    await page.waitForFunction(() => typeof (globalThis as any).prepare === "function")
    requireStartupNative(await reader.read())
    console.log(`PLAYSRC_CORRECTNESS_BROWSER ${(await reader.read()).browserPid}`)
    records.push(await page.evaluate(() => (globalThis as any).prepare()))
    await expect.poll(async () => {
      try { requireStartupNative(await reader!.read()); return true } catch { return false }
    }, { timeout: 30_000 }).toBe(true)
    const admit = async (label: string) => {
      try { requireStartupNative(await reader!.read()) }
      catch (error) {
        await reader!.read(path.join(directory, `${label}-private-native.png`), "window").catch(() => {})
        throw error
      }
    }
    for (const phase of [0, 1]) {
      await admit(`before-${phase}`)
      const value: any = await page.evaluate(phase => (globalThis as any).capture(phase), phase)
      records.push(value)
      for (const plane of value.result.planes) { expect(plane.mismatches).toBe(0); expect(plane.identicalDrawOrder).toBe(true) }
      expect(value.separateReferenceImages).toBe(8)
      expect(value.result.planes.find((plane: any) => plane.plane === "color").actorPixels).toBeGreaterThan(100)
      await page.locator("canvas").screenshot({ path: path.join(directory, `phase-${phase}.png`) })
      await admit(`after-${phase}`)
    }
    const colorHash = (value: any) => value.result.planes.find((plane: any) => plane.plane === "color").sha256
    expect(colorHash(records[1])).not.toBe(colorHash(records[2]))
    const terminal = await page.evaluate(() => (globalThis as any).finish()); records.push(terminal)
    expect(terminal).toMatchObject({ errors: [], terminalLiveAuthored: 0 })
  } finally {
    await writeFile(path.join(directory, "report.json"), JSON.stringify({ records, native: reader?.records, performanceSample: false }, null, 2))
    await reader?.close(); closeStartupNativeProbe(); await browser?.close(); server.close()
    await releaseHeadedProfileLock(lockPath, lock.token)
  }
})
