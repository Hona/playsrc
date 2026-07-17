# TF2 Jump

## Objective

Define how TF2 mechanics form a jump course and timed run.

## Responsibilities

- Own course, zone, checkpoint, timer, reset, start, completion, and run-validity rules.
- Define jump-specific restrictions and permitted TF2 state transitions.
- Consume TF2 rocket, stickybomb, damage, impulse, class, loadout, and movement behavior.
- Validate explicit map-bound linear course definitions, consume ordered Entity entry/stay/exit facts, and publish per-player start/checkpoint/completion/invalidation state and integer-tick results.
- Request TF2-owned projectile cleanup and respawn atomically for reset; invalidate active runs on respawn, eligibility loss, or noclip; preserve run identity across ordinary map teleports.
- Select the 900-health Soldier policy and ruleset-approved noclip through TF2/Movement adapters; retain stock Demoman health and all physical calculations in their owners.

## Non-Responsibilities

- Implementing TF2 weapons, explosions, player movement, or collision.
- Defining surf, bhop, or KZ behavior for another game.
- Owning Tempus records, rankings, APIs, or UI.

## Relationships

Composes TF2 with generic simulation and world packages; the Tempus application adds product behavior around this ruleset.

## Completion

Complete when the declared TF2 jump behavior family and course lifecycle are implemented and supported by credible gameplay evidence.
