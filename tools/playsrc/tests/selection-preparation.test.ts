import { expect, test } from "bun:test"
import { selectionPreparation } from "../profile/selection-comparison"

test("preparation attribution distinguishes sequential ownership from overlapping native readiness", () => {
  const measurement = { evidence: { owners: [{ kind: "pipeline-start", at: 10 }, { kind: "visible-pipelines-ready", at: 100 }],
    gpuOperations: [{ kind: "createRenderPipelineAsync", at: 20, end: 50 }, { kind: "createRenderPipelineAsync", at: 30, end: 60 },
      { kind: "writeTexture", at: 35, end: 40 }, { kind: "createRenderPipelineAsync", at: 70, end: 80 }] } }
  expect(selectionPreparation(measurement, "visible-world")).toEqual({ owner: "visible-world", wallMilliseconds: 90, nativePipelines: 3,
    maximumNativeInFlight: 2, summedNativeMilliseconds: 70, nativeCoveredMilliseconds: 50 })
  expect(() => selectionPreparation(measurement, "model")).toThrow("Complete model")
  measurement.evidence.gpuOperations[0]!.end = 101
  expect(() => selectionPreparation(measurement, "visible-world")).toThrow("before recorded native readiness")
})
