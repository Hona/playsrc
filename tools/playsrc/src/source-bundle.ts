import path from "node:path"
import type { LocalConfig } from "./config"
import { repositoryRoot } from "./config"
import { rustEnvironment } from "./setup"
import type { ObjectDescriptor } from "@playsrc/asset-store"
import toolchains from "../toolchains.json"

export type SourceBundleArtifact = Readonly<{
  bundlePath: string
  ledgerPath: string
  report: Readonly<{
    target: string
    contentBuild: "24207079"
    providers: number
    requests: number
    authoritativeAbsences: number
    entries: number
    bundleDescriptor: ObjectDescriptor
    ledgerDescriptor: ObjectDescriptor
  }>
}>

type SourceBundleReport = Readonly<{
  target?: unknown
  contentBuild?: unknown
  providers?: unknown
  requests?: unknown
  authoritativeAbsences?: unknown
  entries?: unknown
  bytes?: unknown
  sha256?: unknown
  bundleDescriptor?: unknown
  ledgerBytes?: unknown
  ledgerSha256?: unknown
  ledgerDescriptor?: unknown
}>

const descriptor = (
  value: unknown,
  kind: "derived-object",
  mediaType: string,
  byteLength: unknown,
  sha256: unknown,
): ObjectDescriptor => {
  if (
    typeof value !== "object"
    || value === null
    || Array.isArray(value)
    || Object.keys(value).sort().join("\0") !== "byteLength\0kind\0mediaType\0sha256"
  ) throw new Error("source bundle object descriptor is malformed")
  const record = value as Record<string, unknown>
  if (
    record.kind !== kind
    || record.mediaType !== mediaType
    || record.byteLength !== String(byteLength)
    || record.sha256 !== sha256
    || !/^(0|[1-9]\d*)$/.test(record.byteLength as string)
    || !/^[0-9a-f]{64}$/.test(record.sha256 as string)
  ) throw new Error("source bundle object descriptor differs from its report")
  return Object.freeze(record as ObjectDescriptor)
}

export function parseSourceBundleReport(output: string, target: string): SourceBundleArtifact["report"] {
  let report: SourceBundleReport
  try {
    report = JSON.parse(output) as SourceBundleReport
  } catch {
    throw new Error("source bundle report is not JSON")
  }
  if (
    report.target !== target
    || report.contentBuild !== "24207079"
    || !Number.isSafeInteger(report.providers)
    || (report.providers as number) < 2
    || (report.providers as number) > 65
    || !Number.isSafeInteger(report.requests)
    || (report.requests as number) < 1
    || (report.requests as number) > 4_096
    || !Number.isSafeInteger(report.authoritativeAbsences)
    || (report.authoritativeAbsences as number) < 0
    || (report.authoritativeAbsences as number) > (report.requests as number)
    || !Number.isSafeInteger(report.entries)
    || (report.entries as number) < 1
    || (report.entries as number) > (report.requests as number)
    || (report.entries as number) + (report.authoritativeAbsences as number) !== report.requests
    || !Number.isSafeInteger(report.bytes)
    || (report.bytes as number) < 12
    || (report.bytes as number) > 512 * 1024 * 1024
    || !Number.isSafeInteger(report.ledgerBytes)
    || (report.ledgerBytes as number) < 2
    || (report.ledgerBytes as number) > 8 * 1024 * 1024
    || typeof report.sha256 !== "string"
    || !/^[0-9a-f]{64}$/.test(report.sha256)
    || typeof report.ledgerSha256 !== "string"
    || !/^[0-9a-f]{64}$/.test(report.ledgerSha256)
  ) {
    throw new Error("source bundle report is malformed")
  }
  const bundleDescriptor = descriptor(
    report.bundleDescriptor,
    "derived-object",
    "application/octet-stream",
    report.bytes,
    report.sha256,
  )
  const ledgerDescriptor = descriptor(
    report.ledgerDescriptor,
    "derived-object",
    "application/vnd.playsrc.source-dependency-ledger+json",
    report.ledgerBytes,
    report.ledgerSha256,
  )
  return Object.freeze({
    target,
    contentBuild: "24207079",
    providers: report.providers as number,
    requests: report.requests as number,
    authoritativeAbsences: report.authoritativeAbsences as number,
    entries: report.entries as number,
    bundleDescriptor,
    ledgerDescriptor,
  })
}

export async function buildSourceBundle(config: LocalConfig, target: string): Promise<SourceBundleArtifact> {
  const executable = process.platform === "win32" ? "cargo.exe" : "cargo"
  const cargo = path.join(config.sourceCacheDir, "toolchains", "rust", "cargo", "bin", executable)
  const child = Bun.spawn([
    cargo,
    `+${toolchains.rust.toolchain}`,
    "run",
    "-p",
    "playsrc-source-bundle",
    "--",
    target,
  ], {
    cwd: repositoryRoot,
    env: { ...process.env, ...rustEnvironment(config.sourceCacheDir) },
    stdout: "pipe",
    stderr: "inherit",
  })
  const output = await new Response(child.stdout).text()
  if (await child.exited !== 0) throw new Error("source bundle build failed")
  const report = parseSourceBundleReport(output, target)
  return Object.freeze({
    bundlePath: path.join(config.sourceCacheDir, "browser-bundles", `${target}.psdb`),
    ledgerPath: path.join(config.sourceCacheDir, "browser-bundles", `${target}.dependencies.json`),
    report,
  })
}
