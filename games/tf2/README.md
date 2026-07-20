# TF2

## Objective

Implement the complete Team Fortress 2 game over generic Source modules.

## Responsibilities

- Own TF2 players, classes, items, attributes, weapons, projectiles, buildings, conditions, teams, and damage behavior.
- Own TF2 movement differences, prediction state, replication schemas, entities, objectives, game rules, and game-specific presentation mappings.
- Advance one bounded atomic core-state transition and immutable snapshot for all nine playable classes and both gameplay teams; selected stock schema/prefab/item instances; 19 class and three account loadout positions; ordered attribute providers/cache/invalidation; all 131 condition identities; ordinary health, overheal, healing, damage, crit, resistance, death/respawn and health/ammo/dropped pickups. [`inventories/core-state.md`](inventories/core-state.md) fixes the generated 322-item authority set without claiming the broader TF2 inventories complete.
- Provide the game behavior used by every TF2 ruleset and product.
- Declare exact TF2 content-provider targets, including the immutable `jump_beef` download identity consumed by local development.
- Advance one deterministic local RED/BLU Soldier/Demoman producer with separate restorable authority and predicted-presentation random streams, draw journals, game-owned launch/explosion sound events with selected original wave ordinals, configured stock clip/reserve state, held-primary cadence, deploy and honorbound-first-primary deadlines, four-phase activity-timed reload, sticky charge/detonation, explicit death/respawn, sustained resupply, exact self-blast damage/force, typed rocket Collision and sticky Physics requests/results, and separate Jump requests.
- Expose the session through one atomic 1–64-tick WASM phase, one selected-teamspawn startup descriptor, one explicit map-bound Jump course configuration, and one bounded generation-bound version-3 binary snapshot containing complete Movement, TF2 loadout, Entity transform/event, Jump, projectile, and gameplay sections.
- Expose one immutable producer snapshot containing five-word conditions, exact weapon/lifecycle/projectile state, activities, damage requests, Entity mover requests, Collision/Physics requests, map effects, reconciliation requests, and resupply-model timing for later Simulation/WASM publication.
- Stage and atomically activate direct-BSP browser sessions through a game-owned module worker; transfer canonical map bytes only on a verified derived-cache miss.
- Apply generic translated-brush `trigger_teleport` contacts after movement, preserve physical velocity, and publish trigger/destination/position/yaw facts to Jump and browser presentation consumers.
- Project Soldier/Demoman class speed, condition/item speed and jump factors, air-control cap, 1.2-times bunnyhop cap, backward clamp, TF2 hull/view values, jump eligibility/impulse, crouch eligibility, surface factors, and ruleset noclip permission through one policy consumed by generic Movement.
- Retain the complete Movement state/result inside the TF2 session, expose its versioned snapshot and accepted mode-request seam, and keep the compact browser player projection presentation-only.
- Execute `jump_beef`'s five buttons, four doors, one cyclic movelinear, three respawn rooms, 22 multiple triggers, two hurt triggers, 56 teleports, and 22 `func_regenerate` volumes through ordered Entity phases; route admitted direct rocket targets through button damage and linked-door I/O; publish mover requests/results, landmark-aware teleports, filter decisions, trigger effects, edge-counted room/regenerate contacts, associated dynamic-prop body plus open/close animation state, and three-second cooldown without game-owned mover math.
- Publish the existing Entity brush-model snapshot without remapping it: one expected Entity revision and the current Collision snapshot revision atomically expose source/registry identity, Entity tick/revision, source-ordered model/local/world/parent/render/draw fields, and complete mover state.
- Publish rockets through point-trace requests and stickies through IVP body requests. Rocket launch uses the Source forward/right/up basis, stock/crouched/item-flipped/player-flipped/center-fire muzzle policy, distinct aim and brush-muzzle masks, velocity orientation, and one same-order result per preceding flight request. A rocket touching sky emits one non-explosive terminal event before removal, allowing presentation to stop its trail and destroy it without explosion. Require one same-order accepted result for every preceding sticky create/step request before game state advances; reject missing, stale, duplicate, reordered, unsolicited, malformed, or motion-disabled flying results atomically. Retain owner, launcher, team, charge/arm/stuck state, airborne pulse eligibility without a contact normal, accepted-surface contact data, ordered presentation-neutral events, and one exact radius-damage request without substituting a projectile solver.

## Non-Responsibilities

- Generic Source parsing, collision, physics, movement, simulation, networking, or presentation behavior.
- Rules belonging to one TF2 mode or community ruleset.
- Tempus records, rankings, UI, or other product behavior.

## Relationships

Composes packages and supplies game behavior to [`rulesets/`](rulesets/), TF2 applications, replay, and future gameplay servers.

## Completion

Complete when the declared TF2 behavior universe is handled with credible evidence across gameplay, prediction, replay interpretation, and presentation integration.
