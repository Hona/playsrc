# TF2

## Objective

Implement the complete Team Fortress 2 game over generic Source modules.

## Responsibilities

- Own TF2 players, classes, items, attributes, weapons, projectiles, buildings, conditions, teams, and damage behavior.
- Own TF2 movement differences, prediction state, replication schemas, entities, objectives, game rules, and game-specific presentation mappings.
- Provide the game behavior used by every TF2 ruleset and product.
- Declare exact TF2 content-provider targets, including the immutable `jump_beef` download identity consumed by local development.

## Non-Responsibilities

- Generic Source parsing, collision, physics, movement, simulation, networking, or presentation behavior.
- Rules belonging to one TF2 mode or community ruleset.
- Tempus records, rankings, UI, or other product behavior.

## Relationships

Composes packages and supplies game behavior to [`rulesets/`](rulesets/), TF2 applications, replay, and future gameplay servers.

## Completion

Complete when the declared TF2 behavior universe is handled with credible evidence across gameplay, prediction, replay interpretation, and presentation integration.
