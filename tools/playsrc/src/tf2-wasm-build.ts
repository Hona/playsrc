import path from "node:path"
import toolchains from "../toolchains.json"
import type { LocalConfig } from "./config"
import { repositoryRoot } from "./config"
import { rustEnvironment } from "./setup"

export class Tf2WasmBuildError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "Tf2WasmBuildError"
  }
}

export async function buildTf2Wasm(config: LocalConfig): Promise<string> {
  const executable = process.platform === "win32" ? "cargo.exe" : "cargo"
  const cargo = path.join(config.sourceCacheDir, "toolchains", "rust", "cargo", "bin", executable)
  const child = Bun.spawn(
    [
      cargo,
      `+${toolchains.rust.toolchain}`,
      "build",
      "-p",
      "playsrc-tf2-wasm",
      "--target",
      "wasm32-unknown-unknown",
      "--release",
    ],
    {
      cwd: repositoryRoot,
      env: { ...process.env, ...rustEnvironment(config.sourceCacheDir) },
      stdout: "inherit",
      stderr: "inherit",
    },
  )
  const exitCode = await child.exited
  if (exitCode !== 0) throw new Tf2WasmBuildError(`cargo build exited with code ${exitCode}`)
  return path.join(repositoryRoot, "target", "wasm32-unknown-unknown", "release", "playsrc_tf2_wasm.wasm")
}
