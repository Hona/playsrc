import { describe, expect, test } from "bun:test"
import input from "../../content-build.json"
import { parseTf2ContentBuildContract, TF2_CONTENT_BUILD } from "../src/content-build"

const clone = (): any => structuredClone(input)

describe("TF2 content-build contract", () => {
  test("retains the exact current retail identity", () => {
    expect(TF2_CONTENT_BUILD).toEqual(input)
    expect(Object.isFrozen(TF2_CONTENT_BUILD)).toBe(true)
    expect(Object.isFrozen(TF2_CONTENT_BUILD.archiveIndexes)).toBe(true)
    expect(Object.isFrozen(TF2_CONTENT_BUILD.installedDepots)).toBe(true)
  })

  test("rejects changed shape, identities, and provider disposition", () => {
    for (const changed of [
      { ...clone(), extra: true },
      { ...clone(), appId: "441" },
      { ...clone(), contentBuild: "" },
      { ...clone(), patchVersion: "current" },
      { ...clone(), gameinfoSha256: "0".repeat(63) },
      { ...clone(), customModProviders: "all" },
      { ...clone(), archiveIndexes: { ...clone().archiveIndexes, tf2Misc: "0".repeat(63) } },
      { ...clone(), installedDepots: clone().installedDepots.slice(0, 2) },
      { ...clone(), installedDepots: clone().installedDepots.map((depot: any) => ({ ...depot, depot: "440" })) },
    ]) expect(() => parseTf2ContentBuildContract(changed)).toThrow()
  })
})
