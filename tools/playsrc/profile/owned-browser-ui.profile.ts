import path from "node:path"
import { mkdir, writeFile } from "node:fs/promises"
import { test } from "./application-test"
import { loadLocalConfig } from "../src/config"
import { startupNativeReader } from "./native-startup"

test.use({ preserveStartupMovie: true })
test("private read-only owned browser UI diagnosis", async ({ page }) => {
  const config = await loadLocalConfig(), directory = path.join(config.sourceCacheDir, "profiles/owned-browser-ui")
  await mkdir(directory, { recursive: true })
  const reader = await startupNativeReader(page, config.sourceCacheDir), records: unknown[] = []
  try {
    records.push(await reader.diagnoseOwnedWindow(path.join(directory, "before.png")))
    await writeFile(path.join(directory, "report.json"), JSON.stringify({ diagnosticOnly: true, recordedAt: Date.now(), records }, null, 2))
    // No input or activation. Navigation keeps the same measured page/window.
    await page.goto("/", { waitUntil: "load", timeout: 20_000 })
    records.push(await reader.diagnoseOwnedWindow(path.join(directory, "after-navigation.png")))
  } finally {
    await writeFile(path.join(directory, "report.json"), JSON.stringify({ diagnosticOnly: true, recordedAt: Date.now(), records, nativeRecords: reader.records }, null, 2))
    await reader.close()
  }
})
