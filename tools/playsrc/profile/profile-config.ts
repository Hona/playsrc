import path from "node:path"
import { defineConfig, type PlaywrightTestConfig } from "@playwright/test"

const MAX_PROFILE_MILLISECONDS = 175_000

type ProfileConfiguration = Readonly<{
  match: string
  target?: "jump_beef" | "pl_upward" | "ctf_2fort"
  output?: string
  channel?: "msedge"
  preciseMemory?: boolean
  grep?: RegExp
}>

export function headedProfileConfiguration(options: ProfileConfiguration): PlaywrightTestConfig {
  const port = process.env.PLAYSRC_DEV_PORT ?? "4173"
  const origin = `http://127.0.0.1:${port}`
  const root = path.resolve(import.meta.dir, "../../..")
  return defineConfig({
    testDir: path.join(root, "tools", "playsrc", "profile"),
    testMatch: options.match,
    ...(options.output ? { outputDir: options.output } : {}),
    ...(options.grep ? { grep: options.grep } : {}),
    timeout: MAX_PROFILE_MILLISECONDS,
    globalTimeout: MAX_PROFILE_MILLISECONDS,
    expect: { timeout: 30_000 },
    fullyParallel: false,
    workers: 1,
    reporter: [
      ["line"],
      [path.join(root, "tools", "playsrc", "profile", "wall-clock-reporter.ts")],
    ],
    use: {
      baseURL: origin,
      ...(options.channel ? { channel: options.channel } : {}),
      headless: false,
      viewport: { width: 1280, height: 720 },
      ...(options.preciseMemory ? { launchOptions: { args: ["--enable-precise-memory-info"] } } : {}),
    },
    webServer: {
      command: `bun tools/playsrc/src/cli.ts dev ${options.target ?? "jump_beef"}`,
      url: `${origin}/`,
      reuseExistingServer: true,
      timeout: MAX_PROFILE_MILLISECONDS,
      stdout: "pipe",
      stderr: "pipe",
    },
  })
}
