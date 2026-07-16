# Legacy Source 1 CS:GO Ruleset Universe Roadmap

[`../../../docs/roadmap-contract.md`](../../../docs/roadmap-contract.md) defines aggregation, inventory, evidence, and denominator-review requirements. [`../../../TERMINOLOGY.md`](../../../TERMINOLOGY.md) defines ruleset and delivery status.

## Completion Denominator

This aggregation roadmap contains exactly 19 rows: one inventory-currentness row, 17 candidate child-ruleset completion rows, and one cross-child integration row. [`inventories/rulesets.md`](inventories/rulesets.md) indexes the same 17 child identities; it has no generator or accepted review and contributes no additional denominator identities. The current completion denominator is exactly 19 items.

The 17 candidate children are Casual, Competitive, Wingman, Weapons Expert, Arms Race, Demolition, Deathmatch, Training, Custom Baseline, Guardian, Co-op Strike, Flying Scoutsman, Retakes, Danger Zone, Surf, Bhop, and KZ. War Games is a catalog grouping rather than a selectable behavior owner; Arms Race, Demolition, Flying Scoutsman, and Retakes own its final active behaviors. Competitive owns its long/short live-match policy. Premier queueing, map veto, skill rating, and matchmaking are not ruleset behavior.

The denominator is Not accepted. Exact final-build mode bytes and target captures, the inventory generator, all child roadmaps, accepted community-mode contract manifests, required game/package interfaces, reviewer identity, review date, reviewed commit, and passing review record are Missing.

## Inputs

- The accepted legacy CS:GO game interface and typed team, agent, item, inventory, economy, damage, death, spawn, movement, weapon, equipment, projectile, C4, hostage, bot, entity, objective, and common-rule facts from [`../ROADMAP.md`](../ROADMAP.md).
- One selected ruleset identity and immutable configuration containing every lifecycle phase, objective policy, score rule, spawn rule, player/team restriction, timer, overtime rule, completion predicate, win predicate, and positive finite operation limit. No field has an implicit default.
- One immutable map/scenario/course classification and exact map/content identity. Rulesets consume typed game primitives and cannot inspect raw BSP, NAV, KeyValues, CFG, or plugin source bytes.
- Ordered admitted player/ruleset commands and ordered legacy CS:GO event/request journals at one Simulation tick boundary.
- For first-party children, the exact target `1.38.8.1` `gamemodes.txt`, referenced mode CFGs, map configurations/scripts, item/weapon definitions, and controlled target captures from depot `731` manifest `1224088799001669801` plus the target Windows runtime from depot `732` manifest `6314304446937576250`.
- For Surf, immutable `surftimer/SurfTimer` release `1.1.4` commit `6a563cd2e8023049815d2b8b301bcda7e0d75afa`, with the accepted stock CS:GO configuration, dependency set, and generated zone/style inventory.
- For Bhop, immutable `shavitush/bhoptimer` release `4.0.1` commit `e1be060c1c59529fd312bddc88fcd8e57e34a2c9`, with the accepted stock CS:GO configuration and generated track/zone/style inventory.
- For KZ, immutable `KZGlobalTeam/gokz` release `3.7.0` commit `c56aa84f0581167bc5a2998e9f631382f67141be`, with the accepted stock configuration and generated Vanilla, SimpleKZ, KZTimer, course, checkpoint, and teleport inventory.

## Outputs

- One immutable next ruleset state for exactly one selected legacy CS:GO ruleset.
- Ordered typed decisions for lifecycle phase, objective eligibility/transition, scoring, spawn permission/restriction, player/team restrictions, overtime, completion, winner, and next-round, next-stage, reset, or terminal transition.
- Ordered requests to legacy CS:GO game primitives. A ruleset cannot directly mutate game-owned player, item, weapon, economy, damage, entity, projectile, C4, hostage, bot, or movement state.
- Surf, Bhop, and KZ additionally emit immutable run facts: course/track/style identity, start tick, checkpoint/stage/teleport transitions, invalidation reason, finish tick, elapsed ticks, completion, and result eligibility. Durable records and rankings are not ruleset state.
- A structured failure naming ruleset identity, tick, phase, input identity, violated invariant, offending value, configured limit, and one coverage classification. Failure publishes no partial ruleset state or game request batch.
- One aggregate readiness result derived from the 17 child roadmaps, inventory currentness, and cross-child integration predicate.

