import path from "node:path"
import os from "node:os"
import { copyFile, mkdir, readFile, readdir, rename, rm, stat, writeFile } from "node:fs/promises"
import { constants } from "node:fs"
import toolchains from "../toolchains.json"
import type { LocalConfig } from "./config"
import { repositoryRoot } from "./config"
import { rustEnvironment } from "./setup"
import { buildCacheDirectory, rustBuildIdentity } from "./build-identity"
import { fileFingerprint } from "./file-fingerprint"

export class Tf2WasmBuildError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "Tf2WasmBuildError"
  }
}

export function threadedWasmRustFlags(root: string, cargoHome: string, sysroot: string): string[] {
  return [
    "-Ctarget-feature=+atomics,+bulk-memory",
    "-Clink-arg=--shared-memory",
    "-Clink-arg=--max-memory=4294967296",
    "-Clink-arg=--import-memory",
    "-Clink-arg=--export=__wasm_init_tls",
    "-Clink-arg=--export=__tls_size",
    "-Clink-arg=--export=__tls_align",
    "-Clink-arg=--export=__tls_base",
    ...wasmSourcePathFlags(root, cargoHome, sysroot),
  ]
}

function wasmSourcePathFlags(root: string, cargoHome: string, sysroot: string): string[] {
  return [`--remap-path-prefix=${root}=/playsrc`, `--remap-path-prefix=${cargoHome}=/cargo`, `--remap-path-prefix=${sysroot}=/rust`]
}

export function audioWasmRustFlags(root: string, cargoHome: string, sysroot: string): string[] {
  // Audio has its own unshared memory and measured decoder SIMD requirement.
  // Gameplay keeps its independently qualified target features.
  return ["-Ctarget-feature=+simd128", ...wasmSourcePathFlags(root, cargoHome, sysroot)]
}

type WasmBuildManifest = Readonly<{
  schema: "playsrc-threaded-wasm-build-v2"
  identity: string
  files: readonly Readonly<{ name: string; bytes: number; sha256: string }>[]
}>

async function generatedFiles(directory: string, prefix = ""): Promise<string[]> {
  const entries = await readdir(path.join(directory, prefix), { withFileTypes: true })
  const nested = await Promise.all(entries.map(async (entry) => {
    const relative = path.join(prefix, entry.name)
    if (entry.isDirectory()) return generatedFiles(directory, relative)
    if (!entry.isFile() || entry.name === ".playsrc-build.json") return []
    return [relative]
  }))
  return nested.flat().sort()
}

async function copyBuildFile(source: string, destination: string): Promise<void> {
  await mkdir(path.dirname(destination), { recursive: true })
  // A worktree must not be able to mutate another worktree's retained build.
  await copyFile(source, destination, constants.COPYFILE_FICLONE)
}

export async function readWasmManifest(directory: string, identity: string): Promise<WasmBuildManifest | null> {
  try {
    const parsed = JSON.parse(await readFile(path.join(directory, ".playsrc-build.json"), "utf8")) as WasmBuildManifest
    if (parsed.schema !== "playsrc-threaded-wasm-build-v2" || parsed.identity !== identity || !Array.isArray(parsed.files)
      || parsed.files.length === 0 || !parsed.files.some((file) => file.name === "tf2_wasm_bg.wasm")
      || !parsed.files.some((file) => file.name === "audio_wasm.wasm")) return null
    for (const file of parsed.files) {
      if (typeof file.name !== "string" || path.isAbsolute(file.name) || file.name.split(/[\\/]/u).includes("..")
        || !Number.isSafeInteger(file.bytes) || file.bytes < 0) return null
      const metadata = await stat(path.join(directory, file.name))
      if (!metadata.isFile() || metadata.size !== file.bytes || file.sha256 !== await fileFingerprint(path.join(directory, file.name))) return null
    }
    return parsed
  } catch (error) {
    if (["ENOENT", "ENOTDIR"].includes((error as NodeJS.ErrnoException).code ?? "") || error instanceof SyntaxError) return null
    throw error
  }
}

