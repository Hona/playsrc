# Collision

## Sample

```ts
import { createCollisionWorld } from "@playsrc/collision"

const collision = createCollisionWorld(map)
const hit = collision.traceRay(start, end)
```

```rust
let collision = playsrc_collision::World::from_map(&map)?;
let hit = collision.trace_ray(start, end);
```

## Objective

Represent collision geometry and answer deterministic spatial queries.

## Responsibilities

- Build queryable world, brush-model, prop, trigger, and model collision shapes.
- Perform point contents, ray traces, hull sweeps, overlaps, and contact queries.
- Preserve Source masks, contents, surfaces, fractions, normals, and solid-state results.

## Non-Responsibilities

- Advancing rigid bodies or solving constraints.
- Applying player movement or game rules.
- Deciding visual visibility.

## Relationships

Consumes BSP, PHY, and model geometry; supplies queries to movement, physics, entities, simulation, and inspection tools.

## Completion

Complete when the declared collision shape and query families match observable Source behavior with bounded performance.