## Invariants

- Exactly one ruleset owned by legacy CS:GO is selected for one gameplay session. Rulesets never compose through implicit inheritance, overlays, plugins, or same-name reuse from another game.
- A child owns its complete lifecycle, objectives, scoring, spawn restrictions, completion, wins, and mode-state transitions. Shared legacy CS:GO mechanics remain in the game module; game-independent mechanics remain in packages.
- Casual and Competitive select bomb-defusal or hostage-rescue objective policy only from the supplied map scenario. C4 and hostage mechanics remain game-owned.
- Long and short Competitive are explicit immutable configurations of one complete Competitive child. Premier queueing, veto, ratings, trust, penalties, and matchmaking cannot enter that child.
- War Games has no child implementation. Its catalog membership cannot override or duplicate Arms Race, Demolition, Flying Scoutsman, or Retakes behavior.
- Custom Baseline is exactly the final-build custom-mode initialization and map-owned objective/script seam. It does not accept an open universe of server plugins or undeclared rule overlays.
- Guardian and Co-op Strike own mission lifecycle and bot objectives while consuming game-owned bot decisions and map-owned scripted requests. They cannot implement another bot, navigation, entity, or scripting runtime.
- Surf, Bhop, and KZ consume generic Movement results plus explicit legacy CS:GO movement policy. They cannot replace generic acceleration, clipping, stepping, collision queries, or gameplay authority.
- Course timers use integer simulation ticks. Start, stop, stage, checkpoint, teleport, pause, practice, reset, and invalidation transitions are explicitly ordered; wall clock and render time never determine a result.
- Every collection and operation is bounded. Limit-plus-one fails before mutation, and no successful output is truncated.
- Every candidate identity remains visible until generation classifies it as accepted, unsupported, unknown, malformed, missing, or intentionally inert.
- A ruleset cannot own browser UI, durable records/rankings, content parsing, network transport, replay advancement, presentation execution, or Source 2 compatibility.

## Ownership Exclusions

- [`../ROADMAP.md`](../ROADMAP.md) owns legacy CS:GO-wide players, teams, agents, item schema, loadouts, inventory, economy mechanics, damage, weapons, equipment, projectiles, C4/hostage primitives, bots, game entities, movement differences, replicated state, prediction, events, messages, and presentation mappings.
- Movement owns generic movement advancement; Collision owns query truth; Entity owns generic state/I/O/touch/trigger behavior; Simulation owns tick scheduling and publication; the proposed NAV package owns navigation-mesh parsing.
- Applications own menus, HUD layout, input bindings, catalogs, course selection, and product assembly. Services own hosted records, rankings, matchmaking, ratings, and transport endpoints.
- Course definitions embedded in maps are immutable ruleset inputs. Course discovery, authoring, publication, durable record persistence, and ranking policy require separately declared owners.
- Premier, private queues, tournament veto administration, official matchmaking, trust/Prime policy, skill groups, XP/ranks, operation missions/rewards, and Game Coordinator behavior are excluded from these live-match rulesets.
- Inactive historical War Games configurations, community server plugins outside the three pinned targets, TF2 rulesets, CS:S rulesets, Source 2 modes, and undeclared legacy CS:GO modes are excluded. Adding one requires an inventory and denominator change before implementation.

## Behavior Families

