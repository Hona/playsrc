import { expect, test } from "bun:test"
import { classPipelinePoseRequests } from "../src/class-pipeline-preparation"
import { tf2ClassPresentation } from "../src/class"
import { encodeModelPoseBatch } from "../src/presentation"
import type { PresentationArtifacts, ModelArtifact } from "../src/artifacts"
import type { Tf2Class } from "../src/codec"

const camera = { position: [10, 20, 30] as const, yawDegrees: 40, pitchDegrees: 5, far: 32768 }
function artifacts() {
  const models = new Map<string, ModelArtifact>()
  const add = (model: string, profile: "world" | "viewmodel", skins = 2) => models.set(model, {
    profile, skinCount: skins, bodygroupCounts: [2, 3], sequences: [{ label: "reference" }, ...["PRIMARY", "SECONDARY", "MELEE"].map(role => ({ activity: `ACT_MP_STAND_${role}` }))],
  } as ModelArtifact)
  for (let identity = 1; identity <= 9; identity++) add(tf2ClassPresentation(identity as Tf2Class).model, "world")
  add("models/weapons/c_models/c_bottle/c_bottle.mdl", "viewmodel", 1)
  add("models/weapons/c_models/c_medic_arms.mdl", "viewmodel")
  add("models/props/unrelated.mdl", "world")
  return { models } as PresentationArtifacts
}

test("prepares exact declared class previews and resident viewmodels without commands or clock advancement", () => {
  const prepared = classPipelinePoseRequests(artifacts(), 1, camera, 16 / 9)
  expect(prepared.filter(value => value.pass === "panel")).toHaveLength(9)
  expect(prepared.filter(value => value.pass === "view")).toHaveLength(2)
  expect(prepared.filter(value => value.pass === "world")).toHaveLength(18)
  expect(new Set(prepared.map(value => value.request.identity)).size).toBe(prepared.length)
  expect(prepared.find(value => value.request.model.includes("bottle"))!.request.skin).toBe(0)
  expect(prepared.find(value => value.request.model.includes("medic_arms"))!.request.skin).toBe(1)
  expect(prepared.some(value => value.request.model.includes("unrelated"))).toBe(false)
  for (const { request } of prepared) {
    expect(request.currentTimeSeconds).toBe(0)
    expect(request.frameTimeSeconds).toBe(0)
    expect(request.elapsedSeconds).toBe(0)
    expect(request.bodygroups).toEqual([0, 0])
  }
  expect(encodeModelPoseBatch(prepared.map(value => value.request)).byteLength).toBeGreaterThan(0)
})

test("missing class facts and a larger-than-resident resource set fail before preparation", () => {
  const values = artifacts()
  const models = values.models as Map<string, ModelArtifact>
  models.delete(tf2ClassPresentation(3).model)
  expect(() => classPipelinePoseRequests(values, 0, camera, 1)).toThrow("unavailable")
  const full = artifacts()
  for (let i = 0; i < 96; i++) (full.models as Map<string, ModelArtifact>).set(`models/test${i}.mdl`, {
    profile: "viewmodel", skinCount: 1, bodygroupCounts: [], sequences: [{ label: "reference" }],
  } as ModelArtifact)
  expect(() => classPipelinePoseRequests(full, 0, camera, 1)).toThrow("bound")
})
