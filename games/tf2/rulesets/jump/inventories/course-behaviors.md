# TF2 Jump Course-Behavior Inventory

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md)

Authority identity: Valve Source SDK 2013 TF2 class, item, projectile, damage, movement, respawn, trigger, filter, spawn-room, and entity-I/O contracts; Tempus public API v0 OpenAPI contract; Tempus public jump-normalization, zone-teleport cleanup, class-health, and server-policy declarations.

Authority revision: SDK `88fa198fba3fb85d46d4c95018254693fdc3af0a`; Tempus OpenAPI bytes retrieved 2026-07-16, SHA-256 `3d7bf3b65d4e74a2ae3b12f5c6b45a1cf003f29b6dc35882cccac12c8eb59d99`; Tempus public plugin commit `d7491f5295fbacee1a63fa6603c52cf585fcac18`; Tempus public server-policy commit `31171fee1073751a1b1feddc93dfbd327f3d0411`.

Generator command: Missing

Output path: `games/tf2/rulesets/jump/inventories/course-behaviors.md`

Candidate item count: 117

Accepted item count: 0

Generation state: manually derived candidate; hand edits are invalid after a generator exists.

The required coverage is the disposition an implementation must establish. It does not change delivery status. Tempus application identifiers, map catalog entries, records, placements, ranks, demos, tiers, ratings, authors, and videos are excluded.

## Zones And Topologies

Candidate count: 15.

| Stable identity | Accepted input or public identity | Required ruleset behavior | Required coverage |
|---|---|---|---|
| `zone.map-start` | `map`, index 1 | Start the map aggregate run for an eligible player on the accepted entry edge. | Handled |
| `zone.map-end` | `map_end`, index 1 | Complete the map aggregate run only after its topology predicates pass. | Handled |
| `zone.course-start` | `course`, dense positive index | Start the indexed stage run without replacing map-run identity. | Handled |
| `zone.course-end` | `course_end`, dense positive index | Complete the same-index stage except that the final stage completes at `map_end`. | Handled |
| `zone.linear-marker` | `linear`, index 1 | Select linear checkpoint topology; it is metadata and never starts a second timer. | Handled |
| `zone.checkpoint` | `checkpoint`, dense positive index | Admit one ordered checkpoint visit for the topology-defined run identities. | Handled |
| `zone.bonus-start` | `bonus`, dense positive index | Start the same-index bonus run. | Handled |
| `zone.bonus-end` | `bonus_end`, matching bonus index | Complete the same-index bonus run. | Handled |
| `zone.trick` | `trick`, positive index | Retain identity; exact start, end, and validity semantics require an accepted target contract. | Unknown |
| `zone.misc` | `misc`, positive index | Retain identity and supplied name; never infer course behavior from name text. | Unknown |
| `zone.special` | `special`, positive index | Retain identity; exact mode effect requires an accepted target contract. | Unknown |
| `topology.linear` | One map pair, one linear marker, zero course zones, ordered checkpoints | Validate the finite linear graph and require checkpoint admission before map completion. | Handled |
| `topology.staged` | One map pair, no linear marker, `N` course starts, `N-1` course ends | Validate dense stage indices; course `N` and the map both terminate at `map_end`. | Handled |
| `topology.bonus` | Zero or more one-to-one indexed bonus pairs | Isolate each bonus lifecycle from map and stage completion. | Handled |
| `topology.map-aggregate` | One map run over either linear or staged topology | Preserve one map-run identity while subordinate stage or bonus runs transition. | Handled |

## Eligibility And Fixed Ruleset Policy

Candidate count: 20.

