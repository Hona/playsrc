import maps from "../../maps.json"
export const TF2_MAPS = Object.freeze(maps)

export type Tf2TargetName = keyof typeof maps
export type Tf2MapMode = "custom" | "payload" | "capture-the-flag" | "control-point" | "king-of-the-hill"

export function tf2MapTypeToken(target: string): string {
  const source = Object.hasOwn(maps, target) ? maps[target as Tf2TargetName] : undefined
  switch (source?.mode) {
    case "payload": return "#Gametype_Escort"
    case "capture-the-flag": return "#Gametype_CTF"
    case "control-point": return "#Gametype_CP"
    case "king-of-the-hill": return "#Gametype_Koth"
    default: return ""
  }
}

const names = Object.keys(maps) as Tf2TargetName[]

// Integration targets are explicit local inputs. A release separately names its published subset.
export const TF2_TARGET_NAMES = Object.freeze(names.filter((name) => maps[name].admission === "playable"))
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

export function tf2MapMode(target: string): Tf2MapMode | null {
  return Object.hasOwn(maps, target) ? maps[target as Tf2TargetName].mode as Tf2MapMode : null
}
