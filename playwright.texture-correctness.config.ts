import { defineConfig } from "@playwright/test"
import path from "node:path"
import { loadLocalConfig } from "./tools/playsrc/src/config"
const { sourceCacheDir } = await loadLocalConfig()
export default defineConfig({ testDir: "./tools/playsrc/profile", testMatch: "texture-correctness.profile.ts", workers: 1, fullyParallel: false, timeout: 170_000,
  reporter: "line", outputDir: path.join(sourceCacheDir, "profiles/texture-correctness-results"), use: { headless: false } })
