# playsrc

`playsrc` is a modular Source 1 tooling, runtime, and application monorepo.

TF2 is the first complete parity target. CS:S and legacy CS:GO are planned through game adapters. Source 2 is explicitly out of scope.

This repository is currently private during its architecture bootstrap and is intended to become public.

## Status

The repository currently contains structure, documentation, and parity roadmaps only. Existing PoC implementations will be migrated module by module after package boundaries are agreed.

## Documents

- [Architecture](ARCHITECTURE.md)
- [Principles](PRINCIPLES.md)
- [Roadmap](ROADMAP.md)
- [Contributing](CONTRIBUTING.md)
- [Terminology](TERMINOLOGY.md)

## Scope

- Reusable Source 1 format and content packages.
- Map, model, material, entity, physics, visibility, simulation, rendering, audio, particle, and demo modules.
- TF2, CS:S, and legacy CS:GO game adapters.
- Jump, surf, bhop, KZ, and competitive rulesets.
- Web applications, services, repeatable pipelines, and deployment definitions.

Packages must remain independently useful. Applications assemble packages and do not reimplement them.
