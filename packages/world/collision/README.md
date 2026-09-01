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
- Compile attached full-grid displacement positions and unfiltered Source-order triangles into immutable per-patch and per-triangle bounds; execute one-sided rays, swept axis-aligned hulls and stationary hull overlap through the world query authority while retaining source, parent face, triangle, flags, contents and resolved surface identity. The stationary point contract remains non-solid.
- Validate immutable BSP brush inputs, distinguish model 0 world brushes from non-world brush models through the model head-node leaf set, retain each model's ORed contents and one SHA-256 world identity, and sweep points or axis-aligned hulls through Source-space world convex half-spaces with the 1/32-inch brush epsilon.
- Construct source-ordered immutable world and inline physical solids from BSP-owned lump-29 frames and PHY-owned decoded geometry/keydata without replacing ordinary brush traces.
- Accept explicitly supplied authored convex topology and physical metadata through `PhysicsShape::compile_authored`; generic geometry never acquires invented source properties.
- Retain one exact brush set per BSP model and test translated model-space hull overlap for Entity-owned trigger contacts without adding inline models to world-solid traces.
- Compile bounded immutable snapshots containing transformed inline brush models, world-aligned or oriented boxes, and supplied PHY polygon compounds. Every public immutable record retains stable identity, entity/static-prop role, enabled state, transform, conservative world bounds, linear/angular velocity, collision group, exact shape-derived contents, and surface flags; snapshot bytes and results bind the collision-world identity and monotonic revision. Every authored PHY shape retains its exact binary32 center, inertia, radius, optional three-axis drag fractions, surface-deviation byte, internal-space convex points, ordered complete 16-byte triangle records, and three packed directed-edge words for Physics; generic geometry never fabricates topology or physical source properties.
- Trace world brushes in near-leaf and encoded leaf-brush order, clip the entity segment to the world result, reject dynamically disjoint records through conservative swept bounds, preserve accepted record order and strict closer-hit replacement, and retain transformed model/prop/entity feature identity. `Trace::is_sky`, `Trace::entity_identity`, and `Trace::hit_world` expose the generic facts required by projectile consumers without selecting game damage targets.
- Execute one bounded atomic model-lighting ray batch under fixed `MASK_OPAQUE`, caller-supplied sample-set/ray identities, world-only or world-plus-static-prop admission, exact self exclusion, existing nearest-hit ordering, cancellation, and versioned comparison bytes. Lighting owns spherical sample generation, light selection, color, and accumulation.
- Accept every map displacement exactly once as immutable full-grid positions, unfiltered Source-order triangles/tags, contents and stable primary/secondary surface identities; include both the handoff and derived collision authority in world identity publication.
- Build static-prop snapshot objects only from authored solid disposition and checksum-independent loaded PHY geometry; non-solid and missing-PHY records remain absent rather than deriving collision from render meshes.
- Serialize `CSNP` version 4 comparison records only within the snapshot byte limit, including separate follower collider identities and parent hit identities. Shape, object, convex, vertex, triangle, axis, candidate, ignored-identity, and output bytes have explicit nonzero limits.
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
