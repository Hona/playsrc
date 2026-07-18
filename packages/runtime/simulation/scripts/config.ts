import { isAbsolute, join } from "node:path"

export async function simulationWorkflow(): Promise<{ root: string; cargo: string; crate: string }> {
  const root = join(import.meta.dir, "../../../..")
  const config = await Bun.file(join(root, "playsrc.local.json")).json()
  if (
    Object.keys(config).sort().join(",") !== "assetDir,sourceCacheDir,tf2Dir" ||
    !Object.values(config).every((value) => typeof value === "string" && isAbsolute(value))
  ) {
    throw new Error("playsrc.local.json is missing or invalid")
  }
  const cargo = join(
    config.sourceCacheDir,
    "toolchains",
    "rust",
    "cargo",
    "bin",
    process.platform === "win32" ? "cargo.exe" : "cargo",
  )
  if (!(await Bun.file(cargo).exists())) throw new Error(`Configured Rust toolchain is missing: ${cargo}`)
  return { root, cargo, crate: join(import.meta.dir, "../rust/Cargo.toml") }
}
