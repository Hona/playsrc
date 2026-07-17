# TF2

## Objective

Implement the complete Team Fortress 2 game over generic Source modules.

## Responsibilities

- Own TF2 players, classes, items, attributes, weapons, projectiles, buildings, conditions, teams, and damage behavior.
- Own TF2 movement differences, prediction state, replication schemas, entities, objectives, game rules, and game-specific presentation mappings.
- Provide the game behavior used by every TF2 ruleset and product.
- Declare exact TF2 content-provider targets, including the immutable `jump_beef` download identity consumed by local development.
- Advance a deterministic local Soldier/Demoman jump-combat session with stock rocket/Original and stickybomb state, projectiles, self-damage, blast impulse, class health, respawn, and ordered presentation events over generic Movement and Collision.
- Expose the local TF2 session to browser workers through one bounded fixed-tick WASM phase and a compact generation-bound binary snapshot containing player, projectile, and ordered presentation-event state.
- Stage and atomically activate direct-BSP browser sessions through a game-owned module worker; transfer canonical map bytes only on a verified derived-cache miss.

## Non-Responsibilities

- Generic Source parsing, collision, physics, movement, simulation, networking, or presentation behavior.
- Rules belonging to one TF2 mode or community ruleset.
- Tempus records, rankings, UI, or other product behavior.

## Relationships

Composes packages and supplies game behavior to [`rulesets/`](rulesets/), TF2 applications, replay, and future gameplay servers.

## Completion

Complete when the declared TF2 behavior universe is handled with credible evidence across gameplay, prediction, replay interpretation, and presentation integration.
