# Simulation

## Objective

Provide the single deterministic authority that advances composed Source gameplay state.

## Responsibilities

- Own tick progression, command submission, scheduling, state transitions, events, and snapshots.
- Coordinate world, entity, movement, physics, game, and ruleset behavior in a defined order.
- Run the same gameplay implementation in browser workers and native servers.

## Non-Responsibilities

- Reimplementing movement, physics, entities, games, or rulesets.
- Owning network transport, replay authority, or presentation.
- Calling WASM once per tiny domain operation.

## Relationships

Composes world and runtime packages with one game and one game-owned ruleset; networking transports its commands and snapshots.

## Completion

Complete when gameplay advances through one bounded authority with deterministic ordering and coarse native/WASM interfaces.
