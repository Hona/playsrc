import { loadCompositorEvidence, loadMainCpuEvidence, loadCapturePlan } from "./compositor-evidence"
import { RENDER_OWNER_LIMITS, RENDER_OWNER_PLAN, type RenderOwnerEvidence } from "../../../packages/presentation/rendering/src/render-owner-probe"

export function summarizeRenderOwners(evidence: RenderOwnerEvidence) {
  if (evidence.schema !== "playsrc-render-owners-v1" || evidence.plan !== RENDER_OWNER_PLAN
    || !Array.isArray(evidence.frames) || evidence.frames.length > RENDER_OWNER_LIMITS.frames
    || !Array.isArray(evidence.calls) || evidence.calls.length > RENDER_OWNER_LIMITS.calls
    || !Array.isArray(evidence.events) || evidence.events.length > RENDER_OWNER_LIMITS.events
    || !Array.isArray(evidence.identities) || evidence.identities.length > RENDER_OWNER_LIMITS.identities) throw new Error("Invalid bounded render-owner evidence")
  const identities = new Map(evidence.identities.map(i => [i.id, i]))
  const calls = new Map(evidence.calls.map(c => [c.id, c]))
  const owners = new Map<string, { pass: string | null; stage: string; object: number; material: number; calls: number; inclusiveMilliseconds: number }>()
  for (const call of evidence.calls) {
    if (!identities.has(call.renderObject) || !Number.isFinite(call.ended - call.at) || call.ended < call.at) throw new Error("Invalid render-owner call")
    const key = `${call.pass}:${call.stage}:${call.object}:${call.material}`
    let owner = owners.get(key)
    if (!owner) owners.set(key, owner = { pass: call.pass, stage: call.stage, object: call.object, material: call.material, calls: 0, inclusiveMilliseconds: 0 })
    owner.calls++; owner.inclusiveMilliseconds += call.ended - call.at
  }
  const events = new Map<string, { kind: string; pass: string | null; identity: number; dependency: number; updateType: string | null; calls: number; executed: number; trueReturns: number; falseReturns: number; errors: number }>()
  for (const event of evidence.events) {
    const call = calls.get(event.call)
    if (!call || !identities.has(event.identity)) throw new Error("Invalid render-owner event join")
    const key = `${event.kind}:${call.pass}:${event.identity}:${event.updateType}`
    let item = events.get(key)
    if (!item) events.set(key, item = { kind: event.kind, pass: call.pass, identity: event.identity, dependency: event.dependency,
      updateType: event.updateType, calls: 0, executed: 0, trueReturns: 0, falseReturns: 0, errors: 0 })
    item.calls++; item.executed += Number(event.executed === true)
    item.trueReturns += Number(event.outcome === "true"); item.falseReturns += Number(event.outcome === "false"); item.errors += Number(event.outcome === "throw")
  }
  return { complete: evidence.dropped === 0 && evidence.unsupported === 0 && evidence.restored
      && evidence.frames.length === RENDER_OWNER_LIMITS.frames && evidence.frames.every(f => f.complete),
    frames: evidence.frames, dropped: evidence.dropped, unsupported: evidence.unsupported, restored: evidence.restored,
    bookkeepingMilliseconds: evidence.bookkeepingMilliseconds, hookCalls: evidence.hookCalls,
    identities: evidence.identities, owners: [...owners.values()].sort((a,b) => b.inclusiveMilliseconds-a.inclusiveMilliseconds),
    events: [...events.values()],
    limits: "Opt-in sampled frames only. Bookkeeping time is measured probe work, not total causal overhead: dispatch/timer/JIT/GC perturbation is not subtracted. Owner call times include nested probes and are not exclusive CPU. Binding/uniform true/false values are the original comparator results; node true/false values are update return values, NOT value-change evidence. Unchanged comparisons do not prove their dependency checks can be removed. Accessor-backed labels are unknown. Promise identity is preserved, not awaited or timed to settlement." }
}

export async function replayRenderOwners(filename: string) {
  const loaded = await loadCompositorEvidence(filename)
  const main = await loadMainCpuEvidence(filename, loaded)
  const capturePlan = await loadCapturePlan(filename, loaded.manifest)
  const records = loaded.probes.joins.filter(j => j.kind === "render-owners")
  if (capturePlan?.renderOwners !== RENDER_OWNER_PLAN || !records.length) throw new Error("Render owner evidence was not requested/retained")
  return { manifest: filename, source: loaded.manifest.identity.sourceCommit, capturePlan, mainCpu: main?.evidence ?? null,
    traceComplete: loaded.manifest.complete, window: loaded.analysis.window,
    captures: records.map(r => summarizeRenderOwners(r.detail as RenderOwnerEvidence)),
    completedFrames: loaded.probes.joins.filter(j => j.kind === "completed-frame").map(j => j.detail) }
}

if (import.meta.main) console.log(JSON.stringify(await replayRenderOwners(process.argv[2]!), null, 2))
