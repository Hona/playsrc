# TF2 Jump

## Objective

Define how TF2 mechanics form a jump course and timed run.

## Responsibilities

- Own course, zone, checkpoint, timer, reset, start, completion, and run-validity rules.
- Define jump-specific restrictions and permitted TF2 state transitions.
- Consume TF2 rocket, stickybomb, damage, impulse, class, loadout, and movement behavior.
- Validate only authority-complete course definitions, consume ordered Entity entry/stay/exit facts, and publish per-player run state without deriving zones from BSP classnames, targetnames, entity order, or filenames.
- Request TF2-owned projectile cleanup, respawn, class/loadout policy, and movement mode through typed adapters; retain every physical calculation in its owner.
- Pin exact target identities separately from behavior. [`inventories/jump-beef.md`](inventories/jump-beef.md) records all 49 public `jump_beef` zone identities and the missing geometry/contact contracts.

## Non-Responsibilities

- Implementing TF2 weapons, explosions, player movement, or collision.
- Defining surf, bhop, or KZ behavior for another game.
- Owning Tempus records, rankings, APIs, or UI.

## Relationships

Composes TF2 with generic simulation and world packages; the Tempus application adds product behavior around this ruleset.

## Completion

Complete when the declared TF2 jump behavior family and course lifecycle are implemented and supported by credible gameplay evidence.

## Current Status

Tempus course parity is Blocked. Public contracts expose `jump_beef` map/course/bonus/checkpoint/trick identities but omit zone bounds, contact mappings, timer boundaries, checkpoint scope, reset/restart, save/restore, validity, and completion ordering. The existing synthetic linear runtime is not Tempus evidence and cannot be configured from BSP hint/text triggers.
