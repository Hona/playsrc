import { describe, expect, test } from "bun:test"
import { TF2_CONFIGURED_STARTUP, validateTf2StartupDescriptor } from "../../src/startup-presentation"

describe("configured TF2 startup media", () => {
  test("pins the manifest, selected source, browser representation, and decoded ledgers", () => {
    expect(TF2_CONFIGURED_STARTUP).toMatchObject({
      contentBuild: "24207079",
      manifest: { entries: ["media/valve.bik"], sha256: "b832a9961d1feeb7a723b03a5033a59790cc82c5c742fbffd90f197bead13f7c" },
      source: { logicalPath: "media/valve.bik", byteLength: 14_672_796, sha256: "99a57640d7434a7ef948dd00980e752f237e4b412dbcf502529832f679065381", container: "bink", durationMicroseconds: 10_008_340 },
      browserRepresentation: { logicalPath: "media/valve.webm", byteLength: 1_323_798, sha256: "1cd960acdfe89e99aebe1b5199c2699b5bb17d812ff069d26ee1192435bbd403", container: "webm", durationMicroseconds: 10_051_000 },
    })
    expect(validateTf2StartupDescriptor(TF2_CONFIGURED_STARTUP)).toEqual({ ok: true, descriptor: TF2_CONFIGURED_STARTUP })
    expect(Object.isFrozen(TF2_CONFIGURED_STARTUP.source.video)).toBe(true)
  })

  test("rejects malformed and changed configured media", () => {
    expect(validateTf2StartupDescriptor(null)).toEqual({ ok: false, code: "InvalidDescriptor", subject: "root" })
    expect(validateTf2StartupDescriptor({ ...TF2_CONFIGURED_STARTUP, source: { ...TF2_CONFIGURED_STARTUP.source, byteLength: 0 } })).toEqual({ ok: false, code: "InvalidDescriptor", subject: "shape" })
    expect(validateTf2StartupDescriptor({ ...TF2_CONFIGURED_STARTUP, source: { ...TF2_CONFIGURED_STARTUP.source, sha256: "0".repeat(64) } })).toEqual({ ok: false, code: "ChangedConfiguredMedia", subject: "build-24207079" })
  })
})