| Target behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| `rulesets.inventory`: every declared legacy CS:GO ruleset has one stable identity, immutable authority revision, complete child path, sole owner, and coverage classification in [`inventories/rulesets.md`](inventories/rulesets.md). | The 17-item file is manually derived; the final-build first-party inputs, generator, child roadmaps, and review record are missing. | **Ruleset-inventory regeneration:** generate from exact first-party bytes, pinned community manifests, and the Owner Registry; compare identity, order, owner, child path, revision, classification, and exact count. | Blocked |
| `rulesets.casual`: the child owns complete drop-in bomb/hostage match and round lifecycle, teams, objective policy, economy/score policy, spawning, timeout, completion, winner, and transition. | No child roadmap exists, and exact `1.38.8.1` Casual configuration/captures are unavailable. | **Casual aggregation:** audit a Complete child and compare fixed bomb/hostage join, round, objective, elimination, timeout, score, reset, and match-completion timelines. | Blocked |
| `rulesets.competitive`: the child owns one complete long/short Competitive match, warmup, freeze, rounds, halftime, economy/score policy, timeouts, overtime disposition, surrender inputs, completion, winner, and transition. | No child roadmap exists, and exact final-build Competitive policy/captures are unavailable. | **Competitive match differential:** compare accepted long and short policy manifests plus complete round/half/match timelines, and audit that no Premier/matchmaking policy or implicit overlay enters the child. | Blocked |
| `rulesets.wingman`: the child owns 2v2 single-site match/round lifecycle, team limits, economy/score, shortened timers, halftime, spawn restrictions, completion, winner, and transition. | No child roadmap exists, and exact final-build Wingman configuration/captures are unavailable. | **Wingman aggregation:** audit a Complete child and compare fixed 2v2 join, spawn, plant/defuse, elimination, timeout, halftime, score, and match-completion timelines. | Blocked |
| `rulesets.weapons-expert`: the child owns its Competitive-derived match lifecycle plus each player's per-match purchase-use ledger and exact weapon restriction transitions. | No child roadmap exists, and final-build support/configuration is unverified. | **Weapons Expert aggregation:** compare complete match timelines and every purchase eligibility/consumption/reset boundary against exact target behavior. | Blocked |
| `rulesets.arms-race`: the child owns instant-respawn match lifecycle, ordered/random team weapon progressions, kill requirements, knife regression, leader state, golden-knife completion, winner, and reset. | No child roadmap exists, and exact final-build progression/configuration/captures are unavailable. | **Arms Race progression trace:** compare every kill/death/assist/regression/respawn/healthshot/progression step and final golden-knife win for both teams. | Blocked |
| `rulesets.demolition`: the child owns bomb rounds, fixed team weapon progression, kill/bonus upgrades, halftime, objective/score policy, completion, winner, and reset. | No child roadmap exists, and exact final-build Demolition configuration/captures are unavailable. | **Demolition round trace:** compare complete plant/defuse/elimination/timeout rounds, weapon/grenade upgrades, halftime, score, and match completion. | Blocked |
| `rulesets.deathmatch`: the child owns warmup, spawn/respawn, team/FFA policy, loadout selection, kill scoring, bonus weapon/timer, match timer, winner, and reset. | No child roadmap exists, and exact final-build Deathmatch variants/captures are unavailable. | **Deathmatch score timeline:** compare team, free-for-all, and accepted variant spawn/kill/bonus/loadout/score/timer/winner transitions exactly. | Blocked |
| `rulesets.training`: the child owns single-player lesson/course initialization, forced team/loadout, scripted objective sequence, reset/failure, completion, and terminal transition. | No child roadmap exists, and exact final-build training map/script behavior is unavailable. | **Training lesson transcript:** compare every accepted lesson start, instruction gate, target action, failure/reset, completion, and cleanup with fixed commands. | Blocked |
| `rulesets.custom-baseline`: the child owns only final-build Custom initialization, baseline configuration, typed map-rule requests, lifecycle, completion/winner inputs, reset, and failure for declared maps. | No child roadmap exists, and exact final-build custom baseline/captures are unavailable. | **Custom-baseline contract:** compare configuration application and fixed declared-map request/result timelines; require rejection of undeclared plugins, overlays, and rule identities. | Blocked |
| `rulesets.guardian`: the child owns cooperative mission setup, human team, bot-wave/challenge objective, equipment/economy policy, retries, failure, completion, winner, and mission transition. | No child roadmap exists; exact mission/configuration inventory and target captures are unavailable. | **Guardian mission trace:** compare every accepted mission's setup, bot waves, challenge counters, deaths/retries, objective completion, rewards facts, failure, and terminal transition. | Blocked |
| `rulesets.co-op-strike`: the child owns cooperative story-mission setup, human team, scripted stage/objective gates, bot waves, respawn/checkpoint policy, failure, completion, and terminal transition. | No child roadmap exists; exact mission/script inventory and target captures are unavailable. | **Co-op Strike mission trace:** compare every accepted mission's stage gates, scripted requests, bot waves, checkpoint/respawn, failure, completion, and cleanup in tick order. | Blocked |
| `rulesets.flying-scoutsman`: the child owns Casual-derived rounds with fixed SSG 08/knife loadout, no-buy economy, low gravity, air movement/accuracy policy, score, completion, winner, and reset. | No child roadmap exists, and exact final-build skirmish routing/configuration is unverified. | **Flying Scoutsman timeline:** compare initialization, movement-policy inputs, airborne shots, deaths/spawns, objective outcomes, score, and match completion on fixed maps. | Blocked |
| `rulesets.retakes`: the child owns team assignment, site selection, pre-plant sequence, blockers, loadout cards, freeze, retake/defend rounds, score, completion, winner, and reset. | No child roadmap exists, and exact final-build Retakes configuration/card inventory/captures are unavailable. | **Retakes round differential:** compare every site/team/card choice, auto-choice, plant boundary, blocker release, elimination/defuse/explosion, score, and first-to-eight completion. | Blocked |
| `rulesets.danger-zone`: the child owns squad/solo setup, spawn-area selection/deployment, safe-zone phases, loot/economy delivery policy, redeploy, survival objectives, elimination, last-survivor completion, winner, and cleanup. | No child roadmap exists, and exact final-build survival configuration, map scripts, and target captures are unavailable. | **Danger Zone match trace:** compare fixed solo/squad deployments, zone phases, loot/purchase/delivery, teammate redeploy, objectives, elimination, winner, and terminal cleanup. | Blocked |
| `rulesets.surf`: the child owns stock SurfTimer `1.1.4` session/course lifecycle, linear/staged tracks, zones, starts, stages/checkpoints, finishes, timing, spawning, practice/invalidation, completion, and result facts. | The target revision is pinned; no child roadmap, generated stock configuration/zone/style inventory, implementation, or retained differential evidence exists. | **Surf course differential:** replay fixed accepted linear and staged courses; compare movement inputs/results, timer ticks, zones, stages, invalidations, completion, and reset exactly. | Not started |
| `rulesets.bhop`: the child owns stock bhoptimer `4.0.1` session/track/style lifecycle, zones, starts, checkpoints, finishes, timing, spawning, practice/invalidation, completion, and result facts. | The target revision is pinned; no child roadmap, generated stock track/zone/style inventory, implementation, or retained differential evidence exists. | **Bhop course differential:** replay one fixed course per accepted stock style and track; compare movement inputs/results, timer ticks, zones, invalidations, completion, and reset exactly. | Not started |
| `rulesets.kz`: the child owns GOKZ `3.7.0` session/course lifecycle across Vanilla, SimpleKZ, and KZTimer, including starts, checkpoints, teleports, finishes, timing, spawning, invalidation, completion, and result facts. | The target revision is pinned; no child roadmap, generated mode/course inventory, implementation, or retained differential evidence exists. | **KZ course differential:** replay fixed accepted courses in all three modes; compare movement inputs/results, timer ticks, checkpoints/teleports, invalidations, completion, and reset exactly. | Not started |
| `rulesets.integration`: all 17 accepted children consume one current game/Simulation interface, every mode-routed behavior has one owner, and no child duplicates another child, game behavior, generic movement, product data, or transport behavior. | No child implementation or accepted legacy CS:GO/package interface exists. | **Cross-child ownership/integration audit:** execute each selected child through one composition and audit every routed declaration, producer, consumer, duplicate authority, fallback, compatibility layer, legacy path, and stale inventory. | Blocked |

