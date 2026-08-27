import { expect, test } from "bun:test"
import Node from "three/src/nodes/core/Node.js"
import NodeFrame from "three/src/nodes/core/NodeFrame.js"
import NodeUniformsGroup from "three/src/renderers/common/nodes/NodeUniformsGroup.js"
import { NumberNodeUniform } from "three/src/renderers/common/nodes/NodeUniform.js"
import { RenderOwnerProbe, RENDER_OWNER_LIMITS } from "./render-owner-probe"

function fixture() {
  const frame = new NodeFrame()
  frame.frameId = 1; frame.renderId = 1
  const profile = { active: false, currentPass: { identity: "main" }, counters: { completedFrames: 60 } }
  const group = new NodeUniformsGroup("object", { updateType: "object", version: 1 })
  const uniform = new NumberNodeUniform({ name: "lighting", value: 1, type: "float" })
  group.addUniform(uniform)
  const node = new Node()
  node.updateType = "object"
  let executions = 0, gets = 0, references = 0, types = 0
  node.update = function () { executions++; return undefined }
  node.getUpdateType = function () { types++; return this.updateType }
  node.updateReference = function () { references++; return this }
  uniform.getValue = function () { gets++; return this.value }
  const object = { object: { name: "actor", userData: { entity: 9 } }, material: { name: "material" }, camera: {}, context: {} }
  const nodes = { nodeFrame: frame, needsRefresh() { return true }, updateForRender() { frame.updateNode(node) }, updateGroup() { return true } }
  const bindings = { updateForRender() { if (nodes.updateGroup(group)) return group.update() } }
  const renderer = { _nodes: nodes, _bindings: bindings }
  return { frame, profile, group, uniform, node, object, nodes, bindings, renderer, counts: () => ({ executions, gets, references, types }) }
}

test("observes actual node/dependency/comparator calls without repeating getters; restores descriptors", () => {
  const f = fixture(), original = Object.getOwnPropertyDescriptor(f.node, "update")!
  let getterReads = 0
  Object.defineProperty(f.node, "type", { get() { getterReads++; throw Error("must not evaluate") } })
  const probe = new RenderOwnerProbe(f.renderer, f.profile)
  f.nodes.updateForRender(f.object); f.bindings.updateForRender(f.object)
  expect(probe.evidence.calls).toEqual([])
  f.profile.active = true
  for (let index = 0; index < 2; index++) {
    probe.begin(3, 1)
    expect(f.nodes.needsRefresh(f.object)).toBe(true)
    f.nodes.updateForRender(f.object)
    f.bindings.updateForRender(f.object)
    probe.complete(); f.profile.counters.completedFrames++
  }
  expect(getterReads).toBe(0)
  expect(f.counts()).toEqual({ executions: 3, gets: 3, references: 3, types: 3 })
  expect(probe.evidence.events.filter(e => e.kind === "uniform-value").map(e => e.outcome)).toEqual(["false", "false"])
  expect(probe.evidence.events.filter(e => e.kind === "node").map(e => [e.executed, e.updateType])).toEqual([[true, "object"], [true, "object"]])
  expect(probe.evidence.frames.map(f => f.generation)).toEqual([3, 3])
  expect(probe.evidence.restored).toBe(true)
  expect(Object.getOwnPropertyDescriptor(f.node, "update")).toEqual(original)
  expect(Object.hasOwn(f.group, "update")).toBe(false)
  expect(probe.evidence.unsupported).toBe(0)
})

test("frame/render skip, false retry, changed values and exact thrown error are preserved", () => {
  const f = fixture(), probe = new RenderOwnerProbe(f.renderer, f.profile)
  f.profile.active = true; probe.begin(1, 1)
  f.node.updateType = "frame"
  f.nodes.updateForRender(f.object); f.nodes.updateForRender(f.object)
  expect(f.counts().executions).toBe(1)
  expect(probe.evidence.events.filter(e => e.kind === "node").map(e => e.executed)).toEqual([true, false])
  f.bindings.updateForRender(f.object)
  f.uniform.value = 2; f.bindings.updateForRender(f.object)
  expect(probe.evidence.events.filter(e => e.kind === "uniform-value").map(e => e.outcome)).toEqual(["true", "true"])
  const failure = new Error("original")
  f.node.updateType = "object"
  // A replacement method remains the application's method; the probe doesn't
  // re-run or rewrite it to repair diagnostic coverage.
  f.node.update = () => { throw failure }
  expect(() => f.nodes.updateForRender(f.object)).toThrow(failure)
  expect(probe.evidence.calls.at(-1)?.outcome).toBe("throw")
  probe.dispose()
  expect(probe.evidence.frames[0]?.complete).toBe(false)
})

test("owner return promise identity and rejection are not assimilated", async () => {
  const f = fixture()
  const error = new Error("rejected")
  const promise = Promise.reject(error)
  f.nodes.updateForRender = () => promise
  const probe = new RenderOwnerProbe(f.renderer, f.profile)
  f.profile.active = true; probe.begin(1, 1)
  expect(f.nodes.updateForRender(f.object)).toBe(promise)
  await expect(promise).rejects.toBe(error)
  expect(probe.evidence.calls[0]?.outcome).toBe("other")
  probe.dispose()
})

test("render-scoped node retry and frame change use the original NodeFrame cache", () => {
  const f = fixture()
  let executions = 0
  f.node.updateType = "render"
  f.node.update = () => { executions++; return executions === 1 ? false : undefined }
  const probe = new RenderOwnerProbe(f.renderer, f.profile)
  f.profile.active = true; probe.begin(1, 1)
  for (let i = 0; i < 3; i++) f.nodes.updateForRender(f.object)
  f.frame.renderId++; f.nodes.updateForRender(f.object)
  expect(executions).toBe(3)
  expect(probe.evidence.events.filter(e => e.kind === "node").map(e => [e.updateType, e.executed, e.outcome])).toEqual([
    ["render", true, "false"], ["render", true, "undefined"], ["render", false, "undefined"], ["render", true, "undefined"],
  ])
  probe.dispose()
})

test("early generation teardown restores hooks without claiming a complete frame", () => {
  const f = fixture(), before = f.nodes.updateForRender
  const probe = new RenderOwnerProbe(f.renderer, f.profile)
  f.profile.active = true; probe.begin(8, 2); f.nodes.updateForRender(f.object)
  const replacement = () => true
  f.nodes.needsRefresh = replacement
  probe.dispose()
  expect(f.nodes.updateForRender).toBe(before)
  expect(f.nodes.needsRefresh).toBe(replacement)
  expect(probe.evidence.frames[0]?.complete).toBe(false)
  const size = probe.evidence.calls.length
  probe.begin(9, 2); f.nodes.updateForRender(f.object); probe.complete()
  expect(probe.evidence.calls.length).toBe(size)
})

test("bounded records do not suppress required owner calls", () => {
  const f = fixture()
  let calls = 0
  f.nodes.needsRefresh = () => { calls++; return true }
  const probe = new RenderOwnerProbe(f.renderer, f.profile)
  f.profile.active = true; probe.begin(1, 1)
  for (let i = 0; i < RENDER_OWNER_LIMITS.calls + 10; i++) f.nodes.needsRefresh(f.object)
  expect(calls).toBe(RENDER_OWNER_LIMITS.calls + 10)
  expect(probe.evidence.calls.length).toBe(RENDER_OWNER_LIMITS.calls)
  expect(probe.evidence.dropped).toBe(10)
  probe.dispose(); probe.dispose()
  expect(probe.evidence.restored).toBe(true)
})
