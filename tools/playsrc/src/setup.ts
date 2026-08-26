import { chmod, mkdir, readFile, rename, rm, stat } from "node:fs/promises"
import path from "node:path"
import toolchains from "../toolchains.json"
import { ConfigurationError, loadLocalConfig, repositoryRoot } from "./config"

const MAX_INSTALLER_BYTES = 32 * 1024 * 1024

export class SetupError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "SetupError"
  }
}

export function assertCompatibleBun(version: string, revision: string): void {
  if (version !== toolchains.bun.version) {
    throw new SetupError(`Bun ${toolchains.bun.version} is required; found ${version} (${revision})`)
  }
}

function sha256(bytes: Uint8Array): string {
  const hash = new Bun.CryptoHasher("sha256")
  hash.update(bytes)
  return hash.digest("hex")
}

async function readVerified(pathname: string, expected: string): Promise<boolean> {
  try {
    const file = await stat(pathname)
    if (!file.isFile() || file.size > MAX_INSTALLER_BYTES) return false
    return sha256(await readFile(pathname)) === expected
  } catch {
    return false
  }
}

async function run(executable: string, args: string[], env: Record<string, string>): Promise<string> {
  const child = Bun.spawn([executable, ...args], {
    cwd: repositoryRoot,
    env: { ...process.env, ...env },
    stderr: "inherit",
    stdout: "pipe",
  })
  const output = await new Response(child.stdout).text()
  const exitCode = await child.exited
  if (exitCode !== 0) throw new SetupError(`${path.basename(executable)} exited with code ${exitCode}`)
  return output.trim()
}

async function toolchainIsReady(
  rustup: string,
  env: Record<string, string>,
): Promise<boolean> {
  try {
    const wasmBindgen = path.join(path.dirname(rustup), process.platform === "win32" ? "wasm-bindgen.exe" : "wasm-bindgen")
    const [rustupVersion, rustcVersion, cargoVersion, installed, targets, threadedRustc, threadedComponents, wasmBindgenVersion] = await Promise.all([
      run(rustup, ["--version"], env),
      run(rustup, ["run", toolchains.rust.toolchain, "rustc", "--version"], env),
      run(rustup, ["run", toolchains.rust.toolchain, "cargo", "--version"], env),
      run(rustup, ["component", "list", "--toolchain", toolchains.rust.toolchain, "--installed"], env),
      run(rustup, ["target", "list", "--toolchain", toolchains.rust.toolchain, "--installed"], env),
      run(rustup, ["run", toolchains.rust.threadedToolchain, "rustc", "--version"], env),
      run(rustup, ["component", "list", "--toolchain", toolchains.rust.threadedToolchain, "--installed"], env),
      run(wasmBindgen, ["--version"], env),
    ])
    if (!rustupVersion.startsWith(`rustup ${toolchains.rust.rustupVersion} `)) return false
    if (!rustcVersion.startsWith(`rustc ${toolchains.rust.toolchain} `)
      || !cargoVersion.startsWith(`cargo ${toolchains.rust.toolchain} `)) return false
    if (!toolchains.rust.components.every((component) =>
      installed.split("\n").some((line) => line.startsWith(`${component}-`)),
    )) return false
    if (!toolchains.rust.targets.every((target) => targets.split("\n").includes(target))) return false
    if (!threadedRustc.startsWith(`rustc ${toolchains.rust.threadedRustcVersion} `)) return false
    if (!toolchains.rust.threadedComponents.every((component) =>
      threadedComponents.split("\n").some((line) => line === component || line.startsWith(`${component}-`)),
    )) return false
    return wasmBindgenVersion === `wasm-bindgen ${toolchains.wasmBindgen.version}`
  } catch {
    return false
  }
}

