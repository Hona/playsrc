# Generic Entity Behavior Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

| Field | Current value |
|---|---|
| Authority identity | Valve Source SDK 2013 base entity, map-entity, entity-list, handle, input, output, event-queue, hierarchy, template, filter, trigger, generic logic, and save/restore contracts; playsrc Owner Registry. |
| Authority revision | Source SDK commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; reviewed playsrc commit Missing. |
| Generator command | Missing. |
| Output path | `packages/world/entity/inventories/generic-behaviors.md` |
| Item count | 72 candidate items; 0 accepted items. |

Every item is owned by Entity. `Required seam` names a producer or consumer and never transfers behavior ownership. This inventory contains behavior identities, not game classnames.

## Representation

| Stable identity | Entity-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `representation.entity-block-syntax` | Decode flat brace-delimited entity blocks, Source whitespace, `//` comments, quoted tokens, unquoted tokens, and one optional final NUL; reject malformed structure at the exact byte. | BSP supplies the exact lump byte range. | Unsupported |
| `representation.ordered-keyvalues` | Retain every key/value token byte sequence, quote form, source span, repeated key, and source order. | Class bindings consume retained pairs. | Unsupported |
| `representation.definition-identity` | Assign each definition its immutable source ordinal and map provenance without deriving identity from classname or targetname. | Map retains the owning map identity. | Unsupported |
| `representation.classname` | Require and retain the first semantic `classname` while preserving every raw occurrence. | The selected game supplies one class binding. | Unsupported |
| `representation.targetname` | Retain the optional initial `targetname`, expose the current mutable targetname, and preserve its source pair separately. | I/O and game handlers mutate through Entity inputs. | Unsupported |
| `representation.typed-projection` | Project retained strings into declared void, boolean, signed-32, binary32, string, vector, position-vector, RGBA8, or handle values with exact diagnostics. | Class bindings declare field types. | Unsupported |

## References And Handles

| Stable identity | Entity-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `reference.slot-generation-handle` | Identify a live entity by slot plus generation and reject stale generations after slot reuse. | Networking maps handles to its wire encoding. | Unsupported |
| `reference.live-creation-order` | Iterate live entities in creation order independent of slot reuse and targetname changes. | Simulation consumes ordered live sets. | Unsupported |
| `reference.direct-handle` | Resolve a direct handle to one live entity or `Missing` without name or class fallback. | Inputs, games, and runtime packages supply handles. | Unsupported |
| `reference.targetname-query` | Resolve exact ASCII-insensitive targetnames and one trailing-`*` prefix wildcard to all live matches in creation order. | I/O supplies target expressions. | Unsupported |
| `reference.context-selector` | Resolve `!self`, `!caller`, and `!activator` from dispatch context and invoke one registered external resolver for other procedural selectors. | Game, visibility, or picker owners supply external selector results. | Unsupported |
| `reference.classname-fallback` | Search classnames only when a targetname query returns zero matches; return an ordered empty result plus diagnostic when both searches fail. | The selected game supplies classname bindings. | Unsupported |

## Lifecycle

| Stable identity | Entity-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `lifecycle.classified-construction` | Construct only definitions with an accepted binding and retain `Intentionally inert`, `Unsupported`, `Unknown`, and `Malformed` definitions without guessed behavior. | The selected game supplies the complete binding registry. | Unsupported |
| `lifecycle.world-first` | Construct and spawn the world definition before every non-world definition and reject a missing or duplicate world contract. | Map identifies the world definition. | Unsupported |
| `lifecycle.parent-aware-spawn` | Spawn parents before descendants using hierarchy depth, binding spawn priority, and source ordinal as total ordering keys. | Class bindings supply integer spawn priority. | Unsupported |
| `lifecycle.activation-barrier` | Activate each successful spawn exactly once only after the complete construction batch has reached spawned or failed state. | Simulation invokes activation as one batch. | Unsupported |
| `lifecycle.deferred-removal` | Mark removal synchronously, remove lookup participation, notify while readable, and release storage at the Entity phase boundary. | Simulation supplies the phase boundary. | Unsupported |
| `lifecycle.descendant-removal` | Mark remaining descendants for removal, support explicit child-first hierarchy kill, and make repeated removal inert. | Game handlers request kill operations. | Unsupported |

## Hierarchy

| Stable identity | Entity-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `hierarchy.parent-request` | Parse one parent target expression and optional attachment identity while retaining the original keyvalue. | Class bindings identify the parent field. | Unsupported |
| `hierarchy.parent-resolution` | Select the first creation-order parent match, diagnose additional matches, and leave an unresolved request classified `Missing`. | Target resolution supplies candidate handles. | Unsupported |
| `hierarchy.acyclic-forest` | Reject self-parenting, descendant-parenting, and preexisting cycles before changing links. | No external owner. | Unsupported |
| `hierarchy.reparent-preserve-world` | Derive a new local transform so ordinary reparenting preserves world origin and orientation. | Math uses the canonical world transform representation. | Unsupported |
| `hierarchy.detach-preserve-world` | Remove a parent link while preserving world origin and orientation and updating parent/child iteration order. | No external owner. | Unsupported |
| `hierarchy.attachment-parent` | Defer attachment setup until parent spawn, support snap and maintain-offset modes, and compose supplied attachment transforms through descendants. | The authoritative pose owner supplies attachment transforms. | Unsupported |

