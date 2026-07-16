# Legacy Source 1 CS:GO Behavior-Family Inventory

## Inventory Metadata

| Field | Value |
|---|---|
| Owning roadmap | [`../ROADMAP.md`](../ROADMAP.md) |
| Authority identity | Exact legacy CS:GO `1.38.8.1` target manifest and runtime declarations; final Steam depot manifests; current playsrc Owner Registry; Valve's published CS:GO demo/message contracts; generic Valve Source SDK 2013 seams |
| Authority revision | Engine build `8802`; protocol `13881`; client/server `1575`; Source revision `8413246`; depot `731` manifest `1224088799001669801`; depot `732` manifest `6314304446937576250`; demoinfo `049f8dbf49099d3cc544ec5061a7f7252cce7b82`; SDK `88fa198fba3fb85d46d4c95018254693fdc3af0a`; exact retained target bytes/captures Missing |
| Generator command | Missing |
| Output path | `games/csgo/inventories/behavior-families.md` |
| Candidate item count | 49 |
| Accepted item count | 0 |

This file is a manually derived candidate index, not generated output. Each identity equals one behavior-row identity in the owning roadmap and therefore does not create a second denominator item. Generation must retain the order below and fail on an omitted, duplicate, unowned, or unclassified behavior family.

## Candidate Items

