# Inspector

## Objective

Provide one interactive environment for understanding Source input and playsrc output.

## Responsibilities

- Inspect maps, materials, models, entities, collision, visibility, simulation, replay, particles, audio, and frames.
- Display package-owned state and evidence without creating alternate interpretations.
- Support bounded diagnostics through the same interfaces used by products.

## Non-Responsibilities

- Becoming gameplay, replay, material, or rendering authority.
- Hiding unsupported behavior through inspector-only fixes.
- Owning automated regression infrastructure.

## Completion

Complete when declared module state can be inspected deeply enough to diagnose behavior without entering unrelated implementations.
