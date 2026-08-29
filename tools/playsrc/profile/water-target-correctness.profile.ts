import { test, expect, chromium } from "@playwright/test"
import { createServer } from "node:http"
import { readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "../src/config"
import { acquireHeadedProfileLock, releaseHeadedProfileLock } from "../src/profile-lock"
import { startupConsoleIdle, startupNativeReader, closeStartupNativeProbe } from "./native-startup"
import { requireStartupNative } from "./static-startup-gate"

test("water attachment admission preserves real sampled pixels and depth", async () => {
  const { sourceCacheDir } = await loadLocalConfig(), directory = path.join(sourceCacheDir, "evidence/tf2-browser-performance/texture-replacement/current-format/native-water")
  const lockPath = path.join(sourceCacheDir, "evidence/tf2-browser-performance/chromium-profile.lock"), lock = await acquireHeadedProfileLock(lockPath, "water-target-correctness", 2000)
  let browser: Awaited<ReturnType<typeof chromium.launch>> | undefined, reader: Awaited<ReturnType<typeof startupNativeReader>> | undefined
  const records: any[] = []
  const server = createServer(async (request, response) => {
    try { response.setHeader("Content-Type", request.url === "/fixture.js" ? "text/javascript" : "text/html"); response.end(request.url === "/fixture.js" ? await readFile(path.join(directory, "fixture.js")) : '<!doctype html><title>Water attachment correctness</title><style>body{margin:0}canvas{width:1280px;height:720px}</style><canvas></canvas><script type="module" src="/fixture.js"></script>') }
    catch { response.writeHead(500); response.end() }
  })
  try {
    expect(await startupConsoleIdle(sourceCacheDir)).toBeGreaterThanOrEqual(2000)
    await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve))
    browser = await chromium.launch({ headless: false, args: ["--window-size=1296,808", "--window-position=10,140"] })
    const page = await browser.newPage({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 }); reader = await startupNativeReader(page, sourceCacheDir)
    await expect.poll(async () => { try { requireStartupNative(await reader!.read()); return true } catch { return false } }, { timeout: 3000 }).toBe(true)
    await page.goto(`http://127.0.0.1:${(server.address() as any).port}/`); await page.waitForFunction(() => typeof (globalThis as any).prepare === "function")
    requireStartupNative(await reader.read()); await page.evaluate(() => (globalThis as any).prepare())
    for (const mode of ["reference", "candidate"] as const) {
      requireStartupNative(await reader.read())
      const record = await page.evaluate(mode => (globalThis as any).capture(mode), mode); records.push(record)
      expect(record.errors).toEqual([]); expect(record.terminalLive).toBe(0)
      expect(record.primaryAllocations).toBe(mode === "reference" ? 6 : 4)
      expect(record.depthAllocations).toBe(4)
      await page.locator("canvas").screenshot({ path: path.join(directory, `${mode}.png`) }); requireStartupNative(await reader.read())
    }
    expect(records[1].phases).toEqual(records[0].phases)
    const terminal = await page.evaluate(() => (globalThis as any).finish()); records.push(terminal)
    expect(terminal).toMatchObject({ errors: [], terminalLiveWater: 0 })
  } finally {
    await writeFile(path.join(directory, "report.json"), JSON.stringify({ records, native: reader?.records, performanceSample: false }, null, 2))
    await reader?.close(); closeStartupNativeProbe(); await browser?.close(); server.close(); await releaseHeadedProfileLock(lockPath, lock.token)
  }
})
