import { describe, expect, test } from "bun:test"
import { parseSourceBundleCache, parseSourceBundleReport } from "../src/source-bundle"
import { TF2_CONTENT_BUILD } from "@playsrc/game-tf2-browser/content-build"

const graphSha256 = "1".repeat(64)
const ledgerSha256 = "2".repeat(64)
const valid = {
  target: "jump_beef",
  contentBuild: TF2_CONTENT_BUILD.contentBuild,
  providers: 13,
  requests: 345,
  authoritativeAbsences: 49,
  entries: 296,
  packedEntries: 0,
  derivedEntries: 258,
  graphEntries: 554,
  graphChunks: 42,
  graphEncodedBytes: 30_000_000,
  graphDescriptor: {
    kind: "source-root",
    mediaType: "application/vnd.playsrc.resource-graph+json",
    byteLength: "12345",
    sha256: graphSha256,
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
      contentBuild: TF2_CONTENT_BUILD.contentBuild,
      providers: 13,
      requests: 345,
      authoritativeAbsences: 49,
      entries: 296,
      packedEntries: 0,
      derivedEntries: 258,
      graphEntries: 554,
      graphChunks: 42,
      graphEncodedBytes: 30_000_000,
      graphDescriptor: valid.graphDescriptor,
      ledgerDescriptor: valid.ledgerDescriptor,
    })
  })

  test("rejects stale builds and descriptor/report disagreements", () => {
    expect(() => parseSourceBundleReport(JSON.stringify({ ...valid, contentBuild: "1" }), "jump_beef"))
      .toThrow("source bundle report is malformed")
    expect(() => parseSourceBundleReport(JSON.stringify({
      ...valid,
      graphDescriptor: { ...valid.graphDescriptor, kind: "derived-object" },
    }), "jump_beef")).toThrow("source bundle object descriptor differs")
  })

  test("reuses only a report bound to the exact generator", () => {
    const generator = "4".repeat(64)
    const source = "6".repeat(64)
    const cache = JSON.stringify({
      schema: "playsrc-resource-graph-cache-v2",
      generatorSha256: generator,
      sourceIdentity: source,
      report: valid,
    })
    expect(parseSourceBundleCache(cache, "jump_beef", generator, source)).toEqual({
      target: "jump_beef",
      contentBuild: TF2_CONTENT_BUILD.contentBuild,
      providers: 13,
      requests: 345,
      authoritativeAbsences: 49,
      entries: 296,
      packedEntries: 0,
      derivedEntries: 258,
      graphEntries: 554,
      graphChunks: 42,
      graphEncodedBytes: 30_000_000,
      graphDescriptor: valid.graphDescriptor,
      ledgerDescriptor: valid.ledgerDescriptor,
    })
    expect(parseSourceBundleCache(cache, "jump_beef", "5".repeat(64), source)).toBeNull()
    expect(parseSourceBundleCache(cache, "jump_beef", generator, "7".repeat(64))).toBeNull()
    expect(parseSourceBundleCache(JSON.stringify({ ...JSON.parse(cache), extra: true }), "jump_beef", generator, source)).toBeNull()
    expect(parseSourceBundleCache("not-json", "jump_beef", generator, source)).toBeNull()
  })
})