## Generated Inventories

No generated legacy CS:GO ruleset inventory is accepted. Accepted inventory item count: 0.

| Output | Authority identity | Authority revision | Generator command | Candidate items | Accepted items |
|---|---|---|---|---:|---:|
| [`inventories/rulesets.md`](inventories/rulesets.md) | Exact final-build first-party mode declarations plus pinned stock SurfTimer, bhoptimer, and GOKZ contract manifests and the current Owner Registry | Target engine `8802`, protocol `13881`, depot manifests `731/1224088799001669801` and `732/6314304446937576250`; SurfTimer `1.1.4`/`6a563cd2e8023049815d2b8b301bcda7e0d75afa`; bhoptimer `4.0.1`/`e1be060c1c59529fd312bddc88fcd8e57e34a2c9`; GOKZ `3.7.0`/`c56aa84f0581167bc5a2998e9f631382f67141be`; exact retained first-party bytes/captures and accepted community manifests Missing | Missing | 17 | 0 |

The future generator is owned by `tools/playsrc`. It must consume the current Owner Registry, exact final-build mode declarations, pinned community release trees, and one accepted contract/evidence manifest per child; emit all stable identities in inventory order; and fail on duplicate names, cross-game reuse, missing child path, changed revision, unresolved composition, unowned mode behavior, unclassified discovery, or count mismatch. No command name is declared before that implementation exists.