## Inputs And Values

| Stable identity | Entity-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `input.ascii-insensitive-lookup` | Select one declared input or writable input field by ASCII-insensitive identity across a composed binding. | Class bindings declare inputs. | Unsupported |
| `input.dispatch-context` | Preserve target, parameter, activator, caller, output-action identity, tick, and producer sequence through synchronous dispatch. | I/O, games, and runtime packages produce dispatches. | Unsupported |
| `input.handler-dispatch` | Invoke one selected handler synchronously and return its accepted or rejected result before the next target dispatch. | Generic primitives and game handlers implement selected operations. | Unsupported |
| `input.field-assignment` | Assign one declared writable field after successful conversion and emit one state transition. | Class bindings declare writable fields. | Unsupported |
| `input.variant-conversion` | Apply the complete accepted conversion matrix without implicit conversion outside that matrix. | Target resolution supplies string-to-handle lookup. | Unsupported |
| `input.unknown-or-bad-parameter` | Reject an unknown input or failed conversion without handler execution or state mutation and retain an exact diagnostic. | Diagnostics are consumed by tools and evidence. | Unsupported |

## Output Actions

| Stable identity | Entity-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `output.action-delimiter` | Select the escape-byte delimiter when present and comma otherwise, then decode exactly five bounded fields. | Entity-lump parsing supplies source value bytes and span. | Unsupported |
| `output.action-defaults` | Require nonempty target; default empty input to `Use`, empty parameter to inherited output value, empty delay to zero, and empty count, `0`, or `-1` to always. | Class bindings declare outputs. | Unsupported |
| `output.action-malformed` | Classify bad field count, empty target where prohibited, non-finite delay, invalid count, and overlong fields as unschedulable `Malformed` actions. | Tools consume diagnostics. | Unsupported |
| `output.action-identity` | Assign one stable monotonically ordered identity to each parsed action and preserve its declaration ordinal. | Snapshots retain the next identity. | Unsupported |
| `output.reverse-declaration-fire` | Emit matching actions in reverse source declaration order without reordering by target or input. | The event queue receives emitted actions. | Unsupported |
| `output.finite-fire-state` | Snapshot inherited or override value and context, consume finite count at emission, and remove the action exactly when count reaches zero. | Snapshots retain remaining actions and counts. | Unsupported |

## Event Queue

| Stable identity | Entity-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `queue.due-time` | Compute due simulation time from current time and delay and dispatch on the first Entity phase at or after that time. | Simulation supplies tick and interval. | Unsupported |
| `queue.equal-time-fifo` | Order by due time and enqueue sequence so equal-time events dispatch FIFO. | No external owner. | Unsupported |
| `queue.same-phase-reentrancy` | Drain newly enqueued events already due in the current phase before advancing to a later phase. | Generic and game handlers may emit nested events. | Unsupported |
| `queue.target-dispatch` | Dispatch to every ordered name match, otherwise every ordered classname match, or one live direct handle. | Target resolution supplies matches. | Unsupported |
| `queue.cancellation-and-pending` | Cancel caller-owned events by live caller handle plus name/class match, or direct-target events by case-sensitive input prefix; report pending state with the same direct-target prefix rule. | Generic handlers invoke cancellation. | Unsupported |
| `queue.dispatch-failure` | Consume missing-target and unknown-input events, emit bounded diagnostics, and continue with later events. | Tools and evidence consume diagnostics. | Unsupported |

## Templates

| Stable identity | Entity-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `template.member-selection` | Resolve up to 16 ordered member target expressions and retain every creation-order match. | Target resolution supplies member handles. | Unsupported |
| `template.definition-capture` | Capture each member's complete entity definition and transform relative to the template without dropping repeated keyvalues or actions. | Map supplies immutable definitions. | Unsupported |
| `template.prototype-lifecycle` | Remove captured prototypes by default or retain them only when the selected template contract explicitly requests preservation. | Simulation observes lifecycle events. | Unsupported |
| `template.name-fixup` | Assign one deterministic instance suffix to member targetnames and every declared internal reference unless names are preserved. | Class bindings declare reference-valued fields. | Unsupported |
| `template.instance-spawn` | Construct all accepted members at the requested template transform through the normal hierarchy, spawn, and activation pipeline. | The selected game supplies member bindings. | Unsupported |
| `template.instance-result` | Return the ordered created handles, emit one completion output after successful construction, and fail without a partially authoritative instance result. | Callers consume handles and output. | Unsupported |

## Filters

