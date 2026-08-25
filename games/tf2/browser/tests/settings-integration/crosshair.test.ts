import { describe, expect, test } from "bun:test"
import { decodeSettingsPersistence, TF2_SELECTED_OPTIONS } from "@playsrc/settings"
import { tf2CrosshairHudValues, tf2CrosshairSettings } from "../../src/hud"
import { initializeTf2BrowserSettings } from "../../src/settings-integration"

function settings(persistence: Uint8Array | null = null) {
  return initializeTf2BrowserSettings({
    persistence,
    owners: { renderer: "available", audio: "available", input: "available", game: "available", application: "available" },
    async apply(request) { return Object.freeze({ requestId: request.requestId, status: "applied" as const }) },
  })
}

describe("TF2 crosshair console and Options share one archived settings authority", () => {
  test("publishes exact five archived ConVar defaults from current state", () => {
    expect(settings().crosshairConVars()).toEqual([
      { name: "cl_crosshair_red", settingId: "multiplayer.crosshair-red", defaultValue: "200", value: "200" },
      { name: "cl_crosshair_green", settingId: "multiplayer.crosshair-green", defaultValue: "200", value: "200" },
      { name: "cl_crosshair_blue", settingId: "multiplayer.crosshair-blue", defaultValue: "200", value: "200" },
      { name: "cl_crosshair_scale", settingId: "multiplayer.crosshair-scale", defaultValue: "32.0", value: "32" },
      { name: "cl_crosshair_file", settingId: "multiplayer.crosshair-file", defaultValue: "", value: "" },
    ])
  })

  test("accepts unbounded manual values, Source byte wrapping, and exact style identities", () => {
    const current = settings()
    expect(current.setCrosshairConVar("CL_CROSSHAIR_RED", "300.75")).toMatchObject({ value: "300.75" })
    expect(current.setCrosshairConVar("cl_crosshair_green", "-1")).toMatchObject({ value: "-1" })
    expect(current.setCrosshairConVar("cl_crosshair_blue", "nonnumeric")).toMatchObject({ value: "0" })
    expect(current.setCrosshairConVar("cl_crosshair_scale", "96")).toMatchObject({ value: "96" })
    expect(current.setCrosshairConVar("cl_crosshair_file", "crosshair6")).toMatchObject({ value: "crosshair6" })
    expect(tf2CrosshairHudValues(tf2CrosshairSettings(current.snapshot().settings.current))).toEqual({
      texture: "vgui/crosshairs/crosshair6",
      color: [44, 255, 0, 255],
      scale: 96,
    })
    expect(() => current.setCrosshairConVar("cl_crosshair_alpha", "255")).toThrow("Unknown TF2 crosshair ConVar")
    expect(() => current.setCrosshairConVar("cl_crosshair_scale", "Infinity")).toThrow("not finite")
  })

  test("restores every manual ConVar through canonical archived persistence", () => {
    const current = settings()
    current.setCrosshairConVar("cl_crosshair_red", "300")
    current.setCrosshairConVar("cl_crosshair_green", "4")
    current.setCrosshairConVar("cl_crosshair_blue", "9")
    current.setCrosshairConVar("cl_crosshair_scale", "64")
    current.setCrosshairConVar("cl_crosshair_file", "default")
    const bytes = current.persistence()
    expect(decodeSettingsPersistence(TF2_SELECTED_OPTIONS, bytes)).toMatchObject({
      ok: true,
      decoded: { values: {
        "multiplayer.crosshair-red": 300,
        "multiplayer.crosshair-green": 4,
        "multiplayer.crosshair-blue": 9,
        "multiplayer.crosshair-scale": 64,
        "multiplayer.crosshair-file": "default",
      } },
    })
    expect(settings(bytes).crosshairConVars()).toEqual(current.crosshairConVars())
  })

  test("keeps Multiplayer edits bounded and preserves the committed value on Cancel", async () => {
    const current = settings()
    current.begin()
    expect(() => current.set("multiplayer.crosshair-red", 256)).toThrow("InvalidValue")
    expect(() => current.set("multiplayer.crosshair-scale", 15)).toThrow("InvalidValue")
    current.set("multiplayer.crosshair-red", 0)
    current.set("multiplayer.crosshair-scale", 48)
    current.set("multiplayer.crosshair-file", "crosshair3")
    expect(current.snapshot().settings.current["multiplayer.crosshair-red"]).toBe(200)
    current.cancel()
    expect(current.snapshot().settings.current["multiplayer.crosshair-red"]).toBe(200)
    current.begin()
    current.set("multiplayer.crosshair-red", 7)
    current.set("multiplayer.crosshair-file", "crosshair3")
    expect((await current.apply()).lastApply?.complete).toBe(true)
    expect(current.crosshairConVars().find((value) => value.name === "cl_crosshair_red")?.value).toBe("7")
    expect(current.crosshairConVars().find((value) => value.name === "cl_crosshair_file")?.value).toBe("crosshair3")
  })
})
