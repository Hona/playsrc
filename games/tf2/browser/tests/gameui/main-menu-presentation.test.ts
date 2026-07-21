import { expect, test } from "bun:test"
import { TF2_MAIN_MENU_STATE } from "../../src/gameui"
import { tf2CharacterImageVisible, tf2MainMenuAspectCondition } from "../../src/gameui-integration/runtime"

test("shows the configured character image only on the out-of-game Main Menu", () => {
  expect(tf2CharacterImageVisible(TF2_MAIN_MENU_STATE, true)).toBe(true)
  expect(tf2CharacterImageVisible(TF2_MAIN_MENU_STATE, false)).toBe(false)
  expect(tf2CharacterImageVisible(Object.freeze({ kind: "in-game" }), true)).toBe(false)
})

test("selects configured Main Menu aspect conditions across the viewport matrix", () => {
  const viewport = (width: number, height: number, devicePixelRatio = 1) => ({ width, height, devicePixelRatio })
  expect([
    [1192, 1339], [1280, 720], [1024, 768], [2560, 1080], [390, 844], [844, 390],
  ].map(([width, height]) => tf2MainMenuAspectCondition(viewport(width!, height!)))).toEqual([
    "if_taller", "if_wider", "if_taller", "if_wider", "if_taller", "if_wider",
  ])
  expect(tf2MainMenuAspectCondition(viewport(1280, 720, 2))).toBe("if_wider")
})
