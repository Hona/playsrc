import path from "node:path"
import { mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises"
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

const THREADED_RUSTFLAGS = [
  "-C target-feature=+atomics,+bulk-memory",
  "-C link-arg=--shared-memory",
  "-C link-arg=--max-memory=4294967296",
  "-C link-arg=--import-memory",
  "-C link-arg=--export=__wasm_init_tls",
  "-C link-arg=--export=__tls_size",
  "-C link-arg=--export=__tls_align",
  "-C link-arg=--export=__tls_base",
].join(" ")

export async function buildThreadedTf2Wasm(
  cargo: string,
  wasmBindgen: string,
  environment: Record<string, string | undefined>,
): Promise<string> {
  const child = Bun.spawn([
    cargo,
    `+${toolchains.rust.threadedToolchain}`,
    "build",
    "-p",
    "playsrc-tf2-wasm",
    "--target",
    "wasm32-unknown-unknown",
    "--release",
    "--features",
    "threaded",
    "-Z",
    "build-std=panic_abort,std",
  ], {
    cwd: repositoryRoot,
    env: { ...process.env, ...environment, RUSTFLAGS: THREADED_RUSTFLAGS },
    stdout: "inherit",
    stderr: "inherit",
  })
  const exitCode = await child.exited
  if (exitCode !== 0) throw new Tf2WasmBuildError(`cargo build exited with code ${exitCode}`)
  const raw = path.join(repositoryRoot, "target", "wasm32-unknown-unknown", "release", "playsrc_tf2_wasm.wasm")
  const output = path.join(repositoryRoot, "games", "tf2", "browser", "src", "wasm-generated")
  await rm(output, { recursive: true, force: true })
  await mkdir(output, { recursive: true })
  const binding = Bun.spawn([
    wasmBindgen,
    "--target", "web",
    "--omit-default-module-path",
    "--out-dir", output,
    "--out-name", "tf2_wasm",
    raw,
  ], {
    cwd: repositoryRoot,
    env: { ...process.env, ...environment },
    stdout: "inherit",
    stderr: "inherit",
  })
  const bindingExitCode = await binding.exited
  if (bindingExitCode !== 0) throw new Tf2WasmBuildError(`wasm-bindgen exited with code ${bindingExitCode}`)
  const snippetRoot = path.join(output, "snippets")
  const snippets = (await readdir(snippetRoot, { withFileTypes: true })).filter((entry) => entry.isDirectory())
  if (snippets.length !== 1) throw new Tf2WasmBuildError("wasm-bindgen emitted an unexpected Rayon snippet set")
  const helper = path.join(snippetRoot, snippets[0]!.name, "src", "workerHelpers.js")
  const source = await readFile(helper, "utf8")
  const patched = source.replaceAll("\r\n", "\n").replace("import('../../..')", "import('../../../tf2_wasm.js')")
  if (patched === source) throw new Tf2WasmBuildError("wasm-bindgen Rayon worker import contract changed")
  await writeFile(helper, patched)
  return path.join(output, "tf2_wasm_bg.wasm")
}

export async function buildTf2Wasm(config: LocalConfig, threaded = true): Promise<string> {
  const executable = process.platform === "win32" ? "cargo.exe" : "cargo"
  const cargo = path.join(config.sourceCacheDir, "toolchains", "rust", "cargo", "bin", executable)
  const environment = rustEnvironment(config.sourceCacheDir)
  const wasmBindgen = path.join(config.sourceCacheDir, "toolchains", "rust", "cargo", "bin", process.platform === "win32" ? "wasm-bindgen.exe" : "wasm-bindgen")
  if (threaded) return buildThreadedTf2Wasm(cargo, wasmBindgen, environment)
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
      env: {
        ...process.env,
        ...environment,
      },
      stdout: "inherit",
      stderr: "inherit",
    },
  )
  const exitCode = await child.exited
  if (exitCode !== 0) throw new Tf2WasmBuildError(`cargo build exited with code ${exitCode}`)
  const raw = path.join(repositoryRoot, "target", "wasm32-unknown-unknown", "release", "playsrc_tf2_wasm.wasm")
  return raw
}
