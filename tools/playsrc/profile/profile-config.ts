import path from "node:path"
import { fileURLToPath } from "node:url"
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
  const configuredOrigin = process.env.PLAYSRC_PROFILE_ORIGIN
  const origin = configuredOrigin ?? `http://127.0.0.1:${port}`
  const width = Number(process.env.PLAYSRC_PROFILE_VIEWPORT_WIDTH ?? 1280)
  const height = Number(process.env.PLAYSRC_PROFILE_VIEWPORT_HEIGHT ?? 720)
  const deviceScaleFactor = Number(process.env.PLAYSRC_PROFILE_DEVICE_SCALE_FACTOR ?? 1)
  if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height) || width < 320 || height < 240) throw new Error("Headed profile viewport dimensions are invalid")
  if (![1, 1.25, 1.5, 2].includes(deviceScaleFactor)) throw new Error("Headed profile device scale factor must be 1, 1.25, 1.5, or 2")
  const root = fileURLToPath(new URL("../../../", import.meta.url))
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
      ...(process.env.PLAYSRC_PROFILE_BROWSER_CHANNEL || options.channel ? { channel: process.env.PLAYSRC_PROFILE_BROWSER_CHANNEL ?? options.channel } : {}),
      headless: false,
      viewport: { width, height },
      deviceScaleFactor,
      ...(options.preciseMemory ? { launchOptions: { args: ["--enable-precise-memory-info"] } } : {}),
    },
    ...(configuredOrigin ? {} : { webServer: {
      command: `bun tools/playsrc/src/cli.ts dev ${options.target ?? "jump_beef"}`,
      url: `${origin}/`,
      reuseExistingServer: true,
      timeout: MAX_PROFILE_MILLISECONDS,
      stdout: "pipe",
      stderr: "pipe",
    } }),
  })
}