async function retainThreadedBuild(config: LocalConfig, identity: string, output: string): Promise<void> {
  const directory = path.join(buildCacheDirectory(config.sourceCacheDir, identity), "threaded-wasm")
  if (await readWasmManifest(directory, identity)) return
  const temporary = `${directory}.${process.pid}.${crypto.randomUUID()}.tmp`
  await mkdir(temporary, { recursive: true })
  try {
    const names = await generatedFiles(output)
    const files = await Promise.all(names.map(async (name) => {
      await copyBuildFile(path.join(output, name), path.join(temporary, name))
      return Object.freeze({ name, bytes: (await stat(path.join(output, name))).size, sha256: await fileFingerprint(path.join(temporary, name)) })
    }))
    const manifest: WasmBuildManifest = Object.freeze({ schema: "playsrc-threaded-wasm-build-v2", identity, files })
    await writeFile(path.join(temporary, ".playsrc-build.json"), `${JSON.stringify(manifest)}\n`)
    await mkdir(path.dirname(directory), { recursive: true })
    try {
      await rename(temporary, directory)
    } catch (error) {
      if (!["EEXIST", "ENOTEMPTY"].includes((error as NodeJS.ErrnoException).code ?? "")) throw error
      if (!await readWasmManifest(directory, identity)) throw new Error("shared threaded WASM build snapshot is malformed")
    }
    await writeFile(path.join(output, ".playsrc-build.json"), `${JSON.stringify(manifest)}\n`)
  } finally {
    await rm(temporary, { recursive: true, force: true })
  }
}

export async function restoreThreadedBuild(config: Pick<LocalConfig, "sourceCacheDir">, identity: string, root = repositoryRoot): Promise<string | null> {
  const output = path.join(root, "games", "tf2", "browser", "src", "wasm-generated")
  if (await readWasmManifest(output, identity)) return path.join(output, "tf2_wasm_bg.wasm")
  const directory = path.join(buildCacheDirectory(config.sourceCacheDir, identity), "threaded-wasm")
  const manifest = await readWasmManifest(directory, identity)
  if (!manifest) return null
  const temporary = `${output}.${process.pid}.${crypto.randomUUID()}.tmp`
  await mkdir(temporary, { recursive: true })
  try {
    await Promise.all(manifest.files.map((file) => copyBuildFile(path.join(directory, file.name), path.join(temporary, file.name))))
    await writeFile(path.join(temporary, ".playsrc-build.json"), `${JSON.stringify(manifest)}\n`)
    await rm(output, { recursive: true, force: true })
    await rename(temporary, output)
  } finally {
    await rm(temporary, { recursive: true, force: true })
  }
  return path.join(output, "tf2_wasm_bg.wasm")
}

export async function resolveCargoExecutable(cargo: string, environment: Record<string, string | undefined>): Promise<string> {
  // Configured toolchains already name an exact file. Do not ask a PATH/PATHEXT
  // search to rediscover it (Windows SSH and interactive shells differ there).
  const executable = path.isAbsolute(cargo) ? cargo : Bun.which(cargo, { PATH: environment.PATH ?? environment.Path })
  if (!executable || !(await stat(executable).catch(() => null))?.isFile()) throw new Tf2WasmBuildError("The pinned Cargo executable is unavailable")
  return executable
}

