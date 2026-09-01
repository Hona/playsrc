import { expect, test } from "bun:test"
import { assertReleasePackageRun } from "../src/release-package-run"
import { assertPreparedReleaseIdentity, assertReleaseStartupAcceptance } from "../src/deploy"
import { createDeployedBrowserConfiguration, parseTf2Release } from "../../../apps/web/tf2/src/deployment"
import releaseJson from "../../../apps/web/tf2/releases/current.json"

test("only a successful exact-source package workflow can supply deployment bytes", () => {
  const sha = "a".repeat(40), repository = "Hona/playsrc"
  const run = { head_sha: sha, head_branch: "main", path: ".github/workflows/prepare-release.yml", event: "workflow_dispatch", status: "completed", conclusion: "success", repository: { full_name: repository } }
  assertReleasePackageRun(run, sha, repository)
  for (const change of [{ head_sha: "b".repeat(40) }, { head_branch: "topic" }, { path: ".github/workflows/checks.yml" }, { event: "pull_request" }, { status: "in_progress" }, { conclusion: "failure" }, { repository: { full_name: "other/repo" } }]) {
    expect(() => assertReleasePackageRun({ ...run, ...change }, sha, repository)).toThrow("exact main commit")
  }
  expect(() => assertReleasePackageRun(null, sha, repository)).toThrow()
})

test("prepared delivery requires the complete checked descriptor and browser configuration", () => {
  const release = parseTf2Release(releaseJson), build = "a".repeat(64)
  const configuration = createDeployedBrowserConfiguration(release, build)
  assertPreparedReleaseIdentity({ release, configuration }, release, build)
  expect(() => assertPreparedReleaseIdentity({ release, configuration }, release, "b".repeat(64))).toThrow("source build")
  expect(() => assertPreparedReleaseIdentity({ release, configuration: { ...configuration, assetOrigin: "https://invalid.example" } }, release, build)).toThrow("configuration")
  expect(() => assertPreparedReleaseIdentity({ release: { ...release, targets: release.targets.slice(1) }, configuration }, release, build)).toThrow("release descriptor")
})

test("receipt mismatch diagnostics expose bounded identities, not arbitrary receipt content", () => {
  const expected = { packageSha256: "a".repeat(64), wasmSha256: "b".repeat(64) }
  const receipt = { packageSha256: "c".repeat(64), wasmSha256: "private arbitrary content" }
  const environment = { PLAYSRC_RELEASE_VERSION: "0.1.0", PLAYSRC_STATIC_STARTUP_RECEIPT: JSON.stringify(receipt) }
  try { assertReleaseStartupAcceptance(expected, environment); throw new Error("unexpected acceptance") }
  catch (error) {
    expect(String(error)).toContain(`accepted package=${receipt.packageSha256} observed package=${expected.packageSha256}`)
    expect(String(error)).toContain(`accepted WASM=absent-or-invalid observed WASM=${expected.wasmSha256}`)
    expect(String(error)).not.toContain("private arbitrary content")
  }
  // Matching hashes alone still cannot authorize deployment without real capture evidence.
  expect(() => assertReleaseStartupAcceptance(expected, { ...environment, PLAYSRC_STATIC_STARTUP_RECEIPT: JSON.stringify(expected) })).toThrow("acceptance is absent")
})
