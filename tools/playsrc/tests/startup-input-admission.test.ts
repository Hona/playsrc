import { expect, test } from "bun:test"
import { admitStartupInput, guardStartupInput } from "../profile/application-test"

test("diagnostic startup Escape cannot precede native admission or repair a failed foreground guard", async () => {
  const page = {}, steps: string[] = []
  guardStartupInput(page, async () => { steps.push("native-read"); throw new Error("owned popup is foreground") })
  await expect(admitStartupInput(page, async () => { steps.push("Escape") })).rejects.toThrow("owned popup")
  expect(steps).toEqual(["native-read"])
  guardStartupInput(page, async () => { steps.push("admitted") })
  await admitStartupInput(page, async () => { steps.push("Escape") })
  expect(steps).toEqual(["native-read", "admitted", "Escape"])
})
