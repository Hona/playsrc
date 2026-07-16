# Counter-Strike: Source Ruleset Universe Roadmap

[`../../../docs/roadmap-contract.md`](../../../docs/roadmap-contract.md) defines aggregation, inventory, evidence, and denominator-review requirements. [`../../../TERMINOLOGY.md`](../../../TERMINOLOGY.md) defines ruleset and delivery status.

## Completion Denominator

This aggregation roadmap contains exactly 10 rows: one inventory-currentness row, eight candidate child-ruleset completion rows, and one cross-child integration row. [`inventories/rulesets.md`](inventories/rulesets.md) indexes the same eight child identities; it has no generator or accepted review and contributes no additional denominator identities. The current completion denominator is exactly 10 items.

The eight candidate child identities are standard bomb defusal, standard hostage rescue, standard assassination, standard escape, competitive, surf, bhop, and KZ. No child roadmap exists. The denominator is Not accepted.

## Inputs

- The accepted CS:S game interface and typed objective, death, team, economy, spawn, entity, movement, C4, hostage, and round-state facts from [`../ROADMAP.md`](../ROADMAP.md).
- One immutable selected ruleset identity and configuration containing every lifecycle phase, objective policy, score rule, spawn rule, player/team restriction, timer, overtime rule, completion predicate, win predicate, and positive finite operation limit. No field has an implicit default.
- One immutable map-scenario classification and exact map/content identity. Rulesets consume typed CS:S primitives and cannot inspect raw BSP or KeyValues bytes.
- Ordered admitted player/ruleset commands and ordered CS:S event/request journals at one Simulation tick boundary.
- The authority manifest declared by [`inventories/rulesets.md`](inventories/rulesets.md), including an immutable target revision and evidence method for each child. Those authorities are currently missing.

## Outputs

- One immutable next ruleset state for exactly one selected CS:S ruleset.
- Ordered typed decisions for lifecycle phase, objective eligibility and transition, scoring, spawn permission/restriction, player/team restrictions, overtime, completion, winner, and next-round or terminal transition.
- Ordered requests to CS:S game primitives; a ruleset cannot mutate game-owned player, weapon, economy, damage, entity, C4, hostage, or movement state directly.
- A structured failure naming ruleset identity, tick, phase, input identity, violated invariant, offending value, configured limit, and one coverage classification. Failure publishes no partial ruleset state or game request batch.
- One aggregate readiness result derived from the eight child roadmaps, inventory currentness, and cross-child integration predicate.

## Invariants

- Exactly one ruleset owned by CS:S is selected for one gameplay session. Rulesets never compose by implicit inheritance, overlay, or same-name reuse from another game.
- A child ruleset owns its complete lifecycle, objectives, scoring, spawning restrictions, completion, wins, and mode-state transitions. Shared CS:S mechanics remain in the game module; game-independent mechanics remain in packages.
- Standard objective rulesets consume reusable C4, hostage, escape, VIP, team, spawn, economy, and death facts without duplicating those mechanics.
- Surf, bhop, and KZ consume generic Movement results plus explicit CS:S movement policy. They cannot replace generic acceleration, clipping, stepping, collision queries, or gameplay authority.
- Competitive cannot be an undeclared policy overlay. Its accepted child contract must be either one complete selectable ruleset or removed and represented as explicit configuration owned by each affected complete ruleset.
- Every timer uses simulation ticks/time. Every collection and operation is bounded. Limit-plus-one fails before mutation, and no successful output is truncated.
- Every candidate identity remains visible until generation classifies it as accepted, unsupported, unknown, malformed, missing, or intentionally inert.
- A ruleset cannot own browser UI, records/rankings services, map parsing, network transport, replay advancement, or presentation execution.

## Ownership Exclusions

- [`../ROADMAP.md`](../ROADMAP.md) owns CS:S-wide players, teams, classes, inventory, economy mechanics, damage, weapons, grenades, C4 and hostage primitives, game entities, movement differences, replicated state, prediction, events, messages, and presentation mappings.
- Movement owns generic movement advancement; Collision owns query truth; Entity owns generic state/I/O/touch/trigger behavior; Simulation owns tick scheduling and publication.
- Applications own menus, HUD layout, input bindings, catalogs, and product assembly. Services own hosted records, rankings, matchmaking, and transport endpoints.
- Course definitions embedded in maps are immutable ruleset inputs. Course discovery, authoring, publication, record persistence, and ranking policy require separately declared owners.
- TF2 rulesets, legacy CS:GO rulesets, Source 2 modes, and undeclared CS:S community modes are excluded. Adding one requires an inventory and denominator change before implementation.