| Stable identity | Legacy CS:GO-owned behavior | Adjacent owner boundary | Status |
|---|---|---|---|
| `csgo.authority-and-content` | Immutable final-build binding and runtime target identity | Content resolves bytes; tools retain manifests and captures. | Blocked |
| `csgo.whole-transition` | One complete game-owned transition and journal | Simulation admits, schedules, and publishes; one ruleset supplies mode decisions. | Blocked |
| `csgo.player-lifecycle` | Join, selection, active, death, observer, reset, respawn, disconnect | Applications own menus; Simulation owns session lifecycle. | Blocked |
| `csgo.teams` | Team/squad selection, balance, relationships, recipients, resources | Rulesets own mode team score and win policy. | Blocked |
| `csgo.player-agents` | Team agent/class selection, model/voice identity, and reset behavior | Presentation packages execute model, voice, and animation requests. | Blocked |
| `csgo.observer-death-respawn` | Game-owned death, observer, respawn, protection, and eligibility facts | Movement owns observer motion; rulesets own spawn permission. | Blocked |
| `csgo.item-schema` | Ordered schema, prefab, item, attribute, finish, sticker, music, and agent semantics | KeyValues parses structure; Content resolves exact bytes. | Blocked |
| `csgo.loadout-inventory-equipment` | Loadouts, item instances, ammo, armor, equipment, pickups, drops | Entity owns generic pickup touch and lifecycle. | Blocked |
| `csgo.economy` | Player/team account ledger, awards, penalties, loss state | Rulesets emit objective and round award facts. | Blocked |
| `csgo.purchasing` | Buy eligibility, loadout alternative selection, transaction, autobuy, rebuy | Applications own buy UI and bindings. | Blocked |
| `csgo.damage-admission` | Game-specific allow/deny and normalized damage identity | Collision, Physics, and Entity provide source facts. | Blocked |
| `csgo.damage-resolution` | Hitgroup, armor, health, force, tagging, flash/deafness, post-hit order | Collision owns traces; Physics owns solver response. | Blocked |
| `csgo.death-attribution` | Lethal result, attribution, drops, rewards, cleanup | Rulesets consume death facts for score and wins. | Blocked |
| `csgo.weapon-common` | Weapon records and common lifecycle/state | Content and parsers supply validated records. | Blocked |
| `csgo.firearms` | Firearm attack, spread, penetration, hit, recoil, and cadence | Collision owns traces; Material owns surface meaning. | Blocked |
| `csgo.firearm-actions` | Reload and specialized alternate-action state machines | Applications capture buttons; presentation executes requests. | Blocked |
| `csgo.shotguns` | Pellet and shell-reload specialization | Collision owns every trace result. | Blocked |
| `csgo.knife-taser` | Knife, fist, tool, and taser attack specialization | Collision owns line/hull query truth. | Blocked |
| `csgo.grenade-common` | Throw and common grenade projectile lifecycle | Physics owns rigid-body/contact response. | Blocked |
| `csgo.he-grenade` | HE targeting, damage, force, and effects | Collision owns visibility; presentation executes effects. | Blocked |
| `csgo.flashbang` | Flash exposure and blindness state | Collision owns visibility; presentation owns screen/audio execution. | Blocked |
| `csgo.smoke-grenade` | Rest, detonation, smoke bloom/volume/lifetime facts | Physics owns contacts; presentation owns particles. | Blocked |
| `csgo.fire-grenades` | Molotov/incendiary projectile, inferno cells, burn, extinguish | Physics/Collision provide contact and visibility facts. | Blocked |
| `csgo.decoy` | Decoy activation, simulated shots, radar facts, final detonation | Presentation executes shot/effect requests. | Blocked |
| `csgo.special-equipment-projectiles` | Danger Zone and accepted special equipment/item behavior | Physics/Entity own generic bodies, contacts, lifecycle, and I/O. | Blocked |
| `csgo.c4` | Carry, drop, plant, timer, defuse, explosion lifecycle | Rulesets own bomb-mode completion and scoring. | Blocked |
| `csgo.accuracy-recoil` | Deterministic inaccuracy, spread, recoil, punch, recovery | Movement supplies stance and motion results. | Blocked |
| `csgo.movement-specialization` | CS:GO speed, stamina, tagging, duck/jump/ladder/water and collision policy | Movement owns generic advancement; rulesets own mode restrictions. | Blocked |
| `csgo.interaction-progress` | Use target and progress-action state | Entity supplies candidates; rulesets supply objective permission. | Blocked |
| `csgo.hostages` | Reusable hostage lifecycle, locomotion requests, and objective facts | Entity/Movement own generic lifecycle and motion; rulesets own rescue completion. | Blocked |
| `csgo.game-entities` | Game entity adapters, zones, items, projectiles, managers | Entity owns generic state, I/O, touch, triggers, parenting, and movers. | Blocked |
| `csgo.objective-primitives` | Reusable scenario entities, resources, survival/course zones, and facts | Rulesets own lifecycle, score, completion, and wins. | Blocked |
| `csgo.spawn-selection` | Game spawn filtering, clearance, selection, and reset | Collision owns clearance; rulesets own spawn permission. | Blocked |
| `csgo.common-rules` | Game-wide policy and legal ruleset request/result seam | Simulation owns tick order; rulesets own mode policy. | Blocked |
| `csgo.bot-ai-navigation` | Bot perception, navigation use, decision, team coordination, and commands | NAV parses bytes; Collision/Movement/Entity provide generic facts. | Blocked |
| `csgo.prediction` | Shared authoritative/predicted transition and restore state | Networking owns acknowledgements and reconciliation mechanics. | Blocked |
| `csgo.replicated-state` | Canonical source values for logical replicated fields | Networking owns tables, encoding, recipients, baselines, and deltas. | Blocked |
| `csgo.network-semantics` | Game meaning of decoded commands, classes, fields, tables, and messages | Networking owns protocol framing, transport, and reconciliation. | Blocked |
| `csgo.replay-interpretation` | Game meaning of decoded recorded state | Demo parses records; Replay advances recorded timelines. | Blocked |
| `csgo.game-events` | Typed event production and disposition | Simulation sequences publication; consumers cannot mutate authority. | Blocked |
| `csgo.user-messages-radio` | Typed user-message and radio production/disposition | Networking owns transport; applications own chat/radio UI. | Blocked |
| `csgo.player-presentation` | Player/agent/model/voice/animation/ragdoll/equipment effect mapping | Rendering, Particle, and Audio execute requests. | Blocked |
| `csgo.weapon-presentation` | Item/weapon/viewmodel/projectile/finish/effect mapping | Presentation packages execute requests. | Blocked |
| `csgo.objective-presentation` | C4/hostage/survival/round/team-resource request mapping | Applications own HUD layout; presentation executes effects. | Blocked |
| `csgo.configuration` | Typed cvar, CFG, map override, weapon/item parameter snapshot | Console-script/KeyValues parsers supply structure; rulesets own mode values. | Blocked |
| `csgo.arithmetic-failures` | Numeric profile, random contract, limits, classifications, atomic failure | Generic packages own their arithmetic and resource bounds. | Blocked |
| `csgo.ruleset-ownership` | Complete routing of every mode-only behavior to one ruleset | The legacy CS:GO ruleset universe owns each mode. | Blocked |
| `csgo.current-interface-integration` | One current interface across all producers and consumers | Each adjacent module retains its registered behavior ownership. | Blocked |
| `csgo.declaration-coverage` | Stable ownership and classification for every encountered declaration | Tools generate inventory; the root registry resolves owners. | Blocked |

## Acceptance Blockers

- Exact retained bytes, file indexes/hashes, and controlled captures for the target depot manifests are Missing.
- No checked-in generator emits this file from declared target inputs.
- The NAV format owner, ruleset universe, generic interfaces, arithmetic profile, and denominator review are not accepted.
