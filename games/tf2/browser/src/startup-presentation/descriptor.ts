export type Tf2StartupMediaFile = Readonly<{
  logicalPath: string
  providerIdentity: "game-10-hl2"
  providerRevision: "24207079"
  byteLength: number
  sha256: string
  container: "bink" | "webm"
  video: Readonly<{
    codec: "binkvideo" | "vp9"
    width: number
    height: number
    frameRateNumerator: number
    frameRateDenominator: number
    frameCount: number
    decodedFrameLedgerSha256: string
  }>
  audio: Readonly<{
    codec: "binkaudio-dct" | "vorbis"
    sampleRate: number
    channels: 2
    decodedPcmSha256: string
  }>
  durationMicroseconds: number
}>

export type Tf2StartupDescriptor = Readonly<{
  schema: "playsrc-tf2-startup-presentation-v1"
  contentBuild: "24207079"
  manifest: Readonly<{
    logicalPath: "media/startupvids.txt"
    byteLength: 16
    sha256: "b832a9961d1feeb7a723b03a5033a59790cc82c5c742fbffd90f197bead13f7c"
    entries: readonly ["media/valve.bik"]
    providerIdentity: "game-09-tf"
    providerRevision: "24207079"
    checkedLocations: readonly ["game-09-tf!media/startupvids.txt"]
  }>
  source: Tf2StartupMediaFile
  browserRepresentation: Tf2StartupMediaFile
}>

export type Tf2StartupDescriptorResult =
  | Readonly<{ ok: true; descriptor: Tf2StartupDescriptor }>
  | Readonly<{ ok: false; code: "InvalidDescriptor" | "ChangedConfiguredMedia"; subject: string }>

const SHA256 = /^[0-9a-f]{64}$/u

export const TF2_CONFIGURED_STARTUP: Tf2StartupDescriptor = Object.freeze({
  schema: "playsrc-tf2-startup-presentation-v1",
  contentBuild: "24207079",
  manifest: Object.freeze({
    logicalPath: "media/startupvids.txt",
    byteLength: 16,
    sha256: "b832a9961d1feeb7a723b03a5033a59790cc82c5c742fbffd90f197bead13f7c",
    entries: Object.freeze(["media/valve.bik"] as const),
    providerIdentity: "game-09-tf",
    providerRevision: "24207079",
    checkedLocations: Object.freeze(["game-09-tf!media/startupvids.txt"] as const),
  }),
  source: Object.freeze({
    logicalPath: "media/valve.bik",
    providerIdentity: "game-10-hl2",
    providerRevision: "24207079",
    byteLength: 14_672_796,
    sha256: "99a57640d7434a7ef948dd00980e752f237e4b412dbcf502529832f679065381",
    container: "bink",
    video: Object.freeze({ codec: "binkvideo", width: 1_024, height: 768, frameRateNumerator: 1_199, frameRateDenominator: 50, frameCount: 240, decodedFrameLedgerSha256: "91bd0b3dab9a68f9a36f945fd73b58d0a4c62421c83f2bb0a3d67c6af7d67096" }),
    audio: Object.freeze({ codec: "binkaudio-dct", sampleRate: 44_100, channels: 2, decodedPcmSha256: "a340e226117fbc483c0136f6aafb6b5398bb095a24eec6d571b7cd3626e5afc9" }),
    durationMicroseconds: 10_008_340,
  }),
  browserRepresentation: Object.freeze({
    logicalPath: "media/valve.webm",
    providerIdentity: "game-10-hl2",
    providerRevision: "24207079",
    byteLength: 1_323_798,
    sha256: "1cd960acdfe89e99aebe1b5199c2699b5bb17d812ff069d26ee1192435bbd403",
    container: "webm",
    video: Object.freeze({ codec: "vp9", width: 1_440, height: 1_080, frameRateNumerator: 30, frameRateDenominator: 1, frameCount: 301, decodedFrameLedgerSha256: "bf2f41800e3a45ec5a6de561bea4801a1fba914959efeeab44bc55e04e9a2e0f" }),
    audio: Object.freeze({ codec: "vorbis", sampleRate: 48_000, channels: 2, decodedPcmSha256: "8e7b4e76135103c107a3ac4f6ebb1e1598496bc9affcbbe1b899e8b47dbc607c" }),
    durationMicroseconds: 10_051_000,
  }),
})

function media(value: unknown): value is Tf2StartupMediaFile {
  if (typeof value !== "object" || value === null) return false
  const file = value as Partial<Tf2StartupMediaFile>
  return typeof file.logicalPath === "string"
    && file.providerIdentity === "game-10-hl2" && file.providerRevision === "24207079"
    && Number.isSafeInteger(file.byteLength) && file.byteLength! > 0
    && typeof file.sha256 === "string" && SHA256.test(file.sha256)
    && (file.container === "bink" || file.container === "webm")
    && typeof file.video === "object" && file.video !== null
    && Number.isSafeInteger(file.video.width) && file.video.width > 0
    && Number.isSafeInteger(file.video.height) && file.video.height > 0
    && Number.isSafeInteger(file.video.frameRateNumerator) && file.video.frameRateNumerator > 0
    && Number.isSafeInteger(file.video.frameRateDenominator) && file.video.frameRateDenominator > 0
    && Number.isSafeInteger(file.video.frameCount) && file.video.frameCount > 0
    && SHA256.test(file.video.decodedFrameLedgerSha256)
    && typeof file.audio === "object" && file.audio !== null
    && Number.isSafeInteger(file.audio.sampleRate) && file.audio.sampleRate > 0
    && file.audio.channels === 2 && SHA256.test(file.audio.decodedPcmSha256)
    && Number.isSafeInteger(file.durationMicroseconds) && file.durationMicroseconds! > 0
}

export function validateTf2StartupDescriptor(value: unknown): Tf2StartupDescriptorResult {
  if (typeof value !== "object" || value === null) return Object.freeze({ ok: false, code: "InvalidDescriptor", subject: "root" })
  const descriptor = value as Partial<Tf2StartupDescriptor>
  if (descriptor.schema !== TF2_CONFIGURED_STARTUP.schema || descriptor.contentBuild !== "24207079" || !media(descriptor.source) || !media(descriptor.browserRepresentation)) {
    return Object.freeze({ ok: false, code: "InvalidDescriptor", subject: "shape" })
  }
  if (JSON.stringify(descriptor) !== JSON.stringify(TF2_CONFIGURED_STARTUP)) {
    return Object.freeze({ ok: false, code: "ChangedConfiguredMedia", subject: "build-24207079" })
  }
  return Object.freeze({ ok: true, descriptor: TF2_CONFIGURED_STARTUP })
}
