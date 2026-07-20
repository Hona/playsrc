import input from "../../content-build.json"

export type Tf2ContentBuildContract = Readonly<{
  schema: "playsrc-tf2-content-build-v1"
  appId: "440"
  contentBuild: string
  patchVersion: string
  gameinfoSha256: string
  customModProviders: "workshop-only"
  archiveIndexes: Readonly<{
    tf2Misc: string
    tf2Textures: string
    tf2SoundMisc: string
  }>
  installedDepots: readonly Readonly<{
    depot: string
    manifest: string
    byteLength: string
  }>[]
}>

const DECIMAL = /^(0|[1-9]\d*)$/u
const SHA256 = /^[0-9a-f]{64}$/u

export function parseTf2ContentBuildContract(value: unknown): Tf2ContentBuildContract {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw new Error("TF2 content-build contract is malformed")
  const record = value as Record<string, unknown>
  if (Object.keys(record).sort().join("\0") !== "appId\0archiveIndexes\0contentBuild\0customModProviders\0gameinfoSha256\0installedDepots\0patchVersion\0schema"
    || record.schema !== "playsrc-tf2-content-build-v1" || record.appId !== "440"
    || typeof record.contentBuild !== "string" || !DECIMAL.test(record.contentBuild)
    || typeof record.patchVersion !== "string" || !DECIMAL.test(record.patchVersion)
    || typeof record.gameinfoSha256 !== "string" || !SHA256.test(record.gameinfoSha256)
    || record.customModProviders !== "workshop-only" || typeof record.archiveIndexes !== "object" || record.archiveIndexes === null || Array.isArray(record.archiveIndexes)
    || !Array.isArray(record.installedDepots)
    || record.installedDepots.length !== 3) throw new Error("TF2 content-build contract is malformed")
  const indexes = record.archiveIndexes as Record<string, unknown>
  if (Object.keys(indexes).sort().join("\0") !== "tf2Misc\0tf2SoundMisc\0tf2Textures"
    || typeof indexes.tf2Misc !== "string" || !SHA256.test(indexes.tf2Misc)
    || typeof indexes.tf2Textures !== "string" || !SHA256.test(indexes.tf2Textures)
    || typeof indexes.tf2SoundMisc !== "string" || !SHA256.test(indexes.tf2SoundMisc)) throw new Error("TF2 archive-index contract is malformed")
  const depots = record.installedDepots.map((value) => {
    if (typeof value !== "object" || value === null || Array.isArray(value)) throw new Error("TF2 depot contract is malformed")
    const depot = value as Record<string, unknown>
    if (Object.keys(depot).sort().join("\0") !== "byteLength\0depot\0manifest"
      || typeof depot.depot !== "string" || !DECIMAL.test(depot.depot)
      || typeof depot.manifest !== "string" || !DECIMAL.test(depot.manifest)
      || typeof depot.byteLength !== "string" || !DECIMAL.test(depot.byteLength)) throw new Error("TF2 depot contract is malformed")
    return Object.freeze({ depot: depot.depot, manifest: depot.manifest, byteLength: depot.byteLength })
  })
  if (new Set(depots.map((depot) => depot.depot)).size !== depots.length) throw new Error("TF2 depot contract contains duplicate depots")
  return Object.freeze({
    schema: record.schema,
    appId: record.appId,
    contentBuild: record.contentBuild,
    patchVersion: record.patchVersion,
    gameinfoSha256: record.gameinfoSha256,
    customModProviders: record.customModProviders,
    archiveIndexes: Object.freeze({ tf2Misc: indexes.tf2Misc, tf2Textures: indexes.tf2Textures, tf2SoundMisc: indexes.tf2SoundMisc }),
    installedDepots: Object.freeze(depots),
  })
}

export const TF2_CONTENT_BUILD = parseTf2ContentBuildContract(input)
