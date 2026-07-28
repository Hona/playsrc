# Collision

## Sample

```rust
let collision = playsrc_collision::compile(&bsp)?;
let hit = collision.trace_hull(start, end, hull, contents_mask)?;
```

`Snapshot::compile` adds an immutable ordered dynamic revision. `trace_snapshot_ray` and `trace_snapshot_hull` consume typed request records, explicit Source trace scope, ignored identities, and a pure game-supplied predicate.

## Objective

Represent collision geometry and answer deterministic spatial queries.

## Responsibilities

- Build queryable world, brush-model, prop, trigger, and model collision shapes.
- Perform point contents, ray traces, hull sweeps, overlaps, and contact queries.
- Preserve Source masks, contents, surfaces, fractions, normals, and solid-state results.
- Retain every BSP displacement patch's source/parent/power/contents, vector-distance samples, and triangle tags in the immutable world identity; displacement ray, hull, overlap, and initial-overlap execution remains explicitly unimplemented.
- Validate immutable BSP brush inputs, distinguish model 0 world brushes from non-world brush models through the model head-node leaf set, retain each model's ORed contents and one SHA-256 world identity, and sweep points or axis-aligned hulls through Source-space world convex half-spaces with the 1/32-inch brush epsilon.
- Retain one exact brush set per BSP model and test translated model-space hull overlap for Entity-owned trigger contacts without adding inline models to world-solid traces.
- Compile bounded immutable snapshots containing transformed inline brush models, world-aligned or oriented boxes, and supplied PHY polygon compounds. Every public immutable record retains stable identity, entity/static-prop role, enabled state, transform, conservative world bounds, linear/angular velocity, collision group, exact shape-derived contents, and surface flags; snapshot bytes and results bind the collision-world identity and monotonic revision.
- Trace world brushes in near-leaf and encoded leaf-brush order, clip the entity segment to the world result, reject dynamically disjoint records through conservative swept bounds, preserve accepted record order and strict closer-hit replacement, and retain transformed model/prop/entity feature identity. `Trace::is_sky`, `Trace::entity_identity`, and `Trace::hit_world` expose the generic facts required by projectile consumers without selecting game damage targets.
- Execute one bounded atomic model-lighting ray batch under fixed `MASK_OPAQUE`, caller-supplied sample-set/ray identities, world-only or world-plus-static-prop admission, exact self exclusion, existing nearest-hit ordering, cancellation, and versioned comparison bytes. Lighting owns spherical sample generation, light selection, color, and accumulation.
- Serialize `CSNP` version 2 comparison records only within the snapshot byte limit. Shape, object, convex, vertex, triangle, axis, candidate, ignored-identity, and output bytes have explicit nonzero limits.
- Produce bounded trigger-selected enter/stay/exit edges from exact transformed brush, AABB or OBB objects and subject hulls. `ContactSnapshot` version 1 retains geometric overlap pairs plus prior subject/trigger transforms so moving-trigger and swept-subject crossings remain collision facts; Entity alone evaluates filters and owns accepted touch state/I/O.

## Non-Responsibilities

- Advancing rigid bodies or solving constraints.
- Applying player movement or game rules.
- Deciding visual visibility.

## Relationships

Consumes BSP, PHY, and model geometry; supplies queries to movement, physics, entities, simulation, and inspection tools.

`bun packages/world/collision/scripts/verify-parity.ts` validates local configuration and runs package, configured `jump_beef` BSP, transformed mover, enabled solid-divider standing/crouched hull, explicit never-solid alpha-fence, sky-surface, model-PHY, WASM compilation, formatting, and stable-Clippy evidence.

## Completion

Complete when the declared collision shape and query families match observable Source behavior with bounded performance.
