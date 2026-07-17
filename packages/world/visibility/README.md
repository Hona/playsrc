# Visibility

## Sample

```rust
let visibility = playsrc_visibility::compile(&bsp)?;
let leaf = visibility.locate_leaf(camera_position)?;
let visible = visibility.visible(from_cluster, to_cluster);
let leaves = visibility.leaves_in_box(bounds)?;
let result = visibility.view(&area_state, &candidates, &query)?;
```

## Objective

Determine potentially visible Source world state independently of a renderer.

## Responsibilities

- Represent BSP leaves, clusters, visibility sets, areas, portals, and occluders.
- Evaluate declared visibility state and map-provided visibility data.
- Supply bounded visibility results to presentation and networking consumers.
- Expand Source zero-run PVS/PAS rows and traverse the immutable BSP plane/node tree without selecting renderer draw policy.
- Enumerate complete AABB leaf memberships, maintain explicit area-portal connectivity revisions, join model/prop/dynamic/detail/entity candidates, and emit front-to-back view results with deterministic cache identities.

## Non-Responsibilities

- Drawing visible objects or owning GPU occlusion queries.
- Defining gameplay relevance or network replication policy.
- Parsing unrelated BSP semantics.

## Relationships

Consumes BSP visibility data and dynamic world state; supplies visibility decisions to rendering and runtime modules.

## Completion

Complete when the declared Source visibility families are represented and verified independently of presentation.
