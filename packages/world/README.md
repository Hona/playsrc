# World Packages

## Objective

Represent and query a runtime-neutral Source 1 world after format parsing.

## Responsibilities

- Own semantic map, material, entity, collision, and visibility behavior.
- Provide representations usable by compilation, gameplay, replay, and presentation.
- Keep game-specific behavior in `games`.

## Non-Responsibilities

- Parsing unrelated Source file containers.
- Owning gameplay authority or GPU presentation.

Each child is an independently useful package. This directory is only a navigational group.
