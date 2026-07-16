# Entity

## Objective

Represent generic Source entity identity, relationships, I/O, and lifecycle behavior.

## Responsibilities

- Preserve entity keyvalues, identifiers, parent relationships, references, and ordered outputs.
- Provide generic lifecycle and event primitives shared across games.
- Classify handled, inert, unsupported, malformed, missing, and unknown entity data explicitly.

## Non-Responsibilities

- Implementing TF2 or other game-specific entity classes.
- Owning movement, physics, rendering, or ruleset behavior.
- Treating every classname as generic data when behavior is required.

## Relationships

Consumes parsed entity data from maps; game modules provide game-specific entity implementations; simulation advances active entity state.

## Completion

Complete when the generic entity behavior family and all declared integration seams are represented and fairly verified.
