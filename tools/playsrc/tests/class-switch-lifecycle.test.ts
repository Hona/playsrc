import { describe, expect, test } from "bun:test"
import { summarizeClassSwitchLifecycle } from "../profile/class-switch-lifecycle"

describe("headed class-switch lifecycle attribution", () => {
  test("separates first class admission, retained repeat, and actual weapon-fire edges", () => {
    expect(summarizeClassSwitchLifecycle([
      { at: 10, phase: "key-down", key: "Comma" },
      { at: 14, phase: "class-panel", visible: true },
      { at: 18, phase: "key-down", key: "Digit5" },
      { at: 29, phase: "selected", playerClass: 6 },
      { at: 35, phase: "weapon-fire" },
      { at: 40, phase: "key-down", key: "Comma" },
      { at: 43, phase: "class-panel", visible: true },
      { at: 47, phase: "key-down", key: "Digit5" },
      { at: 52, phase: "selected", playerClass: 6 },
      { at: 56, phase: "weapon-fire" },
    ])).toEqual([
      { playerClass: 6, admission: "first", openedAt: 10, selectedAt: 29, selectionMilliseconds: 19, panelMilliseconds: 4, fireAt: 35, fireMilliseconds: 6 },
      { playerClass: 6, admission: "retained", openedAt: 40, selectedAt: 52, selectionMilliseconds: 12, panelMilliseconds: 3, fireAt: 56, fireMilliseconds: 4 },
    ])
  })

  test("never fabricates missing selection, panel, or firing evidence", () => {
    expect(summarizeClassSwitchLifecycle([{ at: 8, phase: "selected", playerClass: 9 }])).toEqual([
      { playerClass: 9, admission: "first", openedAt: null, selectedAt: 8, selectionMilliseconds: null, panelMilliseconds: null, fireAt: null, fireMilliseconds: null },
    ])
    expect(() => summarizeClassSwitchLifecycle([{ at: 3, phase: "selected", playerClass: 0 }])).toThrow("class identity")
    expect(() => summarizeClassSwitchLifecycle([{ at: 3, phase: "weapon-fire" }, { at: 2, phase: "selected", playerClass: 4 }])).toThrow("ordered")
  })

  test("attributes synchronous menu visibility published before the captured opening key", () => {
    expect(summarizeClassSwitchLifecycle([
      { at: 19.98, phase: "class-panel", visible: true },
      { at: 20, phase: "key-down", key: "Comma" },
      { at: 31, phase: "selected", playerClass: 7 },
    ])[0]).toMatchObject({ openedAt: 20, panelMilliseconds: 0, selectionMilliseconds: 11 })
  })
})
