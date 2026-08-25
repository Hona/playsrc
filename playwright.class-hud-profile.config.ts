import { defineConfig } from "@playwright/test"
import hud from "./playwright.hud-profile.config"

export default defineConfig(hud, {
  testMatch: "class-hud-portrait.profile.ts",
})
