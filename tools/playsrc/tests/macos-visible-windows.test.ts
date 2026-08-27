import { expect, test } from "bun:test"
import { admitMacWindow } from "../profile/macos-visible-windows"

test("native admission retains layer-eight alerts above a focused browser", () => {
  const browser = { id: 1, pid: 100, owner: "Chrome", layer: 0, alpha: 1, bounds: { X: 0, Y: 25, Width: 1280, Height: 720 } }
  const alert = { ...browser, id: 2, pid: 200, owner: "UserNotificationCenter", layer: 8, bounds: { X: 400, Y: 300, Width: 300, Height: 200 } }
  const screens = [{ X: 0, Y: 0, Width: 1920, Height: 1080 }]
  expect(admitMacWindow({ screens, windows: [alert, browser] }, 100).occluders).toEqual([alert])
  expect(admitMacWindow({ screens, windows: [browser, alert] }, 100).occluders).toEqual([])
  expect(admitMacWindow({ screens, windows: [{ ...alert, alpha: 0 }, browser] }, 100).occluders).toEqual([])
  expect(() => admitMacWindow({ screens, windows: [alert] }, 100)).toThrow("not on screen")
  expect(() => admitMacWindow({ screens, windows: [{ ...browser, bounds: { ...browser.bounds, X: 1000 } }] }, 100)).toThrow("fully contained")
})
