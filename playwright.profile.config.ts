import { defineConfig } from "@playwright/test"

export default defineConfig({
  testDir: "tools/playsrc/profile",
  testMatch: process.env.PROFILE_MATERIAL_ANIMATION === "1"
    ? "material-animation.profile.ts"
    : "input-latency.profile.ts",
  timeout: 600_000,
  expect: { timeout: 30_000 },
  fullyParallel: false,
  workers: 1,
  reporter: "line",
  use: {
    baseURL: `http://127.0.0.1:${process.env.PLAYSRC_DEV_PORT ?? "4173"}`,
    headless: false,
    viewport: { width: 1280, height: 720 },
  },
  webServer: {
    command: `bun tools/playsrc/src/cli.ts dev ${process.env.PROFILE_MATERIAL_ANIMATION === "1" ? "pl_upward" : "jump_beef"}`,
    url: `http://127.0.0.1:${process.env.PLAYSRC_DEV_PORT ?? "4173"}/`,
    reuseExistingServer: false,
    timeout: 600_000,
    stdout: "pipe",
    stderr: "pipe",
  },
})
