# Format Packages

## Objective

Interpret Source 1 files and recorded streams as bounded typed data.

## Responsibilities

- Validate format identifiers, versions, offsets, counts, and limits.
- Preserve information required by later semantic modules.
- Report malformed, unsupported, and unknown input explicitly.

## Non-Responsibilities

- Gameplay, game-specific behavior, rendering, and product behavior.
- Finding logical resources across mounted content.
- Publishing compiled artifacts.

Each child is an independently useful package. This directory is only a navigational group.