export async function buildThreadedTf2Wasm(
  cargo: string,
  wasmBindgen: string,
  environment: Record<string, string | undefined>,
): Promise<string> {
  const buildEnvironment = { ...process.env, ...environment }
  const targetRoot=path.resolve(repositoryRoot,buildEnvironment.CARGO_TARGET_DIR ?? "target")
  const executable = await resolveCargoExecutable(cargo, buildEnvironment)
  const rustc = path.join(path.dirname(executable), process.platform === "win32" ? "rustc.exe" : "rustc")
  const compiler = Bun.spawn([rustc, `+${toolchains.rust.threadedToolchain}`, "--print", "sysroot"], {
    cwd: repositoryRoot, env: buildEnvironment, stdout: "pipe", stderr: "inherit",
  })
  const sysroot = (await new Response(compiler.stdout).text()).trim()
  if (await compiler.exited !== 0 || !path.isAbsolute(sysroot)) throw new Tf2WasmBuildError("The pinned Rust sysroot is unavailable")
  const cargoHome = path.resolve(buildEnvironment.CARGO_HOME ?? path.join(os.homedir(), ".cargo"))
  // Source-location constants are runtime data too; keep host paths out of the artifact.
  const flags = threadedWasmRustFlags(repositoryRoot, cargoHome, sysroot)
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
    env: { ...buildEnvironment, RUSTFLAGS: undefined, CARGO_ENCODED_RUSTFLAGS: flags.join("\x1f"), CARGO_BUILD_JOBS: process.env.PLAYSRC_PROFILE_OWNER_TOKEN ? "2" : process.env.CARGO_BUILD_JOBS },
    stdout: "inherit",
    stderr: "inherit",
  })
  const exitCode = await child.exited
  if (exitCode !== 0) throw new Tf2WasmBuildError(`cargo build exited with code ${exitCode}`)
  const raw = path.join(targetRoot, "wasm32-unknown-unknown", "release", "playsrc_tf2_wasm.wasm")
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
  const audioTarget = path.join(targetRoot, "audio-client")
  const audio = Bun.spawn([
    cargo, `+${toolchains.rust.threadedToolchain}`, "build", "-p", "playsrc-audio-wasm",
    "--target", "wasm32-unknown-unknown", "--target-dir", audioTarget, "--release", "-Z", "build-std=panic_abort,std",
  ], {
    cwd: repositoryRoot,
    env: { ...buildEnvironment, RUSTFLAGS: undefined, CARGO_ENCODED_RUSTFLAGS: audioWasmRustFlags(repositoryRoot, cargoHome, sysroot).join("\x1f"), CARGO_BUILD_JOBS: process.env.PLAYSRC_PROFILE_OWNER_TOKEN ? "2" : process.env.CARGO_BUILD_JOBS },
    stdout: "inherit", stderr: "inherit",
  })
  const audioExit = await audio.exited
  if (audioExit !== 0) throw new Tf2WasmBuildError(`audio cargo build exited with code ${audioExit}`)
  await copyFile(path.join(audioTarget, "wasm32-unknown-unknown", "release", "playsrc_audio_wasm.wasm"), path.join(output, "audio_wasm.wasm"))
  return path.join(output, "tf2_wasm_bg.wasm")
}

export async function buildTf2Wasm(config: LocalConfig, threaded = true): Promise<string> {
  const executable = process.platform === "win32" ? "cargo.exe" : "cargo"
  const cargo = path.join(config.sourceCacheDir, "toolchains", "rust", "cargo", "bin", executable)
  const environment = rustEnvironment(config.sourceCacheDir)
  const wasmBindgen = path.join(config.sourceCacheDir, "toolchains", "rust", "cargo", "bin", process.platform === "win32" ? "wasm-bindgen.exe" : "wasm-bindgen")
  if (threaded) {
    const identity = await rustBuildIdentity()
    const cached = await restoreThreadedBuild(config, identity)
    if (cached) return cached
    const output = await buildThreadedTf2Wasm(cargo, wasmBindgen, environment)
    await retainThreadedBuild(config, identity, path.dirname(output))
    return output
  }
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
  const raw = path.join(path.resolve(repositoryRoot,process.env.CARGO_TARGET_DIR ?? "target"), "wasm32-unknown-unknown", "release", "playsrc_tf2_wasm.wasm")
  return raw
}
