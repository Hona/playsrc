import { expect, test } from "bun:test"
import { awaitMacBrowserOverlay, type MacPageAdmission } from "../profile/macos-page-admission"

const record = (occluder?: number) => ({ at: Date.now(), linkage: {}, window: {}, page: { browserPid: 7 },
  occluders: occluder ? [{ pid: occluder, owner: "test", id: 3, layer: 999 }] : [] }) as MacPageAdmission

test("pre-sample browser overlay wait retains rejected reads, not admitted sample records", async () => {
  const retained: MacPageAdmission[] = [], readings = [record(7), record()]
  await awaitMacBrowserOverlay(async () => readings.shift()!, retained)
  expect(retained).toHaveLength(2)
  expect(retained[0]!.occluders).toHaveLength(1)
  expect(retained[1]!.occluders).toHaveLength(0)
})

test("foreign occlusion and an expired browser notice fail without whitelisting", async () => {
  for (const occluder of [7, 8]) {
    const retained: MacPageAdmission[] = []
    await expect(awaitMacBrowserOverlay(async () => record(occluder), retained, 0)).rejects.toThrow("occluded")
    expect(retained).toHaveLength(1)
  }
})
