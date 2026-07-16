# playsrc

`playsrc` is a modular Source 1 tooling, runtime, and application monorepo.

TF2 is the first complete parity target. Counter-Strike: Source and legacy Source 1 Counter-Strike: Global Offensive follow through their own game modules. Source 2 is explicitly out of scope.

This repository is currently private during architecture and migration work and is intended to become public.

## Status

The repository currently defines its module ownership and architecture. Existing proof-of-concept behavior has not yet been migrated into the new packages.

## Structure

```text
packages/     independently useful Source modules
games/        game behavior and game-owned rulesets
apps/         deployed web and service applications
tools/        developer and operator programs
infra/        hosting resources and environments
docs/         cross-cutting public documentation
```

Packages are grouped by mental model:

```text
formats/       Source files and recorded data
world/         runtime-neutral world representation
runtime/       movement, physics, simulation, networking, and replay
presentation/  rendering, particles, and audio
content/       raw Source logical-path resolution
asset-store/   immutable compiled playsrc output
```

Every module README defines its objective, responsibilities, non-responsibilities, relationships, and completion criteria before implementation begins.

## Documents

- [Architecture](ARCHITECTURE.md)
- [Principles](PRINCIPLES.md)
- [Roadmap](ROADMAP.md)
- [Contributing](CONTRIBUTING.md)
- [Terminology](TERMINOLOGY.md)

## Scope

- Reusable Source 1 format, world, runtime, and presentation packages.
- Complete TF2 game behavior and game-owned rulesets, beginning with TF2 jump.
- Future CS:S and legacy Source 1 CS:GO games with their own rulesets.
- Browser products, future online multiplayer services, tools, asset publication, and infrastructure.

Applications assemble modules. They do not reimplement Source, game, or ruleset behavior.
