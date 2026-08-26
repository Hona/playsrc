import path from "node:path"
import { defineConfig } from "@playwright/test"
import { loadLocalConfig } from "./tools/playsrc/src/config"

const local = await loadLocalConfig()
const port = process.env.PLAYSRC_DEV_PORT ?? "4173"

export default defineConfig({
  testDir: "tools/playsrc/profile",
  testMatch: "three-map-load.profile.ts",
  outputDir: path.join(local.sourceCacheDir, "profiles", "three-map-load", "playwright-results"),
  timeout: 1_800_000,
  expect: { timeout: 30_000 },
  fullyParallel: false,
  workers: 1,
  reporter: "line",
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    headless: false,
    viewport: { width: 1280, height: 720 },
  },
  webServer: {
    command: "bun tools/playsrc/src/cli.ts dev jump_beef",
    url: `http://127.0.0.1:${port}/`,
    reuseExistingServer: process.env.PLAYSRC_REUSE_DEV_SERVER === "1",
    timeout: 600_000,
    stdout: "pipe",
    stderr: "pipe",
  },
})