| Stable identity | Required value or input | Ruleset decision | Behavior executor | Required coverage |
|---|---|---|---|---|
| `eligibility.class.soldier` | TF2 class 3 | Eligible for map, stage, bonus, and accepted trick runs. | TF2 game | Handled |
| `eligibility.class.demoman` | TF2 class 4 | Eligible for map, stage, bonus, and accepted trick runs. | TF2 game | Handled |
| `eligibility.team.red` | TF2 team 2 | Eligible under the same course rules as BLU. | TF2 game | Handled |
| `eligibility.team.blue` | TF2 team 3 | Eligible under the same course rules as RED. | TF2 game | Handled |
| `eligibility.team.unassigned` | Unassigned or auto-assignment state | Cannot start or complete a run. | TF2 game | Handled |
| `eligibility.team.spectator` | Spectator or observer state | Cannot start or complete a run. | TF2 game | Handled |
| `policy.soldier-max-health` | 900 | Supply the jump maximum-health policy for Soldier; TF2 computes health state. | TF2 game | Handled |
| `policy.soldier-fall-damage` | Disabled | Reject Soldier fall-damage mutation without changing generic landing output. | TF2 game | Handled |
| `policy.air-acceleration` | `sv_airaccelerate = 10` | Select the fixed value consumed by Movement. | Movement | Handled |
| `policy.random-criticals` | `tf_weapon_criticals = 0` | Disable random critical classification. | TF2 game | Handled |
| `policy.fixed-weapon-spread` | `tf_use_fixed_weaponspreads = 1` | Select fixed TF2 spread policy. | TF2 game | Handled |
| `policy.respawn-wave` | `mp_respawnwavetime = 0` | Request no mode-added respawn-wave delay. | TF2 game | Handled |
| `policy.waiting-for-players` | `mp_waitingforplayers_time = 0` | Start no waiting-for-players phase. | TF2 game/ruleset composition | Handled |
| `policy.demoman-charge-drain` | `tf_demoman_charge_drain_time = 0` | Supply the normalized Demoman charge-drain value. | TF2 game | Handled |
| `policy.maximum-charge-speed` | `tf_max_charge_speed = 0` | Supply the normalized TF2 charge-speed value. | TF2 game | Handled |
| `policy.whip-speed-increase` | `tf_whip_speed_increase = 0` | Supply no whip speed increase. | TF2 game | Handled |
| `policy.movement-restart-freeze` | `tf_player_movement_restart_freeze = 0` | Request no round-restart movement freeze. | TF2 game | Handled |
| `policy.auto-team-balance` | `mp_autoteambalance = 0` | Disable mode-driven automatic team reassignment. | TF2 game/ruleset composition | Handled |
| `policy.team-unbalance-limit` | `mp_teams_unbalance_limit = 0` | Apply no mode-driven team-size difference limit. | TF2 game/ruleset composition | Handled |
| `policy.teammate-pushaway` | `tf_avoidteammates_pushaway = 0` | Disable TF2 teammate pushaway while retaining the separately accepted collision policy. | TF2 game | Handled |

## Loadout Item Policies

Candidate count: 47. Each identity is an item-definition index. TF2 owns item construction, attributes, entity class, weapon behavior, and loadout slots; Jump supplies only the listed allow-normalize or reject decision.

