import { expect, test } from "bun:test"
import { selectDiagnosticModelBase } from "../src/diagnostic-model"

test("diagnostic models preserve authored base textures and identify only missing input", () => {
  expect(selectDiagnosticModelBase(true)).toBe("authored-texture")
  expect(selectDiagnosticModelBase(false)).toBe("identity-color")
})
