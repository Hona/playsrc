#!/usr/bin/env bun

import { createHash } from "node:crypto"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig, repositoryRoot } from "../../../../tools/playsrc/src/config"

type MapSource = Readonly<{
  logicalPath: string
  download?: Readonly<{ decodedByteLength: number; decodedSha256: string }>
  installed?: Readonly<{ contentBuild: string; byteLength: number; sha256: string }>
}>

type OwnerScenario = Readonly<{
  name: string
  samples: number
  candidateNanoseconds: number[]
  mainViewNanoseconds: number[]
  duplicateViewNanoseconds: number[]
  totalNanoseconds: number[]
  cacheIdentities: string[]
  outputSha256: string[]
  skyKinds: number[]
}>

function distribution(values: readonly number[]) {
  if (values.length === 0 || values.some((value) => !Number.isSafeInteger(value) || value < 0)) {
    throw new Error("Visibility owner profile contains invalid phase samples")
  }
  const ordered = [...values].sort((left, right) => left - right)
  const percentile = (fraction: number) => ordered[Math.min(ordered.length - 1, Math.floor(ordered.length * fraction))]!
  return { samples: ordered.length, minimum: ordered[0]!, p50: percentile(0.5), p95: percentile(0.95), p99: percentile(0.99), maximum: ordered.at(-1)! }
}

async function main(arguments_: string[]): Promise<void> {
  if (arguments_.length !== 1 || !["jump_beef", "pl_upward", "ctf_2fort"].includes(arguments_[0]!)) {
    throw new Error("Usage: bun packages/world/visibility/scripts/profile.ts <jump_beef|pl_upward|ctf_2fort>")
  }

  const target = arguments_[0]!
  const [config, mapsText, buildText] = await Promise.all([
    loadLocalConfig(repositoryRoot),
    readFile(path.join(repositoryRoot, "games/tf2/maps.json"), "utf8"),
    readFile(path.join(repositoryRoot, "games/tf2/content-build.json"), "utf8"),
  ])
  const maps = JSON.parse(mapsText) as Record<string, MapSource>
  const build = JSON.parse(buildText) as { contentBuild?: string }
  const source = maps[target]
  if (!source || source.logicalPath !== `maps/${target}.bsp` || Boolean(source.download) === Boolean(source.installed)) {
    throw new Error(`Declared map source is malformed: ${target}`)
  }
  if (source.installed && source.installed.contentBuild !== build.contentBuild) {
    throw new Error(`Declared map content build differs: ${target}`)
  }

  const expectedHash = source.download?.decodedSha256 ?? source.installed!.sha256
  const expectedLength = source.download?.decodedByteLength ?? source.installed!.byteLength
  const object = source.download
    ? path.join(config.sourceCacheDir, "objects", "sha256", expectedHash.slice(0, 2), expectedHash)
    : path.join(config.tf2Dir, source.logicalPath)
  const bytes = await readFile(object)
  const actualHash = createHash("sha256").update(bytes).digest("hex")
  if (bytes.byteLength !== expectedLength || actualHash !== expectedHash) {
    throw new Error(`Configured map differs from its declared identity: ${source.logicalPath}`)
  }

  const buildProcess = Bun.spawn(["cargo", "build", "--release", "-p", "playsrc-visibility", "--example", "profile"], {
    cwd: repositoryRoot,
    stdout: "inherit",
    stderr: "inherit",
    stdin: "ignore",
  })
  if (await buildProcess.exited !== 0) throw new Error("Visibility profile executable build failed")

  const executable = path.join(repositoryRoot, "target", "release", "examples", process.platform === "win32" ? "profile.exe" : "profile")
  const child = Bun.spawn([executable, target], {
    cwd: repositoryRoot,
    stdout: "pipe",
    stderr: "inherit",
    stdin: "pipe",
  })
  child.stdin.write(bytes)
  await child.stdin.flush()
  child.stdin.end()
  const output = await new Response(child.stdout).text()
  if (await child.exited !== 0) throw new Error("Visibility owner profile failed")
  const phases = JSON.parse(output) as Record<string, unknown> & { scenarios: OwnerScenario[] }
  if (!Array.isArray(phases.scenarios)) throw new Error("Visibility owner profile omitted its scenarios")
  const scenarios = phases.scenarios.map((scenario) => ({
    ...scenario,
    distributionsNanoseconds: {
      candidate: distribution(scenario.candidateNanoseconds),
      mainView: distribution(scenario.mainViewNanoseconds),
      duplicateView: distribution(scenario.duplicateViewNanoseconds),
      total: distribution(scenario.totalNanoseconds),
    },
  }))
  const report = {
    schema: "playsrc-visibility-owner-profile-v1",
    runtime: "native-rust-diagnostic-not-browser-evidence",
    target,
    contentBuild: build.contentBuild,
    logicalPath: source.logicalPath,
    byteLength: bytes.byteLength,
    sha256: actualHash,
    measuredAt: new Date().toISOString(),
    ...phases,
    scenarios,
  }
  const directory = path.join(config.sourceCacheDir, "evidence", "tf2-browser-performance")
  await mkdir(directory, { recursive: true })
  const filename = `visibility-owner-${target}-${report.measuredAt.replaceAll(":", "-")}-${process.pid}.json`
  const reportPath = path.join(directory, filename)
  await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`)
  console.log(JSON.stringify({
    reportPath,
    target,
    topology: phases.topology,
    scenarios: scenarios.map(({ name, distributionsNanoseconds }) => ({ name, distributionsNanoseconds })),
  }))
}

try {
  await main(process.argv.slice(2))
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error))
  process.exitCode = 1
}
