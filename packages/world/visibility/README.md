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
- Preserve first-visible-leaf/leaf-face world-surface candidate order independently from material, alpha, cull, LOD, or draw policy; fixed `jump_beef` view identities live in `inventories/jump-beef-view.md`.
- Accept Map-supplied displacement parent-face AABBs, push each through model-zero BSP nodes with Source touching-plane behavior, and merge its source-ordered leaf references into PVS/frustum world-surface output without visibility inferring displacement geometry.
- Keep the caller-supplied PVS origin distinct from the render-frustum camera and apply an optional exact area filter. PVIS v4 uses this contract for authored 3D-sky views without all-visible fallback.
- Retain bounded immutable cluster/ancestor indexes, exact portal- and origin-qualified view results, and dense traversal scratch so repeated views preserve every ordered visibility output without rescanning the complete BSP.

## Owner Diagnostics

`bun packages/world/visibility/scripts/profile.ts jump_beef` profiles the exact configured map; replace the logical target with `pl_upward` for build `24245096`. Each command verifies the declared BSP identity and writes bounded native visibility-phase samples and output hashes under configured `sourceCacheDir`. Native phase reports do not replace headed Chromium gameplay evidence.

## Non-Responsibilities

- Drawing visible objects or owning GPU occlusion queries.
- Defining gameplay relevance or network replication policy.
- Parsing unrelated BSP semantics.

## Relationships

Consumes BSP visibility data and dynamic world state; supplies visibility decisions to rendering and runtime modules.

## Completion

Complete when the declared Source visibility families are represented and verified independently of presentation.
