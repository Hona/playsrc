import path from "node:path"
import { randomUUID } from "node:crypto"
import { mkdir, writeFile } from "node:fs/promises"
import { test } from "./application-test"
import { requireStartupNative } from "./static-startup-gate"
import { loadLocalConfig } from "../src/config"
import { startupNativeReader } from "./native-startup"

test.use({ preserveStartupMovie: true })
test("private read-only owned browser UI diagnosis", async ({ page }) => {
  const config = await loadLocalConfig(), directory = path.join(config.sourceCacheDir, "profiles/owned-browser-ui", randomUUID())
  await mkdir(directory, { recursive: true })
  console.log(`PLAYSRC_PRIVATE_OWNED_UI ${directory}`)
  const reader = await startupNativeReader(page, config.sourceCacheDir), records: unknown[] = []
  try {
    records.push(await reader.diagnoseOwnedWindow(path.join(directory, "before.png")))
    await writeFile(path.join(directory, "report.json"), JSON.stringify({ diagnosticOnly: true, recordedAt: Date.now(), records }, null, 2))
    // No input or activation. Navigation keeps the same measured page/window.
    await page.goto("/", { waitUntil: "load", timeout: 20_000 })
    records.push(await reader.diagnoseOwnedWindow(path.join(directory, "after-navigation.png")))
    await page.waitForFunction(() => {
      const root = document.querySelector<HTMLElement>("main")
      return root?.dataset.phase === "MainMenu" || ["Playing", "AwaitingGesture"].includes(root?.dataset.startupState ?? "")
    }, undefined, { timeout: 25_000 })
    records.push(await reader.diagnoseOwnedWindow(path.join(directory, "startup-ui.png")))
    await writeFile(path.join(directory, "report.json"), JSON.stringify({ diagnosticOnly: true, recordedAt: Date.now(), records }, null, 2))
    const admission = await reader.read()
    records.push({ stage: "before-startup-input", admission })
    // No input reaches a background page or a browser-owned dialog. If the
    // page itself is admitted, its normal Escape skips only the startup movie.
    if (admission.foreground) {
      requireStartupNative(admission)
      if (await page.locator("main").getAttribute("data-phase") !== "MainMenu") await page.keyboard.press("Escape")
      await page.waitForFunction(() => document.querySelector<HTMLElement>("main")?.dataset.phase === "MainMenu", undefined, { timeout: 20_000 })
      records.push(await reader.diagnoseOwnedWindow(path.join(directory, "main-menu-ui.png")))
    }
  } finally {
    await writeFile(path.join(directory, "report.json"), JSON.stringify({ diagnosticOnly: true, recordedAt: Date.now(), records, nativeRecords: reader.records }, null, 2))
    await reader.close()
  }
})
