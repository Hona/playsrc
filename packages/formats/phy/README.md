# PHY

## Objective

Parse Source 1 PHY resources into runtime-neutral collision asset data.

## Responsibilities

- Validate serialized physics headers, solids, constraints, and associated metadata.
- Represent collision geometry and physical properties without runtime-engine assumptions.
- Preserve unsupported records explicitly.

## Non-Responsibilities

- Advancing rigid bodies or solving constraints.
- Performing world traces or player movement.
- Applying game-specific prop behavior.

## Relationships

Supplies decoded physics assets to `collision`, `physics`, map compilation, and game modules.

## Completion

Complete when the declared PHY behavior family is bounded, represented, and verified independently of a physics runtime.
