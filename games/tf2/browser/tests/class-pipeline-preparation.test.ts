import { expect, test } from "bun:test"
import { classPipelinePoseRequests } from "../src/class-pipeline-preparation"
import { tf2ClassPresentation } from "../src/class"
import { encodeModelPoseBatch } from "../src/presentation"
import type { PresentationArtifacts, ModelArtifact } from "../src/artifacts"
import type { Tf2Class } from "../src/codec"
import { equipmentPipelinePoseRequests } from "../src/equipment/pipelines"

const camera = { position: [10, 20, 30] as const, yawDegrees: 40, pitchDegrees: 5, far: 32768 }

test("incremental equipment prepares each admitted skin once without requiring unrelated classes", () => {
  const models = new Map<string, ModelArtifact>([["models/player/soldier.mdl", {
    profile: "world", skinCount: 4, bodygroupCounts: [2], sequences: [{ label: "ref" }],
  } as ModelArtifact]])
  const geometry = ["models/player/soldier.mdl", "models/player/soldier.mdl#skin=1"].map(logicalPath => ({ logicalPath, materials: [], primitives: [] }))
  const requests = equipmentPipelinePoseRequests({ models, geometry }, 1.6)
  expect(requests.map(request => request.skin)).toEqual([0, 1])
  expect(new Set(requests.map(request => request.identity)).size).toBe(2)
  for (const request of requests) {
    expect(request.preparation).toBe(true)
    expect(request.modelPanel).toBe(true)
    expect(request.frameTimeSeconds).toBe(0)
    expect(request.bodygroups).toEqual([0])
    expect(request.lighting).toBeDefined()
  }
  expect(encodeModelPoseBatch(requests).byteLength).toBeGreaterThan(0)
})
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
  const prepared = classPipelinePoseRequests(artifacts(), 1, camera, 16 / 9, [])
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
  expect(() => classPipelinePoseRequests(values, 0, camera, 1, [])).toThrow("unavailable")
  const full = artifacts()
  for (let i = 0; i < 96; i++) (full.models as Map<string, ModelArtifact>).set(`models/test${i}.mdl`, {
    profile: "viewmodel", skinCount: 1, bodygroupCounts: [], sequences: [{ label: "reference" }],
  } as ModelArtifact)
  expect(() => classPipelinePoseRequests(full, 0, camera, 1, [])).toThrow("bound")
})

test("prepares only registered eligible wearables without creating effects", () => {
  const item = { itemId: 379, definitionIndex: 378, quality: 5, style: 0, slot: 7, attributes: [{ definition: 134, value: 13 }] }
  const requests = classPipelinePoseRequests(artifacts(), 0, camera, 16 / 9, [{ item, name: "", displayName: "", description: [], image: "", modelPlayer: "", attachToHands: false,
    animationReplacements: [], soundOverrides: [], deathNoticeIcon: null, weapon: null, classSlots: [{ class: 3, slot: 7, weapon: null }, { class: 5, slot: 7, weapon: null }, { class: 6, slot: 7, weapon: null }] }])
  expect(requests.filter(entry => entry.request.equippedItems?.length)).toHaveLength(9)
  expect(requests.every(entry => entry.request.preparation)).toBe(true)
})
