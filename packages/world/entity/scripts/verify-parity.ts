import { isAbsolute, join } from "node:path"

const root = join(import.meta.dir, "../../../..")
const config = await Bun.file(join(root, "playsrc.local.json")).json()
if (
  Object.keys(config).sort().join(",") !== "assetDir,sourceCacheDir,tf2Dir" ||
  !Object.values(config).every((value) => typeof value === "string" && isAbsolute(value))
) throw new Error("playsrc.local.json is missing or invalid")

const cargo = join(
  config.sourceCacheDir,
  "toolchains",
  "rust",
  "cargo",
  "bin",
  process.platform === "win32" ? "cargo.exe" : "cargo",
)
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

await run(["run", "-p", "playsrc-entity", "--bin", "generate-source-map-inventory"], "entity inventory generation")
const inventoryPath = join(root, "packages/world/entity/inventories/source-map-foundations.md")
const firstInventory = await Bun.file(inventoryPath).bytes()
await run(["run", "-p", "playsrc-entity", "--bin", "generate-source-map-inventory"], "repeated entity inventory generation")
const secondInventory = await Bun.file(inventoryPath).bytes()
if (!firstInventory.every((value, index) => value === secondInventory[index]) || firstInventory.length !== secondInventory.length) {
  throw new Error("repeated entity inventory generation differs")
}

for (const packageName of ["playsrc-entity", "playsrc-movement", "playsrc-collision"]) {
  await run(["test", "-p", packageName], `${packageName} tests`)
}
await run(["test", "-p", "playsrc-tf2"], "current TF2 consumer tests")
await run(["check", "-p", "playsrc-tf2-wasm"], "current TF2 WASM consumer compilation")
await run(
  ["test", "-p", "playsrc-entity", "--test", "configured_target", "--", "--ignored", "--nocapture"],
  "configured entity/contact evidence",
)
await run(
  ["test", "-p", "playsrc-collision", "--test", "configured_target", "--", "--ignored", "--nocapture"],
  "configured collision/mover evidence",
)
await run(
  ["test", "-p", "playsrc-tf2", "--test", "configured_automation", "--", "--ignored", "--nocapture"],
  "configured TF2 Entity/mover consumer evidence",
)
await run(["fmt", "--all", "--", "--check"], "workspace formatting")
for (const packageName of ["playsrc-entity", "playsrc-movement", "playsrc-collision"]) {
  await run(
    ["+stable", "clippy", "-p", packageName, "--all-targets", "--no-deps", "--", "-D", "warnings"],
    `${packageName} stable Clippy`,
  )
}
