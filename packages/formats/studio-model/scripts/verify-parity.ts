import { isAbsolute, join, resolve } from "node:path"

const packageDirectory = resolve(import.meta.dir, "..")
const root = resolve(packageDirectory, "../../..")
const config = await Bun.file(join(root, "playsrc.local.json")).json() as Record<string, unknown>
const keys = Object.keys(config).sort()
if (JSON.stringify(keys) !== JSON.stringify(["assetDir", "sourceCacheDir", "tf2Dir"])) {
  throw new Error("playsrc.local.json must contain exactly assetDir, sourceCacheDir, and tf2Dir")
}
for (const key of keys) {
  if (typeof config[key] !== "string" || !isAbsolute(config[key])) throw new Error(`${key} must be an absolute path`)
}

async function run(command: readonly string[], environment = process.env): Promise<void> {
  const child = Bun.spawn(command, { cwd: root, env: environment, stdout: "inherit", stderr: "inherit" })
  const exit = await child.exited
  if (exit !== 0) throw new Error(`${command.join(" ")} exited ${exit}`)
}

await run(["cargo", "run", "--locked", "--release", "-p", "playsrc-source-bundle", "--", "jump_beef"])
await run([
  "cargo",
  "run",
  "--locked",
  "--release",
  "--manifest-path",
  join(packageDirectory, "scripts", "evidence-runner", "Cargo.toml"),
])
