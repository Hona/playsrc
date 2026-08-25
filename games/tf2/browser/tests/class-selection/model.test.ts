import { describe, expect, test } from "bun:test"
import {
  TF2_CLASS_SELECTION_CLASSES,
  TF2_CLASS_SELECTION_INITIAL_STATE,
  tf2ClassSelectionByMenuIndex,
  tf2ClassSelectionByName,
  tf2ClassSelectionClass,
  tf2ClassSelectionImage,
  transitionTf2ClassSelection,
} from "../../src/class-selection"

describe("authored TF2 class selection", () => {
  test("retains exact Source class identities, display order, names, and model identities", () => {
    expect(TF2_CLASS_SELECTION_CLASSES.map((value) => [value.menuIndex, value.identity, value.name])).toEqual([
      [1, 1, "scout"], [2, 3, "soldier"], [3, 7, "pyro"], [4, 4, "demoman"], [5, 6, "heavyweapons"],
      [6, 9, "engineer"], [7, 5, "medic"], [8, 2, "sniper"], [9, 8, "spy"], [12, 12, "random"],
    ])
    expect(tf2ClassSelectionByMenuIndex(2)?.identity).toBe(3)
    expect(tf2ClassSelectionByName("DEMOMAN")?.shortName).toBe("demo")
    expect(tf2ClassSelectionClass(0)).toBeNull()
    expect(tf2ClassSelectionByMenuIndex(10)).toBeNull()
  })

  test("selects exact authored inactive, RED, and BLU images", () => {
    expect(tf2ClassSelectionImage(4, 2, false)).toBe("class_sel_sm_demo_inactive")
    expect(tf2ClassSelectionImage(4, 2, true)).toBe("class_sel_sm_demo_red")
    expect(tf2ClassSelectionImage(4, 3, true)).toBe("class_sel_sm_demo_blu")
    expect(tf2ClassSelectionImage(12, 3, true)).toBe("class_sel_sm_random_blu")
  })

  test("defaults an initial join to Heavy and prevents dismissal before choosing", () => {
    const opened = transitionTf2ClassSelection(TF2_CLASS_SELECTION_INITIAL_STATE, { kind: "show", team: 2, current: null })
    expect(opened.state).toEqual({ visible: true, team: 2, selected: 6, current: null, initialJoin: true })
    expect(transitionTf2ClassSelection(opened.state, { kind: "cancel" })).toEqual({ disposition: "ignored", state: opened.state, request: null })
    expect(transitionTf2ClassSelection(opened.state, { kind: "select", identity: 3 })).toMatchObject({
      disposition: "applied", state: { visible: false, selected: 3, initialJoin: false },
      request: { kind: "join-class", identity: 3, sourceCommand: "joinclass soldier" },
    })
  })

  test("previews without committing and submits every playable class and random exactly", () => {
    const opened = transitionTf2ClassSelection(TF2_CLASS_SELECTION_INITIAL_STATE, { kind: "show", team: 3, current: 4 }).state
    for (const value of TF2_CLASS_SELECTION_CLASSES) {
      const hover = transitionTf2ClassSelection(opened, { kind: "hover", identity: value.identity })
      if (value.identity !== 4) expect(hover.request).toBeNull()
      const selected = transitionTf2ClassSelection(opened, { kind: "select", identity: value.identity })
      expect(selected.request).toEqual({ kind: "join-class", identity: value.identity, sourceCommand: `joinclass ${value.name}` })
      expect(selected.state.visible).toBeFalse()
    }
    expect(transitionTf2ClassSelection(opened, { kind: "cancel" }).state.visible).toBeFalse()
    expect(transitionTf2ClassSelection(opened, { kind: "team-changed", team: 2 }).state.team).toBe(2)
  })
})
