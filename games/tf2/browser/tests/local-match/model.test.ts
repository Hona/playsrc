import { describe, expect, test } from "bun:test"
import {
  createTf2LocalMatchMaps,
  createTf2OfflinePracticeCatalog,
  tf2LocalMatchLaunch,
} from "../../src/local-match/model"
import { tf2UiResources } from "../../src/ui-resources"

const source = tf2UiResources.panels.find((panel) => panel.source.logicalPath === "resource/offline_practice.res")!
const configuredMaps = Object.freeze(["jump_beef", "pl_upward", "ctf_2fort"])

describe("TF2 authored offline practice and local server configuration", () => {
  test("authored attack/defend defaults and both map limits are available through the shared catalog", () => {
    const configured = [...configuredMaps, "cp_dustbowl", "cp_gorge"]
    const practice = createTf2OfflinePracticeCatalog(source, configured)
    expect(practice.defaults.map).toBe("cp_dustbowl")
    expect(practice.maps.filter(map => map.mode === "control-point")).toEqual([
      { identity: "cp_dustbowl", displayName: "Dustbowl", mode: "control-point", minimumPlayers: 12, maximumPlayers: 24 },
      { identity: "cp_gorge", displayName: "Gorge", mode: "control-point", minimumPlayers: 12, maximumPlayers: 24 },
    ])
    for (const map of practice.maps.filter(map => map.mode === "control-point")) {
      expect(tf2LocalMatchLaunch("training", { mapIdentity: map.identity, difficulty: 1, playerCount: 16, quotaMode: "normal" }, map).configuration).toMatchObject({ quota: 15, maximumPlayers: 24, offlinePractice: true })
    }
  })
  test("prepared KOTH maps retain authored practice eligibility and never become custom deathmatch", () => {
    const configured = [...configuredMaps, "koth_viaduct", "koth_sawmill", "koth_harvest_final", "koth_lakeside_final"]
    const practice = createTf2OfflinePracticeCatalog(source, configured)
    expect(practice.maps.filter(map => map.mode === "king-of-the-hill").map(map => map.identity)).toEqual(["koth_viaduct", "koth_lakeside_final", "koth_sawmill"])
    const maps = createTf2LocalMatchMaps(configured, practice)
    expect(maps.filter(map => map.identity.startsWith("koth_")).map(map => map.mode)).toEqual(Array(4).fill("king-of-the-hill"))
    const viaduct = maps.find(map => map.identity === "koth_viaduct")!
    expect(tf2LocalMatchLaunch("training", { mapIdentity: viaduct.identity, difficulty: 2, playerCount: 16, quotaMode: "normal" }, viaduct).configuration.quota).toBe(15)
    expect(tf2LocalMatchLaunch("create-server", { mapIdentity: viaduct.identity, difficulty: 2, playerCount: 23, quotaMode: "normal" }, viaduct).configuration.quota).toBe(23)
    expect(() => createTf2LocalMatchMaps(["koth_unconfigured"], practice)).toThrow("not configured")
  })
  test("retains exact authored defaults and excludes unsupported practice modes and maps", () => {
    const practice = createTf2OfflinePracticeCatalog(source, configuredMaps)
    expect(practice.defaults).toEqual({
      difficulty: 0,
      minimumPlayers: 1,
      maximumPlayers: 32,
      suggestedPlayers: 16,
      map: "cp_dustbowl",
    })
    expect(practice.maps).toEqual([{
      identity: "pl_upward",
      displayName: "Upward",
      mode: "payload",
      minimumPlayers: 12,
      maximumPlayers: 24,
    }])
    expect(createTf2LocalMatchMaps(configuredMaps, practice)).toEqual([
      { identity: "jump_beef", displayName: "jump_beef", mode: "custom", minimumPlayers: 1, maximumPlayers: 32 },
      practice.maps[0],
      { identity: "ctf_2fort", displayName: "2FORT", mode: "capture-the-flag", minimumPlayers: 1, maximumPlayers: 24 },
    ])
  })

  test("subtracts the listen-server host from the authored offline-practice player count", () => {
    const map = createTf2OfflinePracticeCatalog(source, configuredMaps).maps[0]!
    expect(tf2LocalMatchLaunch("training", {
      mapIdentity: "pl_upward", difficulty: 2, playerCount: 8, quotaMode: "fill",
    }, map)).toEqual({
      entry: "training",
      mapIdentity: "pl_upward",
      configuration: {
        quota: 7,
        maximumPlayers: 24,
        mode: "normal",
        difficulty: 2,
        joinAfterPlayer: true,
        autoVacate: false,
        offlinePractice: true,
      },
    })
    expect(tf2LocalMatchLaunch("training", {
      mapIdentity: "pl_upward", difficulty: 0, playerCount: 99, quotaMode: "normal",
    }, map).configuration.quota).toBe(30)
  })

  test("retains normal, fill, and match bot quota modes for every configured server map", () => {
    const maps = createTf2LocalMatchMaps(configuredMaps, createTf2OfflinePracticeCatalog(source, configuredMaps))
    for (const map of maps) for (const mode of ["normal", "fill", "match"] as const) {
      expect(tf2LocalMatchLaunch("create-server", {
        mapIdentity: map.identity, difficulty: 3, playerCount: 6, quotaMode: mode,
      }, map)).toEqual({
        entry: "create-server",
        mapIdentity: map.identity,
        configuration: {
          quota: 6,
          maximumPlayers: map.maximumPlayers,
          mode,
          difficulty: 3,
          joinAfterPlayer: true,
          autoVacate: false,
          offlinePractice: false,
        },
      })
    }
    expect(() => tf2LocalMatchLaunch("training", {
      mapIdentity: "ctf_2fort", difficulty: 1, playerCount: 6, quotaMode: "normal",
    }, maps.find((map) => map.identity === "ctf_2fort")!)).toThrow("offline practice excludes")
  })

  test("fills an exact 24-slot local 2Fort server without reserving an unreachable multiplayer slot", () => {
    const maps = createTf2LocalMatchMaps(configuredMaps, createTf2OfflinePracticeCatalog(source, configuredMaps))
    const map = maps.find((candidate) => candidate.identity === "ctf_2fort")!
    expect(tf2LocalMatchLaunch("create-server", {
      mapIdentity: "ctf_2fort", difficulty: 3, playerCount: 23, quotaMode: "normal",
    }, map).configuration).toEqual({
      quota: 23,
      maximumPlayers: 24,
      mode: "normal",
      difficulty: 3,
      joinAfterPlayer: true,
      autoVacate: false,
      offlinePractice: false,
    })
  })
})