| Stable identity | Entity-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `filter.predicate-contract` | Evaluate one immutable caller/subject view through one declared generic predicate. | Games and runtime packages supply subject fields. | Unsupported |
| `filter.negation` | Invert exactly the completed child predicate result after evaluation. | No external owner. | Unsupported |
| `filter.name-and-class` | Match subject targetname or classname using Entity's exact/wildcard matcher. | Subject views supply current names. | Unsupported |
| `filter.and-or-composition` | Evaluate up to five ordered child filters with AND or OR short-circuit semantics and defined empty-list results. | Target resolution supplies child filters. | Unsupported |
| `filter.invalid-graph` | Fail closed on missing, wrong-kind, cyclic, or over-depth child references and retain the failing edge. | Limits supply maximum filter depth. | Unsupported |
| `filter.test-activator-output` | Test the supplied activator and emit exactly one pass or fail output with that activator and the filter as caller. | I/O invokes and consumes the result. | Unsupported |

## Trigger Contacts

| Stable identity | Entity-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `trigger.contact-input` | Consume ordered start, stay, and end contact records addressed by trigger and subject handles. | Collision produces contact records and sequence. | Unsupported |
| `trigger.enable-and-filter-gate` | Accept starts only while enabled and after the selected filter passes; reject failed starts without contact state. | Filters evaluate; Collision owns geometry. | Unsupported |
| `trigger.unique-contact-set` | Retain one live contact entry per subject, remove stale handles deterministically, and serialize the ordered set. | Entity handles identify subjects. | Unsupported |
| `trigger.start-outputs` | Fire per-contact start for each accepted start record and start-all only on empty-to-nonempty transition. | I/O consumes outputs. | Unsupported |
| `trigger.end-outputs` | Fire end only for a recorded contact and end-all only on nonempty-to-empty transition. | I/O consumes outputs. | Unsupported |
| `trigger.control-inputs` | Enable accepts starts; disable rejects starts while collision ends complete tracked contacts; disable-and-end-touch flushes contacts in reverse insertion order; touch-test emits touching or not-touching only while enabled. | Collision observes enabled state; I/O invokes controls. | Unsupported |

## Generic Logic Primitives

| Stable identity | Entity-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `logic.relay` | Own relay enabled state, spawn output, trigger forwarding, remove-on-fire, retrigger lock, fast retrigger, and caller-owned cancellation. | I/O supplies actions and due times. | Unsupported |
| `logic.timer` | Own fixed/random interval, enabled state, next due time, regular or alternating output, timer adjustment inputs, and RNG state. | Simulation supplies time; deterministic RNG supplies draws. | Unsupported |
| `logic.counter` | Own binary32 value, min/max, hit latches, arithmetic inputs, clamping, polling, and ordered threshold/value outputs. | I/O supplies operands and consumes outputs. | Unsupported |
| `logic.case` | Own 16 ordered string cases, first-match/default behavior, connected-case random choice, shuffle batch, last choice, and RNG state. | Deterministic RNG supplies draws. | Unsupported |
| `logic.enable-disable-toggle` | Provide one reusable enabled-state transition primitive with explicit idempotent enable/disable and state-inverting toggle. | Generic compositions and game handlers consume it. | Unsupported |
| `logic.external-effect-request` | Emit an ordered runtime-neutral request and accept an ordered completion/failure input without implementing movement, physics, damage, rendering, or game rules. | The owning runtime or game module executes the effect. | Unsupported |

## Runtime State, Ordering, And Integration

| Stable identity | Entity-owned observable contract | Required seam | Current coverage |
|---|---|---|---|
| `runtime.complete-snapshot` | Serialize every Entity-owned identity, ordering key, mutable state value, reference, action, queued event, template, filter, contact, logic, and RNG field. | Replay and Networking may transport but not redefine snapshots. | Unsupported |
| `runtime.atomic-restore` | Validate source and registry identities and restore the complete snapshot or leave the prior world unchanged. | Content and the selected game supply matching identities. | Unsupported |
| `runtime.entity-phase-order` | Apply external records, advance due generic primitives, drain due I/O, and commit deferred removal in one deterministic Entity phase. | Simulation invokes the phase once per simulation tick. | Unsupported |
| `runtime.coverage-classification` | Assign every entity, field, reference, action, input, contact, and class binding exactly one technical coverage classification. | Tools and game roadmaps consume coverage. | Unsupported |
| `runtime.bounded-failure` | Enforce fixed and caller limits before allocation or mutation and retain unprocessed authoritative state on `BoundExceeded`. | Callers supply the required limit record. | Unsupported |
| `runtime.sole-authority-integration` | Expose one current Entity interface to every producer and consumer and reject duplicate parser, graph, handle, queue, hierarchy, or trigger authority. | Repository-wide integration removes replaced paths. | Unsupported |

## Generation Contract

The future checked-in generator must read a manifest pinning the authority revision and every contract path named by the owning roadmap, plus the current root Owner Registry. It must emit these identities in section and numeric order, reproduce this file byte-for-byte on repeated runs, and retain `Unsupported`, `Unknown`, `Malformed`, and `Missing` discoveries. Generation fails on an omitted or duplicate identity, missing authority path, changed fixed bound, classname row, unassigned adjacent owner, or item-count mismatch.
