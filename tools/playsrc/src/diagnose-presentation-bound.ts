import { mkdir, readFile, rename, rm, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig, repositoryRoot } from "./config"
import { rustEnvironment } from "./setup"
import toolchains from "../toolchains.json"

const target = process.argv[2]
if (!target || !/^[a-z0-9_]+$/.test(target) || process.argv.length !== 3) {
  throw new Error("usage: bun run diagnose:presentation-bound <target>")
}
const local = await loadLocalConfig()
const cargo = path.join(
  local.sourceCacheDir,
  "toolchains",
  "rust",
  "cargo",
  "bin",
  process.platform === "win32" ? "cargo.exe" : "cargo",
)
const environment = { ...process.env, ...rustEnvironment(local.sourceCacheDir) }
const build = Bun.spawn([
  cargo,
  `+${toolchains.rust.toolchain}`,
  "build",
  "--locked",
  "--profile",
  "source-bundle",
  "-p",
  "playsrc-source-bundle",
], { cwd: repositoryRoot, env: environment, stdout: "ignore", stderr: "inherit" })
if (await build.exited !== 0) throw new Error("presentation-bound diagnostic build failed")
const executable = path.join(
  repositoryRoot,
  "target",
  "source-bundle",
  process.platform === "win32" ? "playsrc-source-bundle.exe" : "playsrc-source-bundle",
)

async function sample(): Promise<Readonly<Record<string, unknown>>> {
  const child = Bun.spawn([executable, target, "--diagnose-presentation-bound"], {
    cwd: repositoryRoot,
    env: environment,
    stdout: "pipe",
    stderr: "inherit",
  })
  const output = await new Response(child.stdout).text()
  if (await child.exited !== 0) throw new Error("presentation-bound diagnostic failed")
  const value = JSON.parse(output) as Record<string, unknown>
  if (value.schema !== "playsrc-static-prop-producer-diagnostic-v1" || value.target !== target) {
    throw new Error("presentation-bound diagnostic report is malformed")
  }
  return Object.freeze(value)
}

const first = await sample()
const second = await sample()
if (JSON.stringify(first) !== JSON.stringify(second)) {
  throw new Error("presentation-bound diagnostic output is not deterministic")
}
const outputDirectory = path.join(local.sourceCacheDir, "evidence", "presentation-bound", target)
const outputPath = path.join(outputDirectory, "report.json")
const temporary = `${outputPath}.tmp-${process.pid}`
const report = `${JSON.stringify(first, null, 2)}\n`
await mkdir(outputDirectory, { recursive: true })
try {
  await writeFile(temporary, report)
  await rename(temporary, outputPath)
} finally {
  await rm(temporary, { force: true })
}
const readback = await readFile(outputPath, "utf8")
if (readback !== report) throw new Error("presentation-bound diagnostic readback differs")
console.log(JSON.stringify({ output: outputPath, byteLength: Buffer.byteLength(report) }))
