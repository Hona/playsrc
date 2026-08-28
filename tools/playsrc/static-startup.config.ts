import { defineConfig } from "@playwright/test"
export default defineConfig({ testDir: "./profile", testMatch: "static-startup.profile.ts", workers: 1, retries: 0,
  timeout: 165_000, globalTimeout: 175_000, reporter: "line", use: { headless: false } })
