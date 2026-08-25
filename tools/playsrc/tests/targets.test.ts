import { describe, expect, test } from "bun:test"
import { resolveMapTarget, TargetError } from "../src/targets"

describe("map target registry", () => {
  test("resolves only the exact declared jump_beef identity", () => {
    expect(resolveMapTarget("jump_beef")).toEqual({
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
    expect(resolveMapTarget("pl_upward")).toEqual({
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
    expect(resolveMapTarget("ctf_2fort")).toEqual({
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
})