| Stable identity | Ruleset decision | Required coverage |
|---|---|---|
| `loadout.item.127` | Allow with gameplay attributes removed. | Handled |
| `loadout.item.730` | Allow with gameplay attributes removed. | Handled |
| `loadout.item.228` | Allow with gameplay attributes removed. | Handled |
| `loadout.item.414` | Allow with gameplay attributes removed. | Handled |
| `loadout.item.1104` | Allow with gameplay attributes removed. | Handled |
| `loadout.item.237` | Allow with gameplay attributes removed. | Handled |
| `loadout.item.1085` | Allow with gameplay attributes removed. | Handled |
| `loadout.item.226` | Reject. | Handled |
| `loadout.item.354` | Reject. | Handled |
| `loadout.item.129` | Reject. | Handled |
| `loadout.item.1001` | Reject. | Handled |
| `loadout.item.405` | Allow with gameplay attributes removed. | Handled |
| `loadout.item.608` | Allow with gameplay attributes removed. | Handled |
| `loadout.item.265` | Allow as the canonical pipebomb-launcher entity class with gameplay attributes removed. | Handled |
| `loadout.item.130` | Allow as the canonical pipebomb-launcher entity class with gameplay attributes removed. | Handled |
| `loadout.item.1150` | Allow as the canonical pipebomb-launcher entity class with gameplay attributes removed. | Handled |
| `loadout.item.406` | Reject. | Handled |
| `loadout.item.131` | Reject. | Handled |
| `loadout.item.1099` | Reject. | Handled |
| `loadout.item.1144` | Reject. | Handled |
| `loadout.item.996` | Reject. | Handled |
| `loadout.item.1151` | Reject. | Handled |
| `loadout.item.308` | Reject. | Handled |
| `loadout.item.307` | Reject. | Handled |
| `loadout.item.132` | Allow with gameplay attributes removed. | Handled |
| `loadout.item.482` | Allow with gameplay attributes removed. | Handled |
| `loadout.item.172` | Allow with gameplay attributes removed. | Handled |
| `loadout.item.1082` | Allow with gameplay attributes removed. | Handled |
| `loadout.item.327` | Allow with gameplay attributes removed. | Handled |
| `loadout.item.266` | Allow with gameplay attributes removed. | Handled |
| `loadout.item.1101` | Reject. | Handled |
| `loadout.item.58` | Reject. | Handled |
| `loadout.item.1083` | Reject. | Handled |
| `loadout.item.222` | Reject. | Handled |
| `loadout.item.44` | Reject. | Handled |
| `loadout.item.528` | Reject. | Handled |
| `loadout.item.1121` | Reject. | Handled |
| `loadout.item.1105` | Reject. | Handled |
| `loadout.item.1172` | Reject. | Handled |
| `loadout.item.444` | Reject. | Handled |
| `loadout.item.5869` | Reject. | Handled |
| `loadout.item.1069` | Reject. | Handled |
| `loadout.item.1070` | Reject. | Handled |
| `loadout.item.775` | Allow with gameplay attributes removed. | Handled |
| `loadout.item.415` | Allow with gameplay attributes removed. | Handled |
| `loadout.item.1153` | Allow with gameplay attributes removed. | Handled |
| `loadout.item.357` | Allow with gameplay attributes removed. | Handled |

Every accepted TF2 item definition not listed above is allowed unchanged. Generation joins that default to every item in the accepted TF2 item inventory and fails on an absent classification, unresolved class/slot, or newly discovered explicit policy requirement.

## Owned-Explosive Cleanup

Candidate count: 4.

| Stable identity | TF2 projectile family | Required reset disposition | Required coverage |
|---|---|---|---|
| `cleanup.rocket` | `tf_projectile_rocket` | Remove every projectile owned by the reset player before publishing the reset state. | Handled |
| `cleanup.energy-ball` | `tf_projectile_energy_ball` | Remove every projectile owned by the reset player before publishing the reset state. | Handled |
| `cleanup.stickybomb` | `tf_projectile_pipe_remote` | Remove every projectile thrown by the reset player before publishing the reset state. | Handled |
| `cleanup.pipebomb` | `tf_projectile_pipe` | Remove every projectile thrown by the reset player before publishing the reset state. | Handled |

## Run States And Transitions

Candidate count: 19.

