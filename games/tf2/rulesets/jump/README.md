# TF2 Jump

## Objective

Define how TF2 mechanics form a jump course and timed run.

## Responsibilities

- Own course, zone, checkpoint, timer, reset, start, completion, and run-validity rules.
- Define jump-specific restrictions and permitted TF2 state transitions.
- Consume TF2 rocket, stickybomb, damage, impulse, class, loadout, and movement behavior.

## Non-Responsibilities

- Implementing TF2 weapons, explosions, player movement, or collision.
- Defining surf, bhop, or KZ behavior for another game.
- Owning Tempus records, rankings, APIs, or UI.

## Relationships

Composes TF2 with generic simulation and world packages; the Tempus application adds product behavior around this ruleset.

## Completion

Complete when the declared TF2 jump behavior family and course lifecycle are implemented and supported by credible gameplay evidence.
