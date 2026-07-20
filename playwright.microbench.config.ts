import { defineConfig } from "@playwright/test"

export default defineConfig({
  testDir: "tools/playsrc/profile",
  testMatch: "browser-overhead.profile.ts",
  timeout: 240_000,
  fullyParallel: false,
  workers: 1,
  reporter: "line",
  use: {
    headless: false,
    viewport: { width: 1280, height: 720 },
  },
})
