import { mkdir, readFile, rename, rm, writeFile } from "node:fs/promises"
import { dirname, join, resolve } from "node:path"

type LocalConfig = {
  tf2Dir: string
  sourceCacheDir: string
  assetDir: string
}

const root = resolve(import.meta.dir, "../../../..")
const configPath = join(root, "playsrc.local.json")
const config = JSON.parse(await readFile(configPath, "utf8")) as LocalConfig
const keys = Object.keys(config).sort()
if (JSON.stringify(keys) !== JSON.stringify(["assetDir", "sourceCacheDir", "tf2Dir"])) {
  throw new Error("playsrc.local.json must contain exactly assetDir, sourceCacheDir, and tf2Dir")
}
for (const key of keys) {
  const value = config[key as keyof LocalConfig]
  if (typeof value !== "string" || value.length === 0 || !value.startsWith("/")) {
    throw new Error(`playsrc.local.json ${key} must be a non-empty absolute path`)
  }
}

const cargo = join(
  config.sourceCacheDir,
  "toolchains",
  "rust",
  "cargo",
  "bin",
  process.platform === "win32" ? "cargo.exe" : "cargo",
)
const command = [
  cargo,
  "run",
  "--locked",
  "--quiet",
  "-p",
  "playsrc-vhv",
  "--features",
  "configured-inventory",
  "--bin",
  "playsrc-vhv-configured-inventory",
]

async function generate(): Promise<Uint8Array> {
  const child = Bun.spawn(command, {
    cwd: root,
    stdout: "pipe",
    stderr: "inherit",
  })
  const output = new Uint8Array(await new Response(child.stdout).arrayBuffer())
  const status = await child.exited
  if (status !== 0) throw new Error(`configured VHV inventory exited ${status}`)
  JSON.parse(new TextDecoder().decode(output))
  return output
}

const first = await generate()
const second = await generate()
if (!Buffer.from(first).equals(Buffer.from(second))) {
  throw new Error("configured VHV inventory was not byte-identical across repeated generation")
}

const output = join(config.sourceCacheDir, "evidence", "vhv", "pl_upward-inventory.json")
const temporary = `${output}.tmp-${process.pid}`
await mkdir(dirname(output), { recursive: true })
try {
  await writeFile(temporary, first)
  await rename(temporary, output)
} finally {
  await rm(temporary, { force: true })
}
const sha256 = new Bun.CryptoHasher("sha256").update(first).digest("hex")
console.log(JSON.stringify({ output, byteLength: first.byteLength, sha256 }))
