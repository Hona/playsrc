# TF2 Rulesets

## Objective

Own every declared TF2 mode, modifier, and in-session match policy without duplicating TF2-wide mechanics.

## Responsibilities

- Define mode lifecycle, objectives, scoring, spawning, restrictions, completion, and game-state transitions.
- Assign every accepted map and match-policy selection to exactly one primary ruleset, zero or more disjoint modifiers, and at most one in-session match policy.
- Publish canonical objective, timer, score, roster, round, win, overtime, stalemate, and HUD-state facts for applications and presentation packages.
- Use TF2 mechanics without reimplementing them.
- Keep each ruleset independent from similarly named rulesets in other games.

## Non-Responsibilities

- Generic Source or TF2-wide behavior.
- Match formation, queues, ratings, penalties, credentials, product UI, records services, or hosting.
- Map parsing, generic entity execution, simulation scheduling, networking transport, replay advancement, or presentation.

## Universe

[`ROADMAP.md`](ROADMAP.md) tracks the aggregation contract. [`inventories/rulesets.md`](inventories/rulesets.md) contains the 31-item candidate universe: 25 primary rulesets, 3 modifiers, and 3 in-session match policies. Only accepted owners receive new directories.
