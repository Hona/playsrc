# Presentation Packages

## Objective

Present canonical world, gameplay, and replay state without owning authority.

## Responsibilities

- Own visual, particle, and audio presentation behavior.
- Consume immutable assets and presentation state through explicit interfaces.
- Keep browser and GPU resource lifetime out of parsers and gameplay modules.

## Non-Responsibilities

- Advancing gameplay or replay truth.
- Reconstructing missing authoritative state.

Each child is an independently useful package. This directory is only a navigational group.