## Exit Criteria

The legacy CS:GO ruleset universe is Complete only when all of these predicates pass:

- All 19 aggregation rows are Ready.
- The 17-item inventory is generated, current, and Accepted with reviewer identity, review date, reviewed commit, exact item count, and all eight review predicates passing.
- Every accepted inventory item has one Complete child roadmap and one complete selectable ruleset contract.
- Exact final-build first-party mode declarations/configuration and controlled target captures are retained and hashed.
- Surf, Bhop, and KZ each have a generated finite stock configuration/style/course schema, timer contract, invalidation rules, evidence corpus, and external product-data boundary at the pinned revision.
- Competitive contains only live-match long/short policy; Premier and matchmaking policy remain outside the ruleset.
- Every mode-routed behavior has exactly one owner; no ruleset duplicates game-wide or generic-package behavior.
- Simulation composes exactly one selected ruleset through the current interface, and applications, services, tools, and inspectors contain no duplicate ruleset transition.
- No required item remains Blocked, Unsupported, Unknown, Missing, Malformed, Partial, stale, duplicated, owner-conflicting, or dependent on a fallback, compatibility layer, or legacy path.

## Blockers

- **First-party target inputs:** exact `1.38.8.1` `gamemodes.txt`, mode CFGs, map configurations/scripts, item joins, file hashes, and controlled mode captures are not retained through a configured provider. The older public declaration snapshot proves candidate identities but cannot establish final-build equality.
- **Community contract manifests:** the three repository revisions are pinned, but the accepted stock dependency/configuration sets, generated course/zone/style universes, gameplay-only ownership cuts, and retained controlled captures are Missing.
- **Child denominators:** none of the 17 future child roadmap paths exists. First-party child contracts cannot be written exactly before target inputs are available; community child contracts have available next actions and remain Not started.
- **Generator and review:** no checked-in command emits [`inventories/rulesets.md`](inventories/rulesets.md), and no review record accepts its authority, count, ownership, evidence feasibility, or integration closure.
- **Required interfaces:** the legacy CS:GO game, Simulation, Movement, Collision, Entity, NAV, Networking, Replay, and presentation denominators are not accepted, so child integration and final evidence cannot complete.
