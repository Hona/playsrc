import { expect, test } from "bun:test"
import { RenderOwnerProbe } from "../../../packages/presentation/rendering/src/render-owner-probe"
import { summarizeRenderOwners } from "../profile/render-owner-evidence"

test("replay retains bounded identity joins, probe time and incomplete evidence", () => {
  const profile = { active: true, currentPass: { identity: "main" }, counters: { completedFrames: 60 } }
  const renderer = { _nodes: { nodeFrame: { updateNode() {} }, needsRefresh() { return true }, updateForRender() {}, updateGroup() {} }, _bindings: { updateForRender() {} } }
  let now = 0
  const probe = new RenderOwnerProbe(renderer, profile, () => ++now)
  const object = { object: {}, material: {}, context: {}, camera: {} }
  for (let frame = 0; frame < 2; frame++) {
    probe.begin(2, 1); renderer._nodes.updateForRender(object); probe.complete(); profile.counters.completedFrames++
  }
  const result = summarizeRenderOwners(probe.evidence)
  expect(result.complete).toBe(true)
  expect(result.bookkeepingMilliseconds).toBeGreaterThan(0)
  expect(result.owners[0]?.calls).toBe(2)
  expect(result.owners[0]?.pass).toBe("main")
  expect(result.limits).toContain("not total causal overhead")
  const incomplete = structuredClone(probe.evidence)
  incomplete.dropped++
  expect(summarizeRenderOwners(incomplete).complete).toBe(false)
  incomplete.calls[0]!.renderObject = 9999
  expect(() => summarizeRenderOwners(incomplete)).toThrow("Invalid render-owner call")
})

test("reference transitions distinguish new draw owners from repeated frame/pass work", () => {
  const profile = { active: true, currentPass: { identity: "main" }, counters: { completedFrames: 60 } }
  const a = {}, b = {}
  let reference = a
  const node = { type: "ReferenceNode", getUpdateType() { return "object" }, updateReference() { return reference }, update() {} }
  const frame = { updateNode(n: typeof node) { n.getUpdateType(); n.updateReference(); n.update() } }
  const nodes = { nodeFrame: frame, needsRefresh() {}, updateForRender() { frame.updateNode(node) }, updateGroup() {} }
  const probe = new RenderOwnerProbe({ _nodes: nodes, _bindings: { updateForRender() {} } }, profile)
  const object = { object: {}, material: {}, context: {}, camera: {} }
  probe.begin(1, 1)
  nodes.updateForRender(object); reference = b; nodes.updateForRender(object); nodes.updateForRender(object)
  probe.complete(); profile.counters.completedFrames++
  probe.begin(1, 1); nodes.updateForRender(object); probe.complete()
  expect(summarizeRenderOwners(probe.evidence).nodeScopes["ReferenceNode:object"]).toEqual({
    executions: 4, firstReference: 1, changedReference: 1, sameReference: 2, repeatedFramePassReference: 1,
  })
})
