import { expect, test } from "bun:test"
import { mapPropPipelinePoseRequests } from "../src/map-prop-pipeline-preparation"
import { encodeModelPoseBatch } from "../src/presentation"
import type { PresentationArtifacts } from "../src/artifacts"
import type { Snapshot } from "../src/codec"

const camera = { position: [10, 20, 30] as const, yawDegrees: 40, pitchDegrees: 5, far: 32768 }
const artifacts = { modelOccurrences: [1, 2, 3, 4].map(entity => ({ entity, model: entity === 4 ? "unrelated" : "door", skin: entity === 3 ? 1 : 0, body: 2 })) } as PresentationArtifacts
const snapshot = { tick: 200n, entityPresentation: {
  studioAnimations: [1, 2, 3].map(sourceIndex => ({ sourceIndex, sequence: "open", elapsedSeconds: 0.25 })),
  studioModels: [1, 2, 3, 4].map(sourceIndex => ({ sourceIndex, worldPosition: [1, 2, 3], worldAngles: [0, 90, 0], draw: false })),
}, regenerateAnimationEvents: [] } as unknown as Snapshot

test("only authority-selected map animations supply posed pipeline variants, deduplicated by model and skin", () => {
  const before = structuredClone(snapshot)
  const requests = mapPropPipelinePoseRequests(artifacts, snapshot, camera, 1.5)
  expect(requests.map(value => value.request.identity)).toEqual([1, 3])
  expect(requests.every(value => value.pass === "world")).toBe(true)
  for (const { request } of requests) {
    expect(request.elapsedSeconds).toBe(0.25)
    expect(request.previousElapsedSeconds).toBe(0.25)
    expect(request.frameTimeSeconds).toBe(0)
    expect(request.lighting!.origin).toEqual([1, 2, 3])
    expect(request.packedBody).toBe(2)
  }
  expect(encodeModelPoseBatch(requests.map(value => value.request)).length).toBeGreaterThan(0)
  expect(snapshot).toEqual(before)
})

test("resupply uses its exact event body and open/close phase without dispatching inputs", () => {
  const value = { ...snapshot, regenerateAnimationEvents: [{ associatedModel: 1, openTick: 100n, closeTick: 150n, body: 7, openAnimation: "open", closeAnimation: "close" }] } as Snapshot
  const request = mapPropPipelinePoseRequests(artifacts, value, camera, 1)[0]!.request
  expect(request.activity).toBe("close")
  expect(request.elapsedSeconds).toBe(0.75)
  expect(request.packedBody).toBe(7)
  expect(() => mapPropPipelinePoseRequests({ modelOccurrences: [] } as unknown as PresentationArtifacts, value, camera, 1)).toThrow("unavailable")
})

test("an untouched authored regenerate association prepares without fabricating a contact event", () => {
  const values = { ...artifacts, modelOccurrences: artifacts.modelOccurrences.map(value => ({ ...value, pipelineAnimation: value.entity === 4 ? "open" : null })) }
  const prepared = mapPropPipelinePoseRequests(values, snapshot, camera, 1)
  const locker = prepared.find(value => value.request.identity === 4)!.request
  expect(locker.activity).toBe("open")
  expect(locker.elapsedSeconds).toBe(0)
  expect(locker.frameTimeSeconds).toBe(0)
  expect(locker.packedBody).toBe(2)
  expect(snapshot.regenerateAnimationEvents).toEqual([])
})
