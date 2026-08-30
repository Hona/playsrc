import { expect, test } from "bun:test"
import { browserOwnsKey } from "../src/browser-input"

const key = (code: string, modifiers: Partial<Parameters<typeof browserOwnsKey>[0]> = {}) => ({
  code, metaKey: false, ctrlKey: false, altKey: false, isComposing: false, keyCode: 0, ...modifiers,
})

test("only platform shortcuts and composition bypass Source keyboard routing", () => {
  for (const code of ["Tab", "Space", "ArrowUp", "ArrowDown", "Escape", "ControlLeft", "ShiftLeft", "KeyW"]) expect(browserOwnsKey(key(code))).toBe(false)
  expect(browserOwnsKey(key("Tab", { ctrlKey: true }))).toBe(true)
  expect(browserOwnsKey(key("KeyW", { ctrlKey: true }))).toBe(true)
  expect(browserOwnsKey(key("KeyL", { metaKey: true }))).toBe(true)
  expect(browserOwnsKey(key("ArrowLeft", { altKey: true }))).toBe(true)
  expect(browserOwnsKey(key("Space", { ctrlKey: true }))).toBe(false)
  for (const code of ["KeyA", "KeyC", "KeyX", "KeyV"]) {
    expect(browserOwnsKey(key(code, { ctrlKey: true }))).toBe(false)
    expect(browserOwnsKey(key(code, { metaKey: true }))).toBe(false)
  }
  expect(browserOwnsKey(key("Enter", { isComposing: true }))).toBe(true)
  expect(browserOwnsKey(key("Backquote", { keyCode: 229 }))).toBe(true)
})
