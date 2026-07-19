import { describe, expect, test } from "bun:test"
import { parseSourceBundleReport } from "../src/source-bundle"

const bundleSha256 = "1".repeat(64)
const ledgerSha256 = "2".repeat(64)
const uiSha256 = "3".repeat(64)
const valid = {
  target: "jump_beef",
  contentBuild: "24207079",
  providers: 13,
  requests: 345,
  authoritativeAbsences: 49,
  entries: 296,
  bytes: 112_303_242,
  sha256: bundleSha256,
  bundleDescriptor: {
    kind: "derived-object",
    mediaType: "application/octet-stream",
    byteLength: "112303242",
    sha256: bundleSha256,
  },
  uiEntries: 200,
  uiBytes: 30_000_000,
  uiSha256,
  uiDescriptor: {
    kind: "derived-object",
    mediaType: "application/octet-stream",
    byteLength: "30000000",
    sha256: uiSha256,
  },
  ledgerBytes: 305_633,
  ledgerSha256,
  ledgerDescriptor: {
    kind: "derived-object",
    mediaType: "application/vnd.playsrc.source-dependency-ledger+json",
    byteLength: "305633",
    sha256: ledgerSha256,
  },
}

describe("source dependency bundle report", () => {
  test("accepts exact bounded bundle and ledger descriptors", () => {
    expect(parseSourceBundleReport(JSON.stringify(valid), "jump_beef")).toEqual({
      target: "jump_beef",
      contentBuild: "24207079",
      providers: 13,
      requests: 345,
      authoritativeAbsences: 49,
      entries: 296,
      bundleDescriptor: valid.bundleDescriptor,
      uiEntries: 200,
      uiDescriptor: valid.uiDescriptor,
      ledgerDescriptor: valid.ledgerDescriptor,
    })
  })

  test("rejects stale builds and descriptor/report disagreements", () => {
    expect(() => parseSourceBundleReport(JSON.stringify({ ...valid, contentBuild: "10822003" }), "jump_beef"))
      .toThrow("source bundle report is malformed")
    expect(() => parseSourceBundleReport(JSON.stringify({
      ...valid,
      bundleDescriptor: { ...valid.bundleDescriptor, byteLength: "1" },
    }), "jump_beef")).toThrow("source bundle object descriptor differs")
  })
})
