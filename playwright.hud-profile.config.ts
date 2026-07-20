import path from "node:path"
import { defineConfig } from "@playwright/test"
import { loadLocalConfig } from "./tools/playsrc/src/config"

const local = await loadLocalConfig()

export default defineConfig({
  testDir: "tools/playsrc/profile",
  testMatch: "hud-layout.profile.ts",
  outputDir: path.join(local.sourceCacheDir, "profiles", "hud", "jump_beef", "playwright-results"),
  timeout: 600_000,
  expect: { timeout: 30_000 },
  fullyParallel: false,
  workers: 1,
  reporter: "line",
  use: {
    baseURL: "http://127.0.0.1:4173",
    headless: false,
    viewport: { width: 1280, height: 720 },
  },
  webServer: {
    command: "bun tools/playsrc/src/cli.ts dev jump_beef",
    url: "http://127.0.0.1:4173/",
    reuseExistingServer: false,
    timeout: 240_000,
    stdout: "pipe",
    stderr: "pipe",
  },
})
