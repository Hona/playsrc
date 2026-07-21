import { describe, expect, test } from "bun:test"
import { TF2_STARTUP_LOADING_PLAQUE, tf2StartupLoadingLabel } from "../../src/startup-presentation"

describe("TF2 startup loading plaque", () => {
  test("retains the configured PC texture and native lower-right geometry", () => {
    expect(TF2_STARTUP_LOADING_PLAQUE).toEqual({
      material: "materials/console/startup_loading.vmt",
      texture: "materials/console/startup_loading.vtf",
      textureSize: { width: 128, height: 64 },
      panel: { x: 2, y: 4, width: 110, height: 44, radius: 6 },
      text: { x: 18, y: 4, height: 44 },
    })
    expect(Object.isFrozen(TF2_STARTUP_LOADING_PLAQUE.panel)).toBe(true)
  })

  test("formats only exact integer download percentages", () => {
    expect(tf2StartupLoadingLabel(0)).toBe("Loading 0%...")
    expect(tf2StartupLoadingLabel(100)).toBe("Loading 100%...")
    for (const value of [-1, 1.5, 101, Number.NaN]) expect(() => tf2StartupLoadingLabel(value)).toThrow()
  })
})
