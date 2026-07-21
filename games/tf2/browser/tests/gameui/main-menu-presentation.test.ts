import { expect, test } from "bun:test"
import { TF2_MAIN_MENU_STATE } from "../../src/gameui"
import { tf2CharacterImageVisible } from "../../src/gameui-integration/runtime"

test("shows the configured character image only on the out-of-game Main Menu", () => {
  expect(tf2CharacterImageVisible(TF2_MAIN_MENU_STATE, true)).toBe(true)
  expect(tf2CharacterImageVisible(TF2_MAIN_MENU_STATE, false)).toBe(false)
  expect(tf2CharacterImageVisible(Object.freeze({ kind: "in-game" }), true)).toBe(false)
})