## Behavior Families

| Target behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| `rulesets.inventory`: every declared CS:S ruleset has one stable identity, immutable authority revision, complete child path, sole owner, and coverage classification in [`inventories/rulesets.md`](inventories/rulesets.md). | The eight-item file is manually derived; target revisions, generator, child roadmaps, and review record are missing. | **Ruleset-inventory regeneration:** generate from accepted declarations and authority manifests; compare identity, order, owner, child path, revision, classification, and exact count. | Blocked |
| `rulesets.standard-bomb-defusal`: the child roadmap completely owns bomb-round lifecycle, objective eligibility, scoring, spawn restrictions, timeout, completion, winner, and round transition while consuming CS:S C4 primitives. | No child roadmap or target authority exists. | **Bomb-ruleset aggregation:** audit an accepted child denominator and compare fixed plant/abort/drop/pickup/defuse/explode/elimination/timeout round timelines through the current CS:S interface. | Blocked |
| `rulesets.standard-hostage-rescue`: the child roadmap completely owns hostage-round lifecycle, rescue eligibility/threshold, scoring, spawn restrictions, timeout, completion, winner, and round transition while consuming CS:S hostage primitives. | No child roadmap or target authority exists. | **Hostage-ruleset aggregation:** audit an accepted child denominator and compare fixed follow/injury/death/rescue/elimination/timeout round timelines through the current CS:S interface. | Blocked |
| `rulesets.standard-assassination`: the child roadmap completely owns VIP selection, escort objective, safety/death/timeout outcomes, scoring, restrictions, completion, winner, and round transition while consuming CS:S VIP primitives. | No child roadmap exists, and current target support for this standard scenario is not established. | **Assassination-ruleset aggregation:** audit accepted authority and child rows, then compare fixed VIP assignment/queue/safety/death/elimination/timeout round timelines. | Blocked |
| `rulesets.standard-escape`: the child roadmap completely owns eligible escapers, escape objective, ratio/threshold outcomes, team rotation, scoring, restrictions, timeout, completion, winner, and round transition while consuming CS:S escape primitives. | No child roadmap exists, and current target support for this standard scenario is not established. | **Escape-ruleset aggregation:** audit accepted authority and child rows, then compare fixed partial/full escape, neutralization, timeout, team-rotation, and round-reset timelines. | Blocked |
| `rulesets.competitive`: the child roadmap owns one complete selectable competitive lifecycle and policy, or an accepted ownership decision removes this identity and assigns each policy value to a complete standard ruleset. | No authoritative competitive policy, revision, composition decision, or child roadmap exists. | **Competitive contract comparison:** compare the accepted policy manifest and complete match/round timelines, and audit that no implicit overlay or duplicated lifecycle remains. | Blocked |
| `rulesets.surf`: the child roadmap owns the declared surf session/course lifecycle, starts, checkpoints, finishes, timing, spawning, restrictions, failures, completion, and results while consuming generic Movement and CS:S policy. | No authoritative surf ruleset organization, revision, course schema, evidence target, or child roadmap exists. | **Surf course differential:** replay fixed accepted courses and command streams; compare movement inputs/results, course state, timer ticks, checkpoints, invalidations, completion, and reset exactly. | Blocked |
| `rulesets.bhop`: the child roadmap owns the declared bhop session/course lifecycle, starts, checkpoints, finishes, timing, spawning, restrictions, failures, completion, and results without replacing generic Movement. | No authoritative bhop ruleset organization, revision, course schema, evidence target, or child roadmap exists. | **Bhop course differential:** replay fixed accepted courses and command streams; compare movement inputs/results, course state, timer ticks, checkpoints, invalidations, completion, and reset exactly. | Blocked |
| `rulesets.kz`: the child roadmap owns the declared KZ session/course lifecycle, starts, checkpoints, teleports, finishes, timing, spawning, restrictions, failures, completion, and results without replacing generic Movement. | No authoritative KZ ruleset organization, revision, course schema, evidence target, or child roadmap exists. | **KZ course differential:** replay fixed accepted courses and command streams; compare movement inputs/results, course state, timer ticks, checkpoints/teleports, invalidations, completion, and reset exactly. | Blocked |
| `rulesets.integration`: all eight accepted children consume one current CS:S/Simulation interface, every mode-routed behavior has one owner, and no child duplicates another child, game behavior, generic movement, product data, or transport behavior. | No child implementation or accepted CS:S/package interface exists. | **Cross-child ownership/integration audit:** execute each selected child through one composition and repository-audit every mode-routed declaration, producer, consumer, duplicate authority, fallback, compatibility layer, legacy path, and stale inventory. | Blocked |

