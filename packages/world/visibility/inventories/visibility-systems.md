# Visibility Systems Inventory

Owning roadmap: [`../ROADMAP.md`](../ROADMAP.md)

## Inventory Metadata

| Field | Value |
|---|---|
| Authority identity | Visibility roadmap behavior rows; Valve Source SDK 2013 BSP, spatial-query, leaf-system, area-portal, occluder, static-prop, view, PVS/PAS, and TF2 consumer contracts |
| Authority revision | Source SDK `88fa198fba3fb85d46d4c95018254693fdc3af0a`; roadmap revision Unknown until committed |
| Generator command | Missing |
| Output path | `packages/world/visibility/inventories/visibility-systems.md` |
| Item count | 42 Candidate; 0 Accepted |

This file is a manually derived Candidate and is not a generated inventory. Its items contribute 0 completion-denominator items until the checked-in generator, current output, and denominator review satisfy [`../../../../docs/roadmap-contract.md`](../../../../docs/roadmap-contract.md). Stable IDs are reserved now so generation cannot silently merge or renumber systems.

## Candidate Items

| Stable ID | System | Exact boundary | Coverage classification |
|---|---|---|---|
| VIS-001 | Plane input | Consume finite typed BSP plane identity, normal, distance, and type; do not parse bytes. | Unsupported |
| VIS-002 | Node topology | Validate stable node identity, plane reference, two child references, bounds, and area summary. | Unsupported |
| VIS-003 | Leaf topology | Validate stable leaf identity, bounds, contents, cluster, area, flags, and map memberships. | Unsupported |
| VIS-004 | Cluster universe | Derive the finite cluster set and validate every leaf and visibility-row reference. | Unsupported |
| VIS-005 | World head node | Validate and retain the world model's starting node or encoded leaf. | Unsupported |
| VIS-006 | Point-to-leaf query | Traverse the world tree with exact negative/zero/positive plane-side behavior. | Unsupported |
| VIS-007 | Point-to-cluster query | Return the resolved leaf's cluster, including valid cluster `-1`. | Unsupported |
| VIS-008 | AABB-to-leaves query | Enumerate every intersected leaf once under declared result limits. | Unsupported |
| VIS-009 | Head-node visibility | Decide whether any descendant leaf has a cluster bit set. | Unsupported |
| VIS-010 | PVS offsets | Validate one PVS compressed-row offset per cluster. | Unsupported |
| VIS-011 | PVS decompression | Expand bounded literal and zero-run bytes to one exact cluster row. | Unsupported |
| VIS-012 | PVS queries | Answer cluster, leaf, point, head-node, and box potential visibility. | Unsupported |
| VIS-013 | PAS offsets | Validate one PAS compressed-row offset per cluster. | Unsupported |
| VIS-014 | PAS decompression | Expand bounded PAS bytes with the same row and padding invariants as PVS. | Unsupported |
| VIS-015 | PAS queries | Return potentially audible cluster, leaf, point, and box results without recipient policy. | Unsupported |
| VIS-016 | Multi-origin union | Union 1 through 32 visibility origins after exact point-to-cluster lookup. | Unsupported |
| VIS-017 | Empty visibility lump | Treat a valid empty visibility lump as all PVS and PAS clusters visible. | Unsupported |
| VIS-018 | Explicit PVS bypass | Return all cluster candidates while retaining portal, occlusion, and presentation boundaries. | Unsupported |
| VIS-019 | Outside-world view | Return all visibility-gated candidates when a finite camera origin resolves to cluster `-1`. | Unsupported |
| VIS-020 | Area records | Validate area identity and each contiguous directed-portal range. | Unsupported |
| VIS-021 | Directed area portals | Validate portal key, destination area, plane, and clip-vertex range. | Unsupported |
| VIS-022 | Portal open state | Apply ordered key-based open/close updates atomically at one revision. | Unsupported |
| VIS-023 | Area connectivity | Return deterministic transitive connectivity under current portal state. | Unsupported |
| VIS-024 | Camera-effective portal state | Separate topology-open, flow-eligible, and presentation-open decisions. | Unsupported |
| VIS-025 | Portal view windows | Clip and intersect ordered portal polygons into normalized per-area windows. | Unsupported |
| VIS-026 | Visible-area query | Return every area reached by current camera flow once in deterministic order. | Unsupported |
| VIS-027 | Occluder records | Validate stable occluder identity, bounds, area, flags, and polygon range. | Unsupported |
| VIS-028 | Occluder polygons | Validate plane and ordered vertex-index ranges with at least three finite vertices. | Unsupported |
| VIS-029 | Occluder active state | Apply ordered active/inactive updates without mutating map geometry. | Unsupported |
| VIS-030 | AABB occlusion query | Classify one projected AABB under active visible-area occluders and explicit thresholds. | Unsupported |
| VIS-031 | Sky classification | Return `3D sky`, `2D sky`, or `not visible` with declared multi-origin precedence. | Unsupported |
| VIS-032 | World-surface membership | Retain map-owned surface-to-leaf memberships and emit visible surface IDs once. | Unsupported |
| VIS-033 | Static-prop membership | Retain compiled prop leaf lists and emit visible prop IDs once. | Unsupported |
| VIS-034 | Dynamic-object membership | Link current finite AABBs to leaves and invalidate membership after every bounds revision. | Unsupported |
| VIS-035 | Detail-prop membership | Retain compiled per-leaf ranges and emit candidates without placement, fade, or draw behavior. | Unsupported |
| VIS-036 | Ordered world list | Emit front-to-back leaves and first-encounter deduplicated world and object candidates. | Unsupported |
| VIS-037 | Presentation candidate contract | Supply immutable IDs, memberships, bounds, portal windows, sky state, and occlusion decisions. | Unsupported |
| VIS-038 | Model LOD inputs | Pass through model origin, radius, camera origin, and FOV scale without selecting LOD or alpha. | Unsupported |
| VIS-039 | Cluster-row cache | Bound decompressed PVS/PAS and merged-cluster entries by identity, count, and bytes. | Unsupported |
| VIS-040 | View cache and invalidation | Key complete results by exact view and state revision and invalidate changed dependencies. | Unsupported |
| VIS-041 | Bounds and deterministic errors | Enforce all count, byte, traversal, membership, update, cache, and output limits before mutation. | Unsupported |
| VIS-042 | Lifecycle and integration | Load, query, update, replace, and unload one map through one interface used by every declared consumer. | Missing |

## Generation Contract

The future generator must emit exactly one row for every accepted stable ID, preserve numeric order, and calculate the item count from emitted rows. It must fail on duplicate IDs, gaps, changed meanings, unclassified official symbols, missing ownership, missing evidence methods, or output differing from the accepted manifest. Hand editing remains invalid after generation exists.
