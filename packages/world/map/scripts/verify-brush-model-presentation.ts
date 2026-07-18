import { isAbsolute, join } from "node:path"

const root = join(import.meta.dir, "../../../..")
const configPath = join(root, "playsrc.local.json")
if (!(await Bun.file(configPath).exists())) throw new Error(`required configuration is missing: ${configPath}`)
const config = await Bun.file(configPath).json()
if (
  Object.keys(config).sort().join(",") !== "assetDir,sourceCacheDir,tf2Dir" ||
  !Object.values(config).every((value) => typeof value === "string" && isAbsolute(value))
) throw new Error(`required configuration is invalid: ${configPath}`)

const cargo = join(config.sourceCacheDir, "toolchains", "rust", "cargo", "bin", process.platform === "win32" ? "cargo.exe" : "cargo")
if (!(await Bun.file(cargo).exists())) throw new Error(`configured Rust toolchain is missing: ${cargo}`)

async function run(arguments_: string[], label: string): Promise<void> {
  const child = Bun.spawn([cargo, ...arguments_], {
    cwd: root,
    stdin: "inherit",
    stdout: "inherit",
    stderr: "inherit",
  })
  const status = await child.exited
  if (status !== 0) throw new Error(`${label} failed with status ${status}`)
}

await run(["test", "-p", "playsrc-map", "-p", "playsrc-entity"], "brush-model package tests")
await run(
  ["test", "-p", "playsrc-map", "--test", "configured_brush_models", "--", "--ignored", "--nocapture"],
  "configured brush-model presentation evidence",
)
await run(["fmt", "--all", "--", "--check"], "brush-model formatting")
await run(
  ["clippy", "-p", "playsrc-map", "-p", "playsrc-entity", "--all-targets", "--", "-D", "warnings"],
  "brush-model Clippy",
)
