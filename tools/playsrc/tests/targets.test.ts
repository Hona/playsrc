import { describe, expect, test } from "bun:test"
import { resolveMapTarget, TargetError } from "../src/targets"
import maps from "../../../games/tf2/maps.json"
import { TF2_TARGET_NAMES, TF2_DEVELOPMENT_TARGET_NAMES, tf2MapBsp } from "@playsrc/game-tf2-browser/maps"

describe("map target registry", () => {
  test("separates released targets from explicit integration admission", () => {
    expect(TF2_TARGET_NAMES).toEqual(["jump_beef", "pl_upward", "ctf_2fort"])
    expect(TF2_DEVELOPMENT_TARGET_NAMES.slice(0, 3)).toEqual(["jump_beef", "pl_upward", "ctf_2fort"])
    for (const [name, map] of Object.entries(maps)) {
      expect(TF2_DEVELOPMENT_TARGET_NAMES.includes(name as keyof typeof maps)).toBe(map.admission !== "source")
    }
    expect(tf2MapBsp("koth_viaduct")).toEqual({ byteLength: "41690668", sha256: "b3574e496550311f5036997ed7bf3d1007be7fe28236f8f33a2352fe0518729c" })
  })

  test("resolves only the exact declared jump_beef identity", () => {
    expect(resolveMapTarget("jump_beef")).toMatchObject({
      admission: "playable",
      mode: "custom",
      navigation: null,
      logicalPath: "maps/jump_beef.bsp",
      download: {
        url: "https://static.tempus2.xyz/tempus/server/maps/jump_beef.bsp.bz2",
        compression: "bzip2",
        encodedByteLength: 12154530,
        encodedSha256: "b8d257bc568964ecf9c5e9dc2c4015bcbac204d7fb3e54ee2d92b0de8b61292c",
        decodedByteLength: 33379388,
        decodedSha256: "b2e22010b56aa03387c76396a55f2fb83cdeb72a9562ed16cfb656a747e58959",
      },
    })
  })

  test("resolves the exact configured pl_upward installed-build identity", () => {
    expect(resolveMapTarget("pl_upward")).toMatchObject({
      admission: "playable",
      mode: "payload",
      navigation: "local",
      logicalPath: "maps/pl_upward.bsp",
      installed: {
        contentBuild: "24245096",
        provider: "game-09-tf",
        byteLength: 25_446_018,
        sha256: "15cbf91981b0d9902c645d1992d196b7e630742aa85111ed834d231f3c3a5709",
      },
    })
  })

  test("resolves the exact configured ctf_2fort installed-build identity", () => {
    expect(resolveMapTarget("ctf_2fort")).toMatchObject({
      admission: "playable",
      mode: "capture-the-flag",
      navigation: "local",
      logicalPath: "maps/ctf_2fort.bsp",
      installed: {
        contentBuild: "24245096",
        provider: "game-09-tf",
        byteLength: 22_751_863,
        sha256: "cbd191411c0be57099da73458167001ec80d58bf37c71cb3c36b2911b6e80fd7",
      },
    })
  })

  test("rejects missing, malformed, aliased, and undeclared targets", () => {
    for (const [identity, code] of [
      [undefined, "TargetMalformed"],
      ["../jump_beef", "TargetMalformed"],
      ["JUMP_BEEF", "TargetMalformed"],
      ["jump-beef", "TargetMalformed"],
      ["jump_other", "TargetMissing"],
    ] as const) {
      try {
        resolveMapTarget(identity)
        throw new Error("target unexpectedly resolved")
      } catch (error) {
        expect(error).toBeInstanceOf(TargetError)
        expect((error as TargetError).code).toBe(code)
      }
    }
  })

  test("pins the six control-point and four king-of-the-hill installed BSPs", () => {
    const additions = Object.keys(maps).map(resolveMapTarget).filter((map) =>
      map.mode === "control-point" || map.mode === "king-of-the-hill")
    expect(additions.filter((map) => map.mode === "control-point")).toHaveLength(6)
    expect(additions.filter((map) => map.mode === "king-of-the-hill")).toHaveLength(4)
    for (const map of additions) {
      expect(map.download).toBeUndefined()
      expect(map.installed?.contentBuild).toBe("24245096")
      expect(map.installed?.provider).toBe("game-09-tf")
      expect(map.installed?.byteLength).toBeGreaterThan(0)
      expect(map.installed?.sha256).toMatch(/^[0-9a-f]{64}$/)
      expect(["local", "content"]).toContain(map.navigation)
    }
  })
})
