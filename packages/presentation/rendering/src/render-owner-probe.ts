/** Opt-in diagnostics for the pinned Three renderer. Observe existing calls and
 * their results; never run a node, dependency getter or value comparator twice. */
export const RENDER_OWNER_PLAN = "two-frames-after-60-v1" as const
export const RENDER_OWNER_LIMITS = Object.freeze({ frames: 2, calls: 4096, events: 131072, identities: 16384, hooks: 16384 })

type Any = Record<string, any>
type Profile = { active: boolean; currentPass: { identity: string } | null; counters: Record<string, number> }
type Outcome = "true" | "false" | "undefined" | "other" | "throw"
export type RenderOwnerEvidence = {
  schema: "playsrc-render-owners-v1"
  plan: typeof RENDER_OWNER_PLAN
  limits: typeof RENDER_OWNER_LIMITS
  identities: { id: number; kind: string; name: string; type: string; source: string | number | null; node: number }[]
  frames: { frame: number; at: number; ended?: number; generation: number; device: number; complete: boolean; bookkeepingMilliseconds?: number }[]
  calls: { id: number; frame: number; pass: string | null; stage: string; at: number; ended: number; renderObject: number; object: number; material: number; camera: number; context: number; nodeFrame: number | null; nodeRender: number | null; outcome: Outcome }[]
  events: { call: number; kind: string; identity: number; dependency: number; updateType: string | null; version: number | null; outcome: Outcome; executed?: boolean; reference?: number }[]
  dropped: number
  unsupported: number
  bookkeepingMilliseconds: number
  hookCalls: number
  restored: boolean
}

// Inspect only data descriptors. Accessors (including type/value getters) are
// deliberately unknown. No toString, JSON serialization or node method calls.
function data(value: any, key: string): any {
  if (!value || (typeof value !== "object" && typeof value !== "function")) return undefined
  const descriptor = Object.getOwnPropertyDescriptor(value, key)
  return descriptor && "value" in descriptor ? descriptor.value : undefined
}
function method(value: any, key: string): Function | undefined {
  for (let owner = value; owner; owner = Object.getPrototypeOf(owner)) {
    const descriptor = Object.getOwnPropertyDescriptor(owner, key)
    if (descriptor) return "value" in descriptor && typeof descriptor.value === "function" ? descriptor.value : undefined
  }
}
const text = (value: any) => typeof value === "string" ? value.slice(0, 192) : ""
const number = (value: any) => typeof value === "number" && Number.isFinite(value) ? value : null
const outcome = (value: any): Outcome => value === true ? "true" : value === false ? "false" : value === undefined ? "undefined" : "other"

export class RenderOwnerProbe {
  readonly evidence: RenderOwnerEvidence = { schema: "playsrc-render-owners-v1", plan: RENDER_OWNER_PLAN, limits: RENDER_OWNER_LIMITS,
    identities: [], frames: [], calls: [], events: [], dropped: 0, unsupported: 0, bookkeepingMilliseconds: 0, hookCalls: 0, restored: false }
  readonly #ids = new WeakMap<object, number>()
  readonly #hooked = new WeakMap<object, Set<string>>()
  readonly #restore: (() => void)[] = []
  readonly #profile: Profile
  readonly #clock: () => number
  #frame: RenderOwnerEvidence["frames"][number] | undefined
  #call: RenderOwnerEvidence["calls"][number] | undefined
  #node: RenderOwnerEvidence["events"][number] | undefined
  #nodeIdentity = 0
  #closed = false
  #frameBookkeeping = 0
  #install?: () => void

