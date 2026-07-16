# TF2 Ruleset Universe Roadmap

[`../../../docs/roadmap-contract.md`](../../../docs/roadmap-contract.md) defines the normative roadmap schema and denominator review gate. [`../../../TERMINOLOGY.md`](../../../TERMINOLOGY.md) defines ruleset, delivery status, coverage classification, inventory, and Complete.

## Completion Denominator

This aggregation roadmap contains exactly 4 behavior rows. [`inventories/rulesets.md`](inventories/rulesets.md) contains 31 candidate ruleset assignments and 0 accepted inventory items. Candidate inventory items do not enter the denominator until one checked-in generator emits the inventory and a denominator review accepts it. The current completion denominator is exactly 4 items; accepting all current candidates would make it 35 items.

The candidate universe contains 25 primary rulesets, 3 modifiers, and 3 in-session match policies. It covers the configured TF2 content build, the declared TF2 jump target, and the official TF2 game-mode, objective, round, training, managed-match, and tournament contracts. Catalog labels that do not alter gameplay are not rulesets.

The denominator is Not accepted. The configured FGD input, checked map/script index, inventory generator, 30 proposed owner registrations, 31 child roadmaps, reviewer identity, review date, reviewed commit, and passing review record are Missing.

## Inputs

- Valve Source SDK 2013 commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`: `src/game/shared/tf/{tf_shareddefs.h,tf_gamerules.h,tf_gamerules.cpp,entity_capture_flag.h,entity_capture_flag.cpp,tf_logic_robot_destruction.h,tf_logic_robot_destruction.cpp,tf_logic_player_destruction.h,tf_logic_player_destruction.cpp,tf_match_description.h,tf_match_description_casual.cpp,tf_match_description_comp.cpp,tf_matchmaking_shared.h,tf_gcmessages.proto}`; `src/game/server/{team_control_point.cpp,team_control_point_master.cpp,team_control_point_round.cpp,team_train_watcher.cpp,trigger_area_capture.cpp}`; `src/game/server/tf/{func_capture_zone.cpp,tf_passtime_logic.cpp,tf_passtime_ball.cpp,func_passtime_goal.cpp,player_vs_environment/tf_mann_vs_machine_logic.cpp,player_vs_environment/tf_population_manager.cpp}`; and mode-specific TF2 client HUD consumers.
- Configured TF2 content build `10822003`: `steam.inf`; `scripts/items/items_game.txt`; the `tf2_misc_dir.vpk` archive index; the exact non-recursive `maps/*.bsp` directory index; each BSP entity lump and embedded PAK index; and the configured TF2 FGD. The FGD input is Missing.
- The accepted TF2 game roadmap and generated TF2 objective, entity, item, attribute, condition, cvar, and networked-state inventories. These inputs are not accepted.
- One selected map identity and immutable map metadata; one admitted roster; one TF2 gameplay state; one simulation tick; ordered player commands and entity events; and an explicit optional in-session match-policy selection.
- The accepted root Owner Registry, every accepted child ruleset roadmap, and the applications, services, tools, and packages named by those child roadmaps.

## Outputs

- One generated `games/tf2/rulesets/inventories/rulesets.md` containing a stable identity, kind, exact owner, identification predicate, behavior boundary, evidence method, and coverage classification for every item.
- One unambiguous composition assignment containing exactly one primary ruleset, zero or more disjoint modifiers in stable identity order, and zero or one in-session match policy. Training, item testing, and jump select no stock objective ruleset beside themselves.
- One exact child-roadmap dependency for every accepted item and one aggregate delivery result derived from those children under the aggregation contract.
- Blocked records for every missing authority, owner, child denominator, producer, consumer, evidence method, or conflicting classification.
- Child-owned canonical objective, score, timer, team, spawn, round, win, overtime, stalemate, and HUD-state outputs consumed through the selected composition. This aggregation roadmap does not produce those states independently.

## Invariants

- Every gameplay session selects exactly one primary ruleset before mode-owned state advances. A primary ruleset cannot silently change because an unrelated entity, filename prefix, catalog label, or presentation resource is present.
- `modifier.mannpower`, `modifier.medieval`, and `modifier.holiday-event` can alter only their inventoried fields. They cannot replace primary objective ownership or duplicate TF2 player, weapon, item, condition, damage, or entity primitives.
- `policy.casual`, `policy.competitive`, and `policy.tournament` are mutually exclusive in one composition. They own admitted-session rules only; match formation, queues, ratings, penalties, credentials, and server allocation remain outside rulesets.
- Map identification follows the accepted generated route. Explicit scripted-mode identities and hybrid markers take precedence over their implementation carrier's base game type; objective signatures then distinguish flag, train, control-point, Arena, Mann vs. Machine, Robot Destruction, Player Destruction, PASS Time, training, and item-testing rules. Only the accepted Special Delivery and Territorial Control contracts use their target-defined prefixes.
- `MAX_CONTROL_POINTS` is 8, each team has at most 3 declared previous-point prerequisites per control point, the replicated capture-layout string is 32 bytes, PASS Time exposes at most 16 track points, and Mann vs. Machine exposes 24 wave-class slots. Child roadmaps must retain these target bounds and add finite execution bounds for every queue, script, roster, objective, and event operation.
- The common TF2 round phases, player/class/item/weapon/projectile/building/condition mechanics, primitive objective entities, replicated state, prediction, and game-specific presentation mappings remain owned by `games/tf2`. A ruleset owns only mode-specific eligibility, ordering, scoring, timers, transitions, and completion.
- A catalog category such as `other`, `featured`, `halloween`, `christmas`, `competitive_6v6`, or `specialevent_placeholder` is never a ruleset merely because it groups maps. A map receives ruleset identities only from accepted gameplay predicates.
- Unknown, malformed, unsupported, missing, or multiply owned mode input produces an explicit classification and no selected composition. Selection has no default-mode, filename-guessing, compatibility, or fallback path.

## Ownership Exclusions

- `games/tf2` owns TF2-wide teams, players, classes, items, attributes, weapons, projectiles, buildings, bots, conditions, damage, movement differences, objective primitives, game-rule primitives, VScript bindings, replication schemas, prediction, and presentation mappings.
- Each accepted `games/tf2/rulesets/<ruleset>` child owns only the mode, modifier, or in-session policy assigned in the generated inventory. This aggregation roadmap owns enumeration, assignment, composition constraints, and acceptance gates, not child simulation.
- `packages/world/map` owns canonical map data; `packages/world/entity` owns generic lifecycle and Entity I/O; `packages/runtime/simulation` owns tick and event ordering; `packages/runtime/networking` owns transport and replicated encoding; `packages/runtime/replay` owns recorded-state advancement; presentation packages own HUD and rendered output after consuming canonical facts.
- TF2 applications select declared experiences and compositions. Matchmaking and game-server services own queueing, credentials, admission transport, allocation, process lifetime, and hosted sessions. Tools own generation and evidence orchestration.
- Tempus records, rankings, APIs, catalogs, and UI belong to the Tempus application and services. The jump child owns only course and run rules.
- Counter-Strike: Source and legacy Source 1 Counter-Strike: Global Offensive rulesets remain separate game-owned universes.

## Behavior Families

| Target behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| The declared TF2 target has one finite ruleset universe containing every primary mode, gameplay modifier, and in-session match policy, while excluding non-gameplay catalog groupings. | The candidate inventory contains 31 assignments, but no generator or accepted configured FGD and map/script index establishes the final item set. | **Authority-union regeneration:** generate from the pinned SDK declarations, configured FGD, item schema, exact BSP/entity/PAK indexes, and declared jump target; compare stable identities exactly and require every discovery to occur once. | Blocked |
| Every inventory item has exactly one registered TF2 child owner and every adjacent TF2-wide, package, application, service, and other-game behavior remains with its registered owner. | `jump` is the only registered child owner; 30 exact proposed child paths are absent from the root Owner Registry. | **Owner-registry join:** join every generated item to the root Owner Registry and every affected Ownership Exclusions section; require one child owner, no duplicate behavior field, and no unregistered path. | Blocked |
| Every accepted child denominator derives lifecycle, map identification, objective state, scoring, teams, spawning, round transitions, win conditions, overtime, stalemate, entities, HUD-state outputs, and modifiers from the same accepted inventory identity. | No child ruleset has a `ROADMAP.md`; the registered jump child contains only a README. | **Child-coverage matrix:** compare all 31 accepted inventory identities with child rows and evidence plans; require every contract dimension to have one child row and every child to record the same inventory revision. | Blocked |
| The inventory and composition assignment are reproducible, bounded, current, and fail instead of guessing when authorities change or disagree. | No checked-in generator command, immutable map/script index, composition manifest, or repeated-generation evidence exists. | **Clean-workspace reproducibility test:** run the checked-in generator twice from fixed authority inputs; require byte-identical output, identical counts and hashes, stable composition order, and explicit retained classifications for every failure. | Not started |

## Generated Inventories

No generated TF2 ruleset inventory is accepted. Accepted item count: 0.

| Output | Authority identity | Authority revision | Generator command | Candidate items | Generated items | Accepted items |
|---|---|---|---|---:|---:|---:|
| [`inventories/rulesets.md`](inventories/rulesets.md) | Official TF2 game types, flags, game-rule markers, objective entities, round logic, mode implementations, match descriptions, tournament policy, and HUD consumers; configured `items_game.txt`; exact configured BSP entity and embedded-script indexes; configured TF2 FGD; declared jump child | SDK `88fa198fba3fb85d46d4c95018254693fdc3af0a`; TF2 build `10822003`; item schema SHA-256 `47900e0d174971625a76625fe311a012910031171d0b121ff5f628078c83214d`; configured FGD Missing; checked immutable map/script index Missing | Missing | 31 | 0 | 0 |

The future generator is owned by `tools/playsrc`. It must consume retained immutable authority inputs, preprocess official TF2 client/server/shared declarations, parse the configured FGD and item schema, enumerate the exact BSP directory and each BSP entity/PAK index, join every mode marker and catalog assignment, and include the accepted jump contract. It must emit the stable inventory order, retain every malformed, unknown, unsupported, missing, excluded, or owner-conflict discovery, and fail on a changed authority, omitted mode, duplicate identity, ambiguous route, unregistered owner, missing contract dimension, or item-count mismatch. No command name is declared before that tool operation exists.

## Exit Criteria

The TF2 ruleset universe is Complete only when all of these predicates pass:

- All 4 aggregation behavior rows are Ready.
- The generator emits exactly the accepted current item set and a denominator review records `Accepted`, reviewer identity, review date, reviewed commit, and a passing result for every denominator-gate predicate.
- Every accepted item has one registered child path, an accepted child roadmap, and all child exit criteria passing.
- Every configured TF2 map, item-schema map reference, Mann vs. Machine mission, mode entity, embedded gameplay script, and declared jump input has exactly one primary, modifier, policy, excluded, or blocked disposition.
- Every primary, modifier, and policy composition passes exact routing, lifecycle, objective, score, timer, team, spawn, transition, win, overtime, stalemate, entity-output, HUD-state, reset, and error comparisons for its declared content inputs.
- TF2, Map, Entity, Simulation, Networking, Replay, presentation, applications, services, and tools consume the current ruleset identities and canonical outputs without a duplicate gameplay authority.
- No required owner, input, dependency, decision, evidence method, behavior, or integration remains Unsupported, Unknown, Missing, Partial, or Blocked; no fallback, compatibility layer, legacy route, or stale inventory remains.

## Blockers

- **Configured FGD:** the exact configured `bin/tf.fgd`, `tf.fgd`, and `scripts/tf.fgd` locations are Missing, and the `tf2_misc_dir.vpk` index contains no FGD entry. The candidate TF2 entity and objective inventories record a configured `tf.fgd` hash that cannot be reproduced from the current configured root. An exact FGD provider identity and bytes are required before inventory acceptance.
- **Owner Registry:** the root Owner Registry contains the TF2 ruleset-universe aggregation and the jump child only. It does not register the other 30 proposed child paths in the candidate inventory. The root owner must accept or reject all 30 paths in one ownership checkpoint.
- **Child denominators:** no TF2 ruleset child has a `ROADMAP.md`; only `rulesets/jump/README.md` exists. The aggregation cannot become Ready until every accepted child has an accepted finite denominator and evidence plan.
- **TF2 parent contract:** `games/tf2/ROADMAP.md` is a 62-row Draft, and its nine manually derived candidate inventories have no generator or accepted review. Shared round, objective-primitive, team, spawn, replicated-state, condition, item, attribute, and VScript-binding dependencies cannot satisfy integration evidence.
- **Aggregation synchronization:** the root roadmap and TF2 game Draft still state that this ruleset-universe roadmap does not exist. Their current dependency and blocker text must be regenerated by those owners after this candidate is reviewed.
- **Mann vs. Machine bot ownership:** the required universe includes Mann vs. Machine, but the TF2 game Draft excludes bots/AI from its Active denominator. MvM requires reusable TF2 bot/player mechanics while the MvM child owns mission, population, wave, bomb, currency, checkpoint, and win policy. No registered owner currently supplies the excluded bot behavior, so the TF2 parent denominator and exclusions must change before the MvM child can be accepted.
- **Map and script authority:** the configured root currently exposes 233 loose BSP names, 239 item-schema category references to 220 unique maps, 29 Mann vs. Machine missions, and 798 embedded script occurrences in 41 maps, but no checked immutable index records their per-file hashes and BSP entity/PAK identities. The live directory listing cannot satisfy an accepted inventory revision.
- **Inventory generation and review:** no checked-in command emits [`inventories/rulesets.md`](inventories/rulesets.md), no composition manifest exists, and no denominator review record exists. Checked the current tool trees, root manifests, and assigned ruleset paths.
