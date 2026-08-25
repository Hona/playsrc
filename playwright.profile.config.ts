import { defineConfig } from "@playwright/test"

export default defineConfig({
  testDir: "tools/playsrc/profile",
  testMatch: process.env.PROFILE_SCENARIOS === "scoreboard"
    ? "scoreboard-hud.profile.ts"
    : process.env.PROFILE_PICKUPS === "1"
      ? "pickup-resupply.profile.ts"
      : process.env.PROFILE_ROUND_RULES === "1"
        ? "round-rules.profile.ts"
        : process.env.PROFILE_CTF_OBJECTIVES === "1"
          ? "ctf-objectives.profile.ts"
          : process.env.PROFILE_MATERIAL_ANIMATION === "1"
            ? "material-animation.profile.ts"
            : process.env.PROFILE_PYRO_STOCK === "1"
              ? "pyro-stock.profile.ts"
              : process.env.PROFILE_SCENARIOS === "demoman"
                ? "demoman-bottle.profile.ts"
                : process.env.PROFILE_TRACKTRAIN === "1"
                  ? "tracktrain.profile.ts"
                  : "input-latency.profile.ts",
  ...(process.env.PROFILE_SCENARIOS === "team-selection" ? { grep: /profile startup and input latency/u } : {}),
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
    command: `bun tools/playsrc/src/cli.ts dev ${process.env.PROFILE_CTF_OBJECTIVES === "1" ? "ctf_2fort" : process.env.PROFILE_COMBAT === "1" || process.env.PROFILE_ROUND_RULES === "1" || process.env.PROFILE_PICKUPS === "1" || process.env.PROFILE_MATERIAL_ANIMATION === "1" || process.env.PROFILE_SCENARIOS === "demoman" || process.env.PROFILE_SCENARIOS === "scoreboard" || process.env.PROFILE_TRACKTRAIN === "1" ? "pl_upward" : "jump_beef"}`,
    url: `http://127.0.0.1:${process.env.PLAYSRC_DEV_PORT ?? "4173"}/`,
    reuseExistingServer: false,
    timeout: 600_000,
    stdout: "pipe",
    stderr: "pipe",
  },
})
