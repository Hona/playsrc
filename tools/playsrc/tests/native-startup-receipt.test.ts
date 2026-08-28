import { expect, test } from "bun:test"
import { NativeStartupProbeError, nativeProbeResponse } from "../profile/native-startup"

test("failed native capture retains both observations, including zero handles, without admitting them", () => {
  const receipt = { before: { handle: 123, ownerPid: 45, windowClass: "Chrome_WidgetWin_1", idleMilliseconds: 10425281 },
    after: { handle: 0, ownerPid: 0, windowClass: null, idleMilliseconds: 10425384 },
    helper: { pid: 67 }, captureStartedEpoch: 1000, captureEndedEpoch: 1103,
    diagnosticPixels: { admitted: false, privacy: "private-desktop-never-upload", path: "rejected.png" } }
  let caught: unknown
  try { nativeProbeResponse({ error: "Native foreground changed during pixel capture", receipt }) } catch(error) { caught=error }
  expect(caught).toBeInstanceOf(NativeStartupProbeError)
  expect((caught as NativeStartupProbeError).receipt).toBe(receipt)
  expect((caught as Error).message).toBe("Native foreground changed during pixel capture")
})

test("successful readback remains unchanged and missing causal data cannot erase a rejection", () => {
  const response = { id: 1, foreground: 123, receipt: { helper: { pid: 67 } } }
  expect(nativeProbeResponse(response)).toBe(response)
  expect(() => nativeProbeResponse({ error: "capture rejected" })).toThrow("capture rejected")
})