## Generated Inventories

No generated CS:S ruleset inventory is accepted. Accepted inventory item count: 0.

| Output | Authority identity | Authority revision | Generator command | Candidate items | Accepted items |
|---|---|---|---|---:|---:|
| [`inventories/rulesets.md`](inventories/rulesets.md) | Current playsrc CS:S declarations plus selected immutable standard, competitive, surf, bhop, and KZ target manifests | playsrc `1c2184c11353a618d8ef941c7f161e488346203e`; all external target revisions Missing | Missing | 8 | 0 |

The future generator is owned by `tools/playsrc`. It must consume the current Owner Registry, explicit CS:S ruleset declarations, and one retained immutable authority manifest per candidate; emit all stable identities in inventory order; and fail on duplicate names, cross-game reuse, missing child path, missing revision, unresolved composition, unowned mode behavior, unclassified discovery, or count mismatch. No command name is declared before that implementation exists.

## Exit Criteria

The CS:S ruleset universe is Complete only when all of these predicates pass:

- All 10 aggregation rows are Ready.
- The eight-item inventory is generated, current, and Accepted with reviewer identity, review date, reviewed commit, exact item count, and all eight review predicates passing.
- Every accepted inventory item has one Complete child roadmap and one complete selectable ruleset contract.
- Competitive has one accepted non-overlay composition contract.
- Surf, bhop, and KZ each name an immutable authority, course schema, movement configuration, timer contract, invalidation rules, evidence corpus, and external product-data boundary.
- Every mode-routed behavior has exactly one owner; no ruleset duplicates CS:S-wide game behavior or generic package behavior.
- Simulation composes exactly one selected CS:S ruleset through the current interface, and applications, services, tools, and inspectors contain no duplicate ruleset state transition.
- No required item remains Blocked, Unsupported, Unknown, Missing, Malformed, Partial, stale, duplicated, owner-conflicting, or dependent on a fallback, compatibility layer, or legacy path.

## Blockers

- **Ruleset authorities:** no immutable target revision, configuration manifest, controlled runtime capture set, or accepted comparison corpus is declared for any of the eight candidates. The standard assassination and escape candidates also lack an established current-target support disposition. Checked the root Owner Registry, both CS:S READMEs, the CS:S game roadmap, Valve application `240` metadata, Valve Source SDK 2013 commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`, and the Valve Developer Community map-prefix and CS:S event pages.
- **Competitive composition:** competitive is declared as a policy target but the repository does not decide whether it is one complete selectable ruleset or explicit configuration owned by each standard objective ruleset. Implicit overlays are prohibited.
- **Community mode definitions:** surf, bhop, and KZ have no selected organization/release, course schema, movement configuration, timer and checkpoint contract, invalidation policy, or records/rankings ownership boundary. Their names alone cannot define a denominator. Checked the CS:S ruleset README, root target declarations, current game/ruleset trees, and the Valve Developer Community Kreedz overview.
- **Generator and review:** no checked-in command emits [`inventories/rulesets.md`](inventories/rulesets.md), and no review record accepts its authority, count, ownership, evidence feasibility, or integration closure. Checked `tools/`, the root Owner Registry, and the current CS:S game/ruleset trees.
- **Required interfaces:** the CS:S game, Simulation, Movement, Collision, Entity, Networking, Replay, and presentation denominators are not accepted, so child integration and evidence cannot complete.