  constructor(renderer: Any, profile: Profile, clock = () => performance.now()) {
    this.#profile = profile
    this.#clock = clock
    this.#install = () => {
    const nodes = data(renderer, "_nodes"), bindings = data(renderer, "_bindings")
    const nodeFrame = data(nodes, "nodeFrame")
    const wrapOwner = (owner: Any, name: string, stage: string) => this.#hook(owner, name, (original, self, args) => {
      if (!this.#frame || !profile.active) return original.apply(self, args)
      const previous = this.#call
      let call: RenderOwnerEvidence["calls"][number] | undefined
      this.#measure(() => {
        if (this.evidence.calls.length === RENDER_OWNER_LIMITS.calls) { this.evidence.dropped++; return }
        const object = args[0]
        call = { id: this.evidence.calls.length + 1, frame: this.#frame!.frame, pass: profile.currentPass?.identity ?? null,
          stage, at: this.#clock(), ended: 0, renderObject: this.#id(object, "render-object"), object: this.#id(data(object, "object"), "object"),
          material: this.#id(data(object, "material"), "material"), camera: this.#id(data(object, "camera"), "camera"), context: this.#id(data(object, "context"), "context"),
          nodeFrame: number(data(nodeFrame, "frameId")), nodeRender: number(data(nodeFrame, "renderId")), outcome: "throw" }
        this.evidence.calls.push(call)
        this.#call = call
      })
      try { const value = original.apply(self, args); if (call) call.outcome = outcome(value); return value }
      finally { this.#measure(() => { if (call) call.ended = this.#clock(); this.#call = previous }) }
    })
    wrapOwner(nodes, "updateForRender", "nodes.updateForRender")
    wrapOwner(bindings, "updateForRender", "bindings.updateForRender")
    wrapOwner(nodes, "needsRefresh", "nodes.needsRefresh")
    this.#hook(nodeFrame, "updateNode", (original, self, args) => {
      if (!this.#call) return original.apply(self, args)
      const node = args[0], previous = this.#node, previousIdentity = this.#nodeIdentity
      this.#measure(() => {
        this.#nodeIdentity = this.#id(node, "node")
        this.#node = this.#event("node", this.#nodeIdentity, 0, null, null)
        if (this.#node) this.#node.executed = false
        this.#hook(node, "getUpdateType", (fn, receiver, values) => {
          const result = fn.apply(receiver, values)
          if (this.#node && this.#nodeIdentity === this.#idKnown(receiver)) this.#measure(() => { this.#node!.updateType = text(result) || null })
          return result
        })
        this.#hook(node, "updateReference", (fn, receiver, values) => {
          const result = fn.apply(receiver, values)
          if (this.#node && this.#nodeIdentity === this.#idKnown(receiver)) this.#measure(() => { this.#node!.reference = this.#id(result, "reference") })
          return result
        })
        this.#hook(node, "update", (fn, receiver, values) => {
          const event = this.#node && this.#nodeIdentity === this.#idKnown(receiver) ? this.#node : undefined
          if (event) event.executed = true
          const result = fn.apply(receiver, values)
          if (event) event.outcome = outcome(result)
          return result
        })
      })
      try {
        const result = original.apply(self, args)
        if (this.#node && !this.#node.executed) this.#node.outcome = outcome(result)
        return result
      }
      finally { this.#measure(() => { this.#node = previous; this.#nodeIdentity = previousIdentity }) }
    })
    this.#hook(nodes, "updateGroup", (original, self, args) => {
      if (!this.#call) return original.apply(self, args)
      const binding = args[0]
      let event: RenderOwnerEvidence["events"][number] | undefined
      this.#measure(() => {
        const group = data(binding, "groupNode")
        event = this.#event("group-dependency", this.#id(binding, "binding"), this.#id(group, "group-node"), text(data(group, "updateType")) || null, number(data(group, "version")))
        if (method(binding, "update")) this.#hook(binding, "update", (fn, receiver, values) => {
          if (!this.#call) return fn.apply(receiver, values)
          let update: RenderOwnerEvidence["events"][number] | undefined
          this.#measure(() => { update = this.#event("binding-value", this.#id(receiver, "binding"), 0, null, null) })
          const result = fn.apply(receiver, values)
          if (update) update.outcome = outcome(result)
          return result
        })
        // UniformsGroup's boolean is its existing component-wise comparison,
        // not an inferred comparison from JS object reference equality.
        if (method(binding, "updateByType")) this.#hook(binding, "updateByType", (fn, receiver, values) => {
          if (!this.#call) return fn.apply(receiver, values)
          let update: RenderOwnerEvidence["events"][number] | undefined
          this.#measure(() => { update = this.#event("uniform-value", this.#id(values[0], "uniform"), this.#id(data(data(values[0], "nodeUniform"), "node"), "node"), null, null) })
          const result = fn.apply(receiver, values)
          if (update) update.outcome = outcome(result)
          return result
        })
      })
      const result = original.apply(self, args)
      if (event) event.outcome = outcome(result)
      return result
    })
    }
  }

  begin(generation: number, device: number): void {
    if (this.#closed || !this.#profile.active || this.#frame) return
    const frame = this.#profile.counters.completedFrames ?? 0
    if (frame < 60 || this.evidence.frames.length >= RENDER_OWNER_LIMITS.frames) return
    this.#measure(() => {
      this.#frameBookkeeping = this.evidence.bookkeepingMilliseconds
      this.#install?.()
      this.#install = undefined
      this.#frame = { frame, generation, device, at: this.#clock(), complete: false }
      this.evidence.frames.push(this.#frame)
    })
  }

  complete(): void {
    if (!this.#frame) return
    this.#measure(() => {
      this.#frame!.ended = this.#clock(); this.#frame!.complete = true
      this.#frame!.bookkeepingMilliseconds = this.evidence.bookkeepingMilliseconds - this.#frameBookkeeping
      this.#frame = undefined
    })
    if (this.evidence.frames.length >= RENDER_OWNER_LIMITS.frames) this.dispose()
  }

  dispose(): void {
    if (this.#closed) return
    this.#measure(() => { for (const restore of this.#restore.reverse()) restore(); this.#restore.length = 0 })
    this.#closed = true
    this.#install = undefined
    this.#frame = undefined
    this.#call = undefined
    this.#node = undefined
    this.evidence.restored = true
  }

  #measure(callback: () => void): void {
    const at = this.#clock()
    try { callback() } finally { this.evidence.bookkeepingMilliseconds += this.#clock() - at }
  }

  #idKnown(value: any): number { return value && (typeof value === "object" || typeof value === "function") ? this.#ids.get(value) ?? 0 : 0 }

  #id(value: any, kind: string): number {
    if (!value || (typeof value !== "object" && typeof value !== "function")) return 0
    const known = this.#ids.get(value)
    if (known) return known
    if (this.evidence.identities.length === RENDER_OWNER_LIMITS.identities) { this.evidence.dropped++; return 0 }
    const id = this.evidence.identities.length + 1, userData = data(value, "userData")
    const source = data(userData, "materialIdentity") ?? data(userData, "entity") ?? data(userData, "model") ?? data(value, "property")
    const record = { id, kind, name: text(data(value, "name")), type: text(data(value, "type")) || text(data(data(Object.getPrototypeOf(value), "constructor"), "name")),
      source: typeof source === "string" ? text(source) : number(source), node: 0 }
    this.evidence.identities.push(record)
    this.#ids.set(value, id)
    record.node = this.#id(data(value, "node"), "node")
    return id
  }

  #event(kind: string, identity: number, dependency: number, updateType: string | null, version: number | null) {
    if (this.evidence.events.length === RENDER_OWNER_LIMITS.events) { this.evidence.dropped++; return undefined }
    const event: RenderOwnerEvidence["events"][number] = { call: this.#call!.id, kind, identity, dependency, updateType, version, outcome: "throw" }
    this.evidence.events.push(event)
    return event
  }

  #hook(owner: any, name: string, call: (original: Function, self: any, args: any[]) => any): void {
    if (!owner || this.#hooked.get(owner)?.has(name)) return
    const original = method(owner, name), descriptor = Object.getOwnPropertyDescriptor(owner, name)
    if (!original || (descriptor && !descriptor.configurable) || !Object.isExtensible(owner)) { this.evidence.unsupported++; return }
    if (this.#restore.length === RENDER_OWNER_LIMITS.hooks) { this.evidence.dropped++; return }
    let methods = this.#hooked.get(owner)
    if (!methods) this.#hooked.set(owner, methods = new Set())
    methods.add(name)
    const probe = this
    const wrapped = function (this: any, ...args: any[]) {
      probe.evidence.hookCalls++
      return call(original, this, args)
    }
    Object.defineProperty(owner, name, { configurable: true, writable: true, value: wrapped })
    this.#restore.push(() => {
      if (Object.getOwnPropertyDescriptor(owner, name)?.value !== wrapped) return
      if (descriptor) Object.defineProperty(owner, name, descriptor); else delete owner[name]
    })
  }
}
