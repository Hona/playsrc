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
- Own bounded live slot/generation handles, typed field projection/writable inputs, complete Source variant conversion, ordered target resolution, attachment hierarchy transforms, delayed I/O, point-template composition/instances, generic timer/mover/logic/filter/trigger state, selected-game external class/disposition bindings, health/spawnflag-addressed button damage with Source output/state/context ordering, class-specific mover completion contexts, and canonical version-5 snapshots behind one Entity phase interface.
- Emit typed mover, brush-state, trigger-effect, block-damage, and game-input requests without executing Movement, Collision, game, Jump, or presentation behavior.
- Classify handled, inert, unsupported, malformed, missing, and unknown entity data explicitly.
- Parse bounded BSP entity text into ordered duplicate-preserving definitions, first-field structural views, brush-model identities, and parsed or malformed output actions.
- Publish immutable source-order brush-model presentation snapshots at an expected Entity revision, retaining current model, composed local/world transforms, parent, render mode, RGBA color, render FX, effect flags, draw admission, and mover progress/request state; selected-game registries declare external brush classes' initial visibility without generic classname inference.
- Keep draw admission independent from solidity: `func_brush` toggles update no-draw state, triggers remain hidden, non-solid movers remain drawable, and broken brushes remain drawable until their scheduled removal.
- Retain current transforms and typed requests for linear/rotating buttons and doors, linear movers, continuous rotators, translating/rotating platforms, corner trains and track trains; generic breakable health, dynamic-prop draw/collision/animation, and registered ordinary-pickup respawn/materialization state remain game-neutral.
- Generate the selected fixed SDK/configured-map inventory with `cargo run -p playsrc-entity --bin generate-source-map-inventory`; the command reads only `playsrc.local.json` and the exact declared source-cache object.
- Run `bun packages/world/entity/scripts/verify-parity.ts` for repeated inventory generation, all three owned package suites, configured BSP/contact/mover evidence, formatting, and stable Clippy with warnings denied.

## Non-Responsibilities

- Implementing TF2 or other game-specific entity classes.
- Owning movement, physics, rendering, or ruleset behavior.
- Treating every classname as generic data when behavior is required.

## Relationships

Consumes parsed entity data and caller-supplied brush bounds; Collision supplies ordered contacts; Simulation advances one Entity phase; Movement and game modules execute typed requests and return completion/input records.

## Completion

Complete when the generic entity behavior family and all declared integration seams are represented and fairly verified.

## Licensing

`rust/src/source_random.rs` adapts the official Source 1 SDK uniform random-stream algorithm and is subject to [`SOURCE-1-SDK-LICENSE.txt`](SOURCE-1-SDK-LICENSE.txt), Valve's copyright notice, and the checked [`thirdpartylegalnotices.txt`](../../presentation/particle/thirdpartylegalnotices.txt). Other files in this checkpoint are independently authored from public behavior contracts and remain under the repository MIT license.
