import { isAbsolute, join } from "node:path"

const root = join(import.meta.dir, "../../../..")
const config = await Bun.file(join(root, "playsrc.local.json")).json()
if (
  Object.keys(config).sort().join(",") !== "assetDir,sourceCacheDir,tf2Dir" ||
  !Object.values(config).every((value) => typeof value === "string" && isAbsolute(value))
) throw new Error("playsrc.local.json is missing or invalid")

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

await run(["test", "-p", "playsrc-collision"], "collision package tests")
await run(
  ["test", "-p", "playsrc-collision", "--test", "configured_target", "--", "--ignored", "--nocapture"],
  "configured collision evidence",
)
await run(
  ["check", "-p", "playsrc-collision", "--target", "wasm32-unknown-unknown"],
  "collision WASM compilation",
)
await run(["fmt", "--all", "--", "--check"], "workspace formatting")
await run(
  ["+stable", "clippy", "-p", "playsrc-collision", "--all-targets", "--no-deps", "--", "-D", "warnings"],
  "collision Clippy",
)
