import { spawn } from "node:child_process"
import { mkdtemp, rm } from "node:fs/promises"
import { createServer } from "node:net"
import path from "node:path"
import type { Browser } from "@playwright/test"
import { test as applicationTest, expect } from "./application-test"
import { loadLocalConfig } from "../src/config"

// Playwright's ordinary browser contexts force focus/visibility with CDP. A
// native default context with noDefaults observes actual tab/window lifecycle.
export const test = applicationTest.extend<{}, { browser: Browser }>({
  browser: [async ({ playwright }, use) => {
    const executable = process.env.PLAYSRC_PROFILE_NATIVE_EDGE
    if (!executable || !path.isAbsolute(executable)) throw new Error("PLAYSRC_PROFILE_NATIVE_EDGE must name the installed visible Edge executable")
    const config = await loadLocalConfig()
    const directory = await mkdtemp(path.join(config.sourceCacheDir, "profiles", "native-edge-"))
    const reservation = createServer()
    await new Promise<void>((resolve) => reservation.listen(0, "127.0.0.1", resolve))
    const address = reservation.address()
    if (!address || typeof address === "string") throw new Error("Native Edge debug port is unavailable")
    await new Promise<void>((resolve) => reservation.close(() => resolve()))
    const endpoint = `http://127.0.0.1:${address.port}`
    const child = spawn(executable, [`--remote-debugging-port=${address.port}`, `--user-data-dir=${directory}`, "--no-first-run", "--no-default-browser-check", "--enable-automation", "about:blank"], { stdio: ["ignore", "ignore", "pipe"] })
    let diagnostics = ""
    child.stderr!.on("data", (bytes) => { diagnostics = (diagnostics + String(bytes)).slice(-8_192) })
    const terminate = () => { child.kill("SIGTERM") }
    process.once("SIGTERM", terminate)
    let browser: Browser | undefined
    try {
      const deadline = Date.now() + 15_000
      while (true) {
        if (await fetch(`${endpoint}/json/version`).then((response) => response.ok).catch(() => false)) break
        if (Date.now() >= deadline || child.exitCode !== null) throw new Error("Visible native Edge did not become ready")
        await new Promise((resolve) => setTimeout(resolve, 100))
      }
      browser = await playwright.chromium.connectOverCDP(endpoint, { noDefaults: true, timeout: 10_000 }).catch((error) => { throw new Error(`${error}\n${diagnostics}`) })
      await use(browser)
    } finally {
      await browser?.close()
      terminate()
      process.removeListener("SIGTERM", terminate)
      await rm(directory, { recursive: true, force: true, maxRetries: 3 }).catch(() => {})
    }
  }, { scope: "worker" }],
  context: async ({ browser }, use) => { await use(browser.contexts()[0]!) },
  page: async ({ context }, use) => {
    const page = await context.newPage()
    await page.setViewportSize({ width: 1280, height: 720 })
    try { await use(page) }
    finally { await page.close() }
  },
})

export { expect }
