import path from "node:path"
import type { LocalConfig } from "./config"
import { repositoryRoot } from "./config"
import { rustEnvironment } from "./setup"
import toolchains from "../toolchains.json"

export async function buildSourceBundle(config: LocalConfig, target: string): Promise<string> {
  const executable = process.platform === "win32" ? "cargo.exe" : "cargo"
  const cargo = path.join(config.sourceCacheDir, "toolchains", "rust", "cargo", "bin", executable)
  const child = Bun.spawn([
    cargo,
    `+${toolchains.rust.toolchain}`,
    "run",
    "-p",
    "playsrc-source-bundle",
    "--",
    target,
  ], {
    cwd: repositoryRoot,
    env: { ...process.env, ...rustEnvironment(config.sourceCacheDir) },
    stdout: "pipe",
    stderr: "inherit",
  })
  const output = await new Response(child.stdout).text()
  if (await child.exited !== 0) throw new Error("source bundle build failed")
  const report = JSON.parse(output) as { target?: unknown; bytes?: unknown; sha256?: unknown }
  if (
    report.target !== target
    || !Number.isSafeInteger(report.bytes)
    || (report.bytes as number) < 12
    || typeof report.sha256 !== "string"
    || !/^[0-9a-f]{64}$/.test(report.sha256)
  ) {
    throw new Error("source bundle report is malformed")
  }
  return path.join(config.sourceCacheDir, "browser-bundles", `${target}.psdb`)
}