| Stable identity | Triggering input | Required observable disposition | Required coverage |
|---|---|---|---|
| `run.state.idle` | No active run for identity | Retain no start tick, elapsed ticks, or checkpoint visits. | Handled |
| `run.state.running` | Accepted start edge | Retain immutable start tick, eligibility snapshot, validity, and ordered visits. | Handled |
| `run.state.completed` | Accepted end edge | Freeze one result and permit no later mutation of that run instance. | Handled |
| `run.state.invalidated` | Accepted invalidating transition | Freeze reason and tick; never emit a valid completion from that instance. | Handled |
| `transition.map-start` | Enter `zone.map-start` | Start the map run once for this contact episode. | Handled |
| `transition.stage-start` | Enter `zone.course-start` | Start the indexed stage run once for this contact episode. | Handled |
| `transition.bonus-start` | Enter `zone.bonus-start` | Start the indexed bonus run once for this contact episode. | Handled |
| `transition.trick-start` | Enter `zone.trick` | Exact start/end relation is unresolved. | Unknown |
| `transition.checkpoint` | Enter `zone.checkpoint` | Apply the accepted duplicate, backward, skipped, and out-of-run decision. | Unknown |
| `transition.stage-end` | Enter `zone.course-end` | Complete only the matching active stage under accepted checkpoint policy. | Handled |
| `transition.map-end` | Enter `zone.map-end` | Complete the map and final stage only when each required predicate passes. | Handled |
| `transition.bonus-end` | Enter `zone.bonus-end` | Complete only the matching active bonus. | Handled |
| `transition.reset` | Player reset request | Exact destination, save retention, run invalidation, health, velocity, and timer disposition require an accepted target contract. | Unknown |
| `transition.restart` | Player restart request | Exact distinction from reset and save deletion requires an accepted target contract. | Unknown |
| `transition.death` | TF2 death fact | Exact run, save, checkpoint, and restart disposition requires an accepted target contract. | Unknown |
| `transition.respawn` | TF2 respawn result | Exact destination and retained course state require an accepted target contract. | Unknown |
| `transition.map-teleport` | TF2 map-entity teleport fact | Preserve physical velocity; exact run-validity effect requires an accepted target contract. | Unknown |
| `transition.save` | Player save request while saving is enabled | Exact saved fields, eligibility, cardinality, lifetime, and run-validity effect require an accepted target contract. | Unknown |
| `transition.restore` | Player restore request with a valid save | Exact transform, velocity, explosives, timer, and run-validity effect require an accepted target contract. | Unknown |

## Run-Result Outputs

Candidate count: 12.

| Stable identity | Required value | Consumer | Required coverage |
|---|---|---|---|
| `result.course-definition-identity` | Exact immutable course-definition identity | Simulation, Tempus application | Handled |
| `result.map-identity` | Canonical map identity and content hash | Simulation, Tempus application | Handled |
| `result.run-kind` | `map`, `course`, `bonus`, or accepted `trick` | Tempus application | Handled |
| `result.zone-index` | Positive index; map is 1 | Tempus application | Handled |
| `result.player-identity` | Stable Simulation player identity | Simulation, Tempus application | Handled |
| `result.class-identity` | 3 or 4 from the start eligibility snapshot | Tempus application | Handled |
| `result.start-tick` | Unsigned simulation tick | Simulation, Tempus application | Handled |
| `result.end-tick` | Unsigned simulation tick not below start | Simulation, Tempus application | Handled |
| `result.elapsed-ticks` | Exact integer difference under the accepted contact-boundary rule | Simulation, Tempus application | Handled |
| `result.tick-interval-bits` | Exact Simulation binary32 interval bits | Simulation, Tempus application | Handled |
| `result.checkpoint-trace` | Ordered accepted checkpoint identities and visit ticks | Inspector, Tempus application | Handled |
| `result.disposition` | Completed or invalidated with one stable reason | Simulation, Tempus application | Handled |

## Generation Contract

The future generator is owned by `tools/playsrc`. It must consume a checked manifest pinning every authority revision above, the accepted TF2 class/item/cvar/projectile/entity inventories, the accepted TF2 ruleset inventory, and a machine-readable Jump course contract. It emits all 117 identities in section and numeric order and reproduces this file byte-for-byte for identical inputs.

Generation fails on a missing authority, changed public contract hash, duplicate identity, unclassified zone or item, unresolved owner, invalid topology relation, item-schema mismatch, omitted cleanup family, missing result field, count mismatch, or any `Unknown`, `Malformed`, `Missing`, or `Unsupported` required item. Failed items remain visible and are never silently omitted.
