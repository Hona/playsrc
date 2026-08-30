import { expect, test } from "bun:test"
import { drawPlaneEvidenceUrl } from "../profile/draw-plane-parity"

test("draw parity imports the measured application module on native Windows and POSIX", () => {
  expect(drawPlaneEvidenceUrl("C:\\owned jobs\\checkout", true)).toBe("/@fs/C:/owned%20jobs/checkout/packages/presentation/rendering/src/draw-lighting-evidence.ts")
  expect(drawPlaneEvidenceUrl("/cache/checkout/", false)).toBe("/@fs//cache/checkout/packages/presentation/rendering/src/skinning-evidence.ts")
})
