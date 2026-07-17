# Collision

## Sample

```rust
let collision = playsrc_collision::compile(&bsp)?;
let hit = collision.trace_hull(start, end, hull, contents_mask)?;
```

## Objective

Represent collision geometry and answer deterministic spatial queries.

## Responsibilities

- Build queryable world, brush-model, prop, trigger, and model collision shapes.
- Perform point contents, ray traces, hull sweeps, overlaps, and contact queries.
- Preserve Source masks, contents, surfaces, fractions, normals, and solid-state results.
- Validate immutable BSP brush inputs, distinguish model 0 world brushes from non-world brush models through the model head-node leaf set, and sweep points or axis-aligned hulls through Source-space world convex half-spaces with the 1/32-inch brush epsilon.
- Retain one exact brush set per BSP model and test translated model-space hull overlap for Entity-owned trigger contacts without adding inline models to world-solid traces.

## Non-Responsibilities

- Advancing rigid bodies or solving constraints.
- Applying player movement or game rules.
- Deciding visual visibility.

## Relationships

Consumes BSP, PHY, and model geometry; supplies queries to movement, physics, entities, simulation, and inspection tools.

## Completion

Complete when the declared collision shape and query families match observable Source behavior with bounded performance.
