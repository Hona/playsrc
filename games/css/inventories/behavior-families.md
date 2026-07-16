# Counter-Strike: Source Behavior-Family Inventory

## Inventory Metadata

| Field | Value |
|---|---|
| Owning roadmap | [`../ROADMAP.md`](../ROADMAP.md) |
| Authority identity | Selected CS:S content/runtime declaration manifest, Valve application `240`, current playsrc Owner Registry, and generic Valve Source SDK 2013 seams |
| Authority revision | CS:S build `Unknown`; SDK `88fa198fba3fb85d46d4c95018254693fdc3af0a`; playsrc `1c2184c11353a618d8ef941c7f161e488346203e` |
| Generator command | Missing |
| Output path | `games/css/inventories/behavior-families.md` |
| Candidate item count | 42 |
| Accepted item count | 0 |

This file is a manually derived candidate index, not generated output. Each identity equals one behavior-row identity in the owning roadmap and therefore does not create a second denominator item. Generation must retain the order below and fail on an omitted, duplicate, unowned, or unclassified behavior family.

## Candidate Items

| Stable identity | CS:S-owned behavior | Adjacent owner boundary | Status |
|---|---|---|---|
| `css.authority-and-content` | Immutable game/content build binding and runtime target identity | Content resolves bytes; tools retain manifests and captures. | Blocked |
| `css.whole-transition` | One complete game-owned state transition and journal | Simulation admits/schedules/publishes; a selected ruleset supplies mode decisions. | Blocked |
| `css.player-lifecycle` | Join, selection, active, death, observer, reset, respawn, disconnect | Applications own menus; Simulation owns session lifecycle. | Blocked |
| `css.teams` | Team selection, balance, relationships, team resources | Rulesets own mode team score and win policy. | Blocked |
| `css.player-classes` | Team class/model selection and class reset behavior | Presentation packages execute model and animation requests. | Blocked |
| `css.observer-death-respawn` | Game-owned death/observer/respawn state and eligibility facts | Movement owns observer motion; rulesets own spawn permission. | Blocked |
| `css.inventory-ammo-equipment` | Inventory, slots, ammo, armor, equipment, pickups, drops | Entity owns generic pickup touch/lifecycle. | Blocked |
| `css.economy` | Player/team account ledger, rewards, penalties, loss state | Rulesets emit objective/round award facts. | Blocked |
| `css.purchasing` | Buy eligibility, transaction, autobuy, rebuy | Applications own buy UI and bindings. | Blocked |
| `css.damage-admission` | Game-specific allow/deny and normalized damage identity | Collision/Physics/Entity provide source facts. | Blocked |
| `css.damage-resolution` | Hitgroup, armor, health, force, flash/deafness, post-hit order | Collision owns traces; Physics owns solver response. | Blocked |
| `css.death-attribution` | Lethal result, attribution, drops, rewards, cleanup | Rulesets consume death facts for score/win decisions. | Blocked |
| `css.weapon-common` | Weapon records and common lifecycle/state | Content and parsers supply validated records. | Blocked |
| `css.firearms` | Firearm attack, spread, penetration, hit and recoil result | Collision owns traces; Material owns surface meaning. | Blocked |
| `css.firearm-actions` | Reload and specialized alternate-action state machines | Applications capture buttons; presentation executes requests. | Blocked |
| `css.shotguns` | Pellet and shell-reload specialization | Collision owns each trace result. | Blocked |
| `css.knife` | Slash/stab query, timing, damage, hit/miss specialization | Collision owns line/hull query truth. | Blocked |
| `css.grenade-common` | Throw and common grenade projectile lifecycle | Physics owns rigid-body/contact response. | Blocked |
| `css.he-grenade` | HE detonation targeting, damage, force, effects | Collision owns visibility; presentation executes effects. | Blocked |
| `css.flashbang` | Flash exposure and blindness state | Collision owns visibility; presentation owns screen/audio execution. | Blocked |
| `css.smoke-grenade` | Rest, detonation, smoke volume/lifetime facts | Physics owns contacts; presentation owns particles. | Blocked |
| `css.c4` | Carry, drop, plant, timer, defuse, explosion lifecycle | Rulesets own bomb-mode completion and scoring. | Blocked |
| `css.accuracy-recoil` | Deterministic inaccuracy, spread, recoil, punch, recovery | Movement supplies stance/motion result. | Blocked |
| `css.movement-specialization` | CS:S speed, stamina, duck/jump/ladder and collision policy | Movement owns generic advancement; rulesets own mode restrictions. | Blocked |
| `css.interaction-progress` | Use target and progress-action state | Entity supplies use candidates; rulesets supply objective permission. | Blocked |
| `css.hostages` | Reusable hostage entity/lifecycle/objective facts | Entity/Movement own generic lifecycle and locomotion; rulesets own rescue completion. | Blocked |
| `css.game-entities` | CS:S entity adapters, zones, items, projectiles, managers | Entity owns generic state, I/O, touch, triggers, parenting, movers. | Blocked |
| `css.objective-primitives` | Reusable scenario entities, resources, and facts | Rulesets own lifecycle, score, completion, and wins. | Blocked |
| `css.spawn-selection` | Game spawn-point filtering, clearance, selection, reset | Collision owns clearance; rulesets own spawn permission/restriction. | Blocked |
| `css.common-rules` | CS:S-wide policy and legal ruleset request/result seam | Simulation owns tick order; rulesets own mode policy. | Blocked |
| `css.prediction` | Shared authoritative/predicted game transition and restore state | Networking owns acknowledgements and reconciliation mechanics. | Blocked |
| `css.replicated-state` | Canonical source values for CS:S logical fields | Networking owns tables, encoding, recipients, baselines, deltas. | Blocked |
| `css.replay-interpretation` | CS:S semantic mapping of decoded recorded state | Demo parses records; Replay advances recorded timelines. | Blocked |
| `css.game-events` | Typed game-event production and disposition | Simulation sequences publication; consumers cannot mutate authority. | Blocked |
| `css.user-messages-radio` | Typed user-message/radio production and disposition | Networking owns wire transport; applications own chat/radio UI. | Blocked |
| `css.player-presentation` | Player/model/animation/ragdoll/equipment effect mapping | Rendering, Particle, and Audio execute requests. | Blocked |
| `css.weapon-presentation` | Weapon/viewmodel/projectile/effect mapping | Presentation packages execute requests. | Blocked |
| `css.objective-presentation` | C4/hostage/round/team-resource request mapping | Applications own HUD layout; presentation packages execute effects. | Blocked |
| `css.arithmetic-failures` | Numeric profile, limits, classifications, atomic failure | Generic packages own their arithmetic and resource bounds. | Blocked |
| `css.ruleset-ownership` | Complete routing of every mode-only behavior to one ruleset | CS:S ruleset universe owns each mode. | Blocked |
| `css.current-interface-integration` | One current interface across all producers and consumers | Each adjacent module retains its registered behavior ownership. | Blocked |
| `css.declaration-coverage` | Stable ownership and coverage classification for every encountered declaration | Tools generate inventory; root registry resolves ownership conflicts. | Blocked |

## Acceptance Blockers

- The selected CS:S content build and immutable runtime target identity are Unknown.
- No checked-in generator emits this file from declared inputs.
- No denominator review records `Accepted`, reviewer identity, review date, reviewed commit, and passing results for all eight review predicates.
