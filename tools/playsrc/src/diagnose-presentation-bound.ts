import { mkdir, readFile, rename, rm, writeFile } from "node:fs/promises"
import path from "node:path"
import os from "node:os"
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
  "--features",
  "presentation-bound-diagnostic",
], { cwd: repositoryRoot, env: environment, stdout: "ignore", stderr: "inherit" })
if (await build.exited !== 0) throw new Error("presentation-bound diagnostic build failed")
const executable = path.join(
  repositoryRoot,
  "target",
  "source-bundle",
  process.platform === "win32" ? "playsrc-source-bundle.exe" : "playsrc-source-bundle",
)

type Sample = Readonly<{ report: Readonly<Record<string, unknown>>; phaseMilliseconds: unknown; wallMilliseconds: number; peakResidentBytes: number | null; cpu: unknown }>

async function residentBytes(pid: number): Promise<number | null> {
  const command = process.platform === "win32"
    ? ["tasklist", "/fi", `PID eq ${pid}`, "/fo", "csv", "/nh"]
    : ["ps", "-o", "rss=", "-p", String(pid)]
  const child = Bun.spawn(command, { stdout: "pipe", stderr: "ignore" })
  const output = (await new Response(child.stdout).text()).trim()
  if (await child.exited !== 0 || output.length === 0) return null
  if (process.platform === "win32") {
    const match = output.match(/,"([\d,]+) K"$/)
    return match ? Number(match[1]!.replaceAll(",", "")) * 1024 : null
  }
  const kib = Number(output)
  return Number.isSafeInteger(kib) && kib >= 0 ? kib * 1024 : null
}

async function sample(): Promise<Sample> {
  const started = performance.now()
  const child = Bun.spawn([executable, target, "--diagnose-presentation-bound"], {
    cwd: repositoryRoot,
    env: environment,
    stdout: "pipe",
    stderr: "inherit",
  })
  let done = false
  let peakResidentBytes: number | null = null
  const exited = child.exited.finally(() => { done = true })
  while (!done) {
    const bytes = await residentBytes(child.pid)
    if (bytes !== null) peakResidentBytes = Math.max(peakResidentBytes ?? 0, bytes)
    await Bun.sleep(50)
  }
  const output = await new Response(child.stdout).text()
  if (await exited !== 0) throw new Error("presentation-bound diagnostic failed")
  const value = JSON.parse(output) as Record<string, unknown>
  if (value.schema !== "playsrc-static-prop-producer-diagnostic-v1" || value.target !== target) {
    throw new Error("presentation-bound diagnostic report is malformed")
  }
  const presentation = value.presentation as Record<string, unknown> | undefined
  const phaseMilliseconds = presentation?.phaseMilliseconds ?? null
  if (presentation) delete presentation.phaseMilliseconds
  const usage = (child as unknown as { resourceUsage?: () => unknown }).resourceUsage?.() ?? null
  return Object.freeze({
    report: Object.freeze(value),
    phaseMilliseconds,
    wallMilliseconds: Number((performance.now() - started).toFixed(3)),
    peakResidentBytes,
    cpu: usage,
  })
}

const first = await sample()
const second = await sample()
const third = await sample()
if (JSON.stringify(first.report) !== JSON.stringify(second.report)
  || JSON.stringify(first.report) !== JSON.stringify(third.report)) {
  throw new Error("presentation-bound diagnostic output is not deterministic")
}
const outputDirectory = path.join(local.sourceCacheDir, "evidence", "presentation-bound", target)
const outputPath = path.join(outputDirectory, "report.json")
const temporary = `${outputPath}.tmp-${process.pid}`
const samples = [first, second, third]
const walls = samples.map((sample) => sample.wallMilliseconds).sort((left, right) => left - right)
const residents = samples.flatMap((sample) => sample.peakResidentBytes === null ? [] : [sample.peakResidentBytes]).sort((left, right) => left - right)
const reportValue = Object.freeze({
  schema: "playsrc-presentation-bound-diagnostic-v2",
  target,
  interval: Object.freeze({
    starts: "admission of exact configured BSP and resolved source-object bytes by the native production compiler",
    ends: "complete production presentation serialization under the explicit diagnostic-only 1 GiB bound",
    excluded: Object.freeze(["network acquisition", "browser parse", "IndexedDB", "GPU upload", "Ready publication"]),
  }),
  host: Object.freeze({
    platform: process.platform,
    architecture: process.arch,
    logicalCpuCount: os.cpus().length,
    totalMemoryBytes: os.totalmem(),
  }),
  producer: first.report,
  samples: Object.freeze(samples.map(({ report: _, ...sample }) => sample)),
  wallMilliseconds: Object.freeze({ minimum: walls[0]!, median: walls[1]!, maximum: walls[2]! }),
  peakResidentBytes: residents.length === 3
    ? Object.freeze({ minimum: residents[0]!, median: residents[1]!, maximum: residents[2]! })
    : null,
})
const report = `${JSON.stringify(reportValue, null, 2)}\n`
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
