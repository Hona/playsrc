# Entity

## Sample

```rust
let graph = playsrc_entity::parse(entity_lump_bytes, limits)?;
let (mut world, startup) = playsrc_entity::EntityWorld::compile(&graph, config)?;
let transitions = world.phase(tick, &ordered_commands)?;
```

## Objective

Represent generic Source entity identity, relationships, I/O, and lifecycle behavior.

## Responsibilities

- Preserve entity keyvalues, identifiers, parent relationships, references, and ordered outputs.
- Provide generic lifecycle and event primitives shared across games.
- Own bounded live slot/generation handles, ordered target resolution, hierarchy transforms, delayed I/O, generic mover/logic/filter/trigger state, and canonical snapshots behind one Entity phase interface.
- Emit typed mover, brush-state, trigger-effect, and block-damage requests without executing Movement, Collision, game, Jump, or presentation behavior.
- Classify handled, inert, unsupported, malformed, missing, and unknown entity data explicitly.
- Parse bounded BSP entity text into ordered duplicate-preserving definitions, first-field structural views, brush-model identities, and parsed or malformed output actions.
- Compile enabled client `trigger_teleport` definitions against exact translated Collision brush models, resolve ordered `info_teleport_destination` targets, and return position/yaw changes while leaving velocity with the game/movement owner.

## Non-Responsibilities

- Implementing TF2 or other game-specific entity classes.
- Owning movement, physics, rendering, or ruleset behavior.
- Treating every classname as generic data when behavior is required.

## Relationships

Consumes parsed entity data and caller-supplied brush bounds; Collision supplies ordered contacts; Simulation advances one Entity phase; Movement and game modules execute typed requests and return completion/input records.

## Completion

Complete when the generic entity behavior family and all declared integration seams are represented and fairly verified.
