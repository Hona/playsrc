import maps from "../../maps.json"

export type Tf2TargetName = keyof typeof maps

const names = Object.keys(maps) as Tf2TargetName[]

// Integration targets are explicit local development inputs, not released capabilities.
export const TF2_TARGET_NAMES = Object.freeze(names.filter((name) => maps[name].admission === "released"))
export const TF2_DEVELOPMENT_TARGET_NAMES = Object.freeze([
  ...TF2_TARGET_NAMES,
  ...names.filter((name) => maps[name].admission === "integration"),
])

export function tf2MapBsp(target: Tf2TargetName): Readonly<{ byteLength: string; sha256: string }> {
  const source = maps[target]
  return "installed" in source
    ? Object.freeze({ byteLength: String(source.installed.byteLength), sha256: source.installed.sha256 })
    : Object.freeze({ byteLength: String(source.download.decodedByteLength), sha256: source.download.decodedSha256 })
}
