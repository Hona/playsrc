import { expect, test } from "bun:test"
import { admitMacWindow, type MacWindowSnapshot, type PageWindowFacts } from "../profile/macos-visible-windows"
import { requireMacPageAdmission } from "../profile/macos-page-admission"

const browser = { id: 1, pid: 100, owner: "Chrome", layer: 0, alpha: 1, bounds: { X: 0, Y: 25, Width: 1282, Height: 800 } }
const popup = { ...browser, id: 2, bounds: { X: 400, Y: 300, Width: 320, Height: 174 } }
const facts: PageWindowFacts = { browserPid: 100, targetId: "main-page", cdpWindowId: 50, bounds: browser.bounds, windowState: "normal", url: "http://localhost/tf2/" }
const snapshot = (windows = [browser]): MacWindowSnapshot => ({ windows, screens: [{ X: 0, Y: 0, Width: 1920, Height: 1080 }], cursorLayer: 2147483630, frontmostPid: browser.pid })

test("a frontmost same-browser popup never substitutes for the measured page", () => {
  const original = admitMacWindow(snapshot(), facts)
  const result = admitMacWindow(snapshot([popup, browser]), facts, original.linkage)
  expect(result.window).toEqual(browser)
  expect(result.occluders).toEqual([popup])
  expect(result.browserOverlays).toEqual([popup])
  expect(result.linkage).toEqual(original.linkage)
  expect(() => requireMacPageAdmission({ at: 0, ...result })).toThrow("occluded")
})

test("navigation and exact native resize retain page, CDP window and native identity", () => {
  const original = admitMacWindow(snapshot(), facts)
  const resized = { ...browser, bounds: { ...browser.bounds, Width: 1026, Height: 780 } }
  const result = admitMacWindow(snapshot([popup, resized]), { ...facts, url: "http://localhost/audit", bounds: resized.bounds }, original.linkage)
  expect(result.window).toEqual(resized)
  expect(result.linkage).toEqual(original.linkage)
  expect(() => admitMacWindow(snapshot([popup]), { ...facts, bounds: popup.bounds }, original.linkage)).toThrow("identity")
  expect(() => admitMacWindow(snapshot(), { ...facts, targetId: "popup" }, original.linkage)).toThrow("identity")
  expect(() => admitMacWindow(snapshot(), { ...facts, cdpWindowId: 51 }, original.linkage)).toThrow("identity")
})

test("exact PID and bounds are required; identical candidates are explicitly ambiguous", () => {
  expect(() => admitMacWindow(snapshot([popup, browser, { ...browser, id: 3 }]), facts)).toThrow("ambiguous")
  expect(() => admitMacWindow(snapshot([browser, { ...browser, id: 3 }]), facts, admitMacWindow(snapshot(), facts).linkage)).toThrow("ambiguous")
  expect(() => admitMacWindow(snapshot([popup]), facts)).toThrow("exact")
  expect(() => admitMacWindow(snapshot([{ ...browser, pid: 101 }]), facts)).toThrow("exact")
  expect(() => admitMacWindow(snapshot(), { ...facts, bounds: { ...browser.bounds, Width: 1281 } })).toThrow("exact")
  expect(() => admitMacWindow(snapshot(), { ...facts, windowState: "minimized" })).toThrow("minimized")
  const offscreen = { ...browser, bounds: { ...browser.bounds, X: 1000 } }
  expect(() => admitMacWindow(snapshot([offscreen]), { ...facts, bounds: offscreen.bounds })).toThrow("fully contained")
})

test("all covering layers including browser notices and layer-eight alerts reject admission", () => {
  const alert = { ...popup, id: 3, pid: 200, owner: "UserNotificationCenter", layer: 8 }
  const cursor = { ...alert, id: 4, layer: snapshot().cursorLayer, owner: "Window Server" }
  const notice = { ...popup, id: 5, layer: 999 }
  const result = admitMacWindow(snapshot([cursor, notice, alert, popup, browser]), facts)
  expect(result.occluders).toEqual([notice, alert, popup])
  expect(result.browserOverlays).toEqual([notice, popup])
  expect(result.cursors).toEqual([cursor])
  expect(admitMacWindow(snapshot([browser, alert]), facts).occluders).toEqual([])
  expect(admitMacWindow(snapshot([{ ...alert, alpha: 0 }, browser]), facts).occluders).toEqual([])
  expect(admitMacWindow(snapshot([{ ...popup, bounds: { ...popup.bounds, X: 1500 } }, browser]), facts).occluders).toEqual([])
})

test("retained native failures and hidden tabs cannot pass the admission gate", () => {
  const clear = { at: 0, ...admitMacWindow(snapshot(), facts), snapshot: snapshot(), document: { url: facts.url, visibility: "visible", focused: true } }
  expect(() => requireMacPageAdmission(clear)).not.toThrow()
  expect(() => requireMacPageAdmission({ ...clear, error: "identity changed" })).toThrow("identity")
  expect(() => requireMacPageAdmission({ ...clear, document: { ...clear.document, visibility: "hidden" } })).toThrow("visible document")
  expect(() => requireMacPageAdmission({ ...clear, snapshot: { ...snapshot(), frontmostPid: 200 } })).toThrow("native foreground")
  expect(() => requireMacPageAdmission({ ...clear, snapshotAfter: { ...snapshot(), frontmostPid: 200 } })).toThrow("native foreground")
  expect(() => requireMacPageAdmission({ at: 0 })).toThrow("Missing")
})
