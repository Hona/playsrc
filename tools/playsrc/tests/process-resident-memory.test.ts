import { expect, test } from "bun:test"
import { validateResidentMemory } from "../profile/process-resident-memory"

test("remote resident memory is bound to the exact browser process inventory", () => {
  const processes = [{ id: 14, type: "GPU" }]
  expect(validateResidentMemory(processes, [{ ...processes[0], residentBytes: 1000 }])).toEqual([{ id: 14, type: "GPU", residentBytes: 1000 }])
  expect(validateResidentMemory(processes, [{ ...processes[0], residentBytes: null }])[0]!.residentBytes).toBeNull()
  for (const value of [[], [{ id: 15, type: "GPU", residentBytes: 1000 }], [{ id: 14, type: "renderer", residentBytes: 1000 }], [{ id: 14, type: "GPU", residentBytes: -1 }]]) {
    expect(() => validateResidentMemory(processes, value)).toThrow("Browser-host memory response")
  }
})
