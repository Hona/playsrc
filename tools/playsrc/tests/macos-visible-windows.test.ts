import { expect, test } from "bun:test"
import { admitMacWindow } from "../profile/macos-visible-windows"

test("native admission retains layer-eight alerts above a focused browser", () => {
  const browser = { id: 1, pid: 100, owner: "Chrome", layer: 0, alpha: 1, bounds: { X: 0, Y: 25, Width: 1280, Height: 720 } }
  const alert = { ...browser, id: 2, pid: 200, owner: "UserNotificationCenter", layer: 8, bounds: { X: 400, Y: 300, Width: 300, Height: 200 } }
  const screens = [{ X: 0, Y: 0, Width: 1920, Height: 1080 }]
  const cursorLayer = 2147483630, cursor = { ...alert, layer: cursorLayer, owner: "Window Server" }
  expect(admitMacWindow({ screens, cursorLayer, windows: [cursor, alert, browser] }, 100).occluders).toEqual([alert])
  expect(admitMacWindow({ screens, cursorLayer, windows: [cursor, browser] }, 100).cursors).toEqual([cursor])
  const notice = { ...alert, pid: browser.pid, owner: browser.owner, layer: 999 }
  expect(admitMacWindow({ screens, cursorLayer, windows: [notice, alert, browser] }, 100).browserOverlays).toEqual([notice])
  expect(admitMacWindow({ screens, cursorLayer, windows: [notice, alert, browser] }, 100).occluders).toEqual([alert])
  expect(admitMacWindow({ screens, cursorLayer, windows: [browser, alert] }, 100).occluders).toEqual([])
  expect(admitMacWindow({ screens, cursorLayer, windows: [{ ...alert, alpha: 0 }, browser] }, 100).occluders).toEqual([])
  expect(() => admitMacWindow({ screens, cursorLayer, windows: [alert] }, 100)).toThrow("not on screen")
  expect(() => admitMacWindow({ screens, cursorLayer, windows: [{ ...browser, bounds: { ...browser.bounds, X: 1000 } }] }, 100)).toThrow("fully contained")
})
