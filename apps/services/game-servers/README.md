# Game Server Service

## Objective

Operate future hosted gameplay server processes without owning game behavior.

## Responsibilities

- Own allocation, startup, health, draining, termination, and session attachment for gameplay servers.
- Select immutable game and asset roots for each server process.
- Expose operational state to matchmaking and administration.

## Non-Responsibilities

- Implementing simulation, networking semantics, games, or rulesets.
- Provisioning the underlying compute platform.

## Completion

Complete when declared server lifecycle behavior is bounded, observable, and preserves one gameplay authority per session.