export async function setup(): Promise<void> {
  assertCompatibleBun(Bun.version, Bun.revision)

  const config = await loadLocalConfig(repositoryRoot, "setup")
  const hostKey = `${process.arch}-${process.platform}` as keyof typeof toolchains.rust.hosts
  const host = toolchains.rust.hosts[hostKey]
  if (!host) throw new SetupError(`unsupported host ${hostKey}`)

  const rustRoot = path.join(config.sourceCacheDir, "toolchains", "rust")
  const cargoHome = path.join(rustRoot, "cargo")
  const rustupHome = path.join(rustRoot, "rustup")
  const env = { CARGO_HOME: cargoHome, RUSTUP_HOME: rustupHome }
  const rustup = path.join(cargoHome, "bin", process.platform === "win32" ? "rustup.exe" : "rustup")
  if (await toolchainIsReady(rustup, env)) return

  const executableName = process.platform === "win32" ? "rustup-init.exe" : "rustup-init"
  const downloadDir = path.join(
    config.sourceCacheDir,
    "downloads",
    "rustup",
    toolchains.rust.rustupVersion,
    host.target,
  )
  const installer = path.join(downloadDir, executableName)
  await mkdir(downloadDir, { recursive: true })

  if (!(await readVerified(installer, host.sha256))) {
    const suffix = process.platform === "win32" ? ".exe" : ""
    const url = `https://static.rust-lang.org/rustup/archive/${toolchains.rust.rustupVersion}/${host.target}/rustup-init${suffix}`
    const response = await fetch(url)
    if (!response.ok) throw new SetupError(`rustup download failed with HTTP ${response.status}`)
    const declaredLength = Number(response.headers.get("content-length"))
    if (!Number.isSafeInteger(declaredLength) || declaredLength < 1 || declaredLength > MAX_INSTALLER_BYTES) {
      throw new SetupError("rustup download has an invalid byte length")
    }
    const bytes = new Uint8Array(await response.arrayBuffer())
    if (bytes.byteLength !== declaredLength || sha256(bytes) !== host.sha256) {
      throw new SetupError("rustup download failed SHA-256 or byte-length verification")
    }

    const temporary = `${installer}.${process.pid}.tmp`
    await rm(temporary, { force: true })
    await Bun.write(temporary, bytes)
    if (process.platform !== "win32") await chmod(temporary, 0o755)
    await rename(temporary, installer)
  } else if (process.platform !== "win32") {
    await chmod(installer, 0o755)
  }

  await run(
    installer,
    [
      "-y",
      "--no-modify-path",
      "--profile",
      "minimal",
      "--default-toolchain",
      toolchains.rust.toolchain,
      ...toolchains.rust.components.flatMap((component) => ["--component", component]),
      ...toolchains.rust.targets.flatMap((target) => ["--target", target]),
    ],
    env,
  )

  await run(
    rustup,
    [
      "toolchain",
      "install",
      toolchains.rust.threadedToolchain,
      "--profile",
      "minimal",
      ...toolchains.rust.threadedComponents.flatMap((component) => ["--component", component]),
    ],
    env,
  )
  const cargo = path.join(cargoHome, "bin", process.platform === "win32" ? "cargo.exe" : "cargo")
  const wasmBindgen = path.join(cargoHome, "bin", process.platform === "win32" ? "wasm-bindgen.exe" : "wasm-bindgen")
  let wasmBindgenReady = false
  try {
    wasmBindgenReady = await run(wasmBindgen, ["--version"], env) === `wasm-bindgen ${toolchains.wasmBindgen.version}`
  } catch {}
  if (!wasmBindgenReady) {
    await run(
      cargo,
      [
        `+${toolchains.rust.toolchain}`,
        "install",
        "wasm-bindgen-cli",
        "--version",
        toolchains.wasmBindgen.version,
        "--locked",
        "--root",
        cargoHome,
      ],
      env,
    )
  }

  if (!(await toolchainIsReady(rustup, env))) throw new SetupError("Rust toolchain verification failed")
}

export function rustEnvironment(sourceCacheDir: string): Record<string, string> {
  const rustRoot = path.join(sourceCacheDir, "toolchains", "rust")
  return {
    CARGO_HOME: path.join(rustRoot, "cargo"),
    RUSTUP_HOME: path.join(rustRoot, "rustup"),
  }
}
