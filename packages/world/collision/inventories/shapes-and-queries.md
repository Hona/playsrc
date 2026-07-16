# Collision Shape And Query Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

Authority identity: Valve Source SDK 2013 `src/public/cmodel.h`, `trace.h`, `gametrace.h`, `const.h`, `collisionutils.h`, `dispcoll_common.{h,cpp}`, `vphysics_interface.h`, `engine/ICollideable.h`, `engine/IEngineTrace.h`, and `engine/IStaticPropMgr.h`; `src/public/bone_setup.cpp`; `src/game/shared/collisionproperty.{h,cpp}`, `util_shared.{h,cpp}`, and `gamemovement.cpp`; exact declared-build BSP, PHY, StudioModel, static-prop, and entity collision indexes.

Authority revision: SDK commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; exact TF2, CS:S, and legacy Source 1 CS:GO content-build indexes Missing.

Generator command: Missing. The future command is owned by `tools/playsrc`.

Output path: `packages/world/collision/inventories/shapes-and-queries.md`

Item count: 76 candidate items; 0 accepted items. Candidate dispositions: 73 Candidate required and 3 Blocked.

`Candidate required` identifies a manually derived target-contract item. It does not assert implementation or inventory acceptance. `Blocked` identifies an item whose exact input contract or sole owner must be resolved before acceptance. Every row becomes a denominator item only after a checked-in generator emits this file and a denominator review accepts it.

## Collision Representations

| Stable identity | Input representation | Required collision representation | Candidate disposition |
|---|---|---|---|
| `shape.world-brush-convex` | BSP world-model brush sides, planes, contents, bevel markers, and surface identities | One bounded convex half-space shape per nonempty world brush, retaining brush and side identity | Candidate required |
| `shape.brush-model-convex` | BSP non-world model head node, owned brushes, model origin, and current transform | Ordered transformed convex brush set retaining model, brush, and entity identity | Candidate required |
| `shape.displacement-patch` | Map-owned compiled displacement vertices, triangles, triangle tags, contents, and surface identities | One-sided collision patch with deterministic triangle and acceleration identities | Candidate required |
| `shape.static-prop-none` | Static-prop solid mode `SOLID_NONE` | Represented static-prop identity classified Intentionally inert for collision | Candidate required |
| `shape.static-prop-bbox` | Static-prop solid mode `SOLID_BBOX`, model bounds, origin, angles, and uniform scale | World-aligned box behavior selected by the static-prop contract, retaining prop index | Candidate required |
| `shape.static-prop-phy` | Static-prop solid mode `SOLID_VPHYSICS` plus decoded PHY compounds | Transformed compound-convex shape retaining prop, solid, convex, contents, and surface identity | Candidate required |
| `shape.entity-none` | `SOLID_NONE` entity collideable | Represented entity identity classified Intentionally inert unless a declared trigger query includes it | Candidate required |
| `shape.entity-bsp` | `SOLID_BSP` brush model and entity transform | Transformed brush-model shape | Candidate required |
| `shape.entity-aabb` | `SOLID_BBOX` mins, maxs, origin, and world-alignment policy | Axis-aligned box with asymmetric local bounds preserved | Candidate required |
| `shape.entity-obb` | `SOLID_OBB` mins, maxs, origin, and three-axis rotation | Oriented box retaining local bounds and exact transform | Candidate required |
| `shape.entity-yaw-obb` | `SOLID_OBB_YAW` mins, maxs, origin, and yaw | Yaw-constrained oriented box | Candidate required |
| `shape.entity-custom` | `SOLID_CUSTOM` or a custom ray/box flag plus a caller-owned query adapter | Stable adapter boundary that returns the current collision result contract without owning entity behavior | Candidate required |
| `shape.entity-phy` | `SOLID_VPHYSICS` entity transform and decoded PHY compounds | Transformed compound-convex collideable | Candidate required |
| `shape.trigger-volume` | Trigger solid flag, ordinary or expanded trigger bounds, optional brush model, contents, and stable entity identity | Separate trigger-query shape that never enters ordinary solid queries unless explicitly selected | Candidate required |
| `shape.studio-hitbox-obb` | Selected hitbox set, ordered boxes, bone transforms, bone contents, surface property, hitgroup, and physics bone | Ordered transformed hitbox OBBs retaining hitbox, hitgroup, bone, and entity identity | Candidate required |
| `shape.phy-convex` | One decoded PHY convex with vertices, faces, game data, material metadata, and source identity | Queryable convex preserving support-map and triangle metadata | Candidate required |
| `shape.phy-compound` | Ordered PHY convexes belonging to one solid | Ordered compound retaining solid and convex identity; no rigid-body state | Candidate required |
| `shape.legacy-prop-hull` | Legacy CS:GO BSP prop-collision, hull, vertex, triangle-range, and blob records | Typed prop hull or triangle shape retaining source record identity | Blocked: the complete supplied typed input contract and declared-build occurrence are Missing |

## Dynamic Query Shapes

| Stable identity | Required input | Required normalization | Candidate disposition |
|---|---|---|---|
| `query-shape.point` | One finite Source-space position | Zero extents and zero motion | Candidate required |
| `query-shape.finite-ray` | Finite origin, finite unit direction, and non-negative finite maximum distance | Start plus supplied direction times maximum distance; zero extents and no hidden renormalization | Candidate required |
| `query-shape.aabb-hull` | Finite start/end plus finite mins and maxs with `mins <= maxs` per axis | Center offset and half extents derived once; asymmetric bounds remain observable | Candidate required |
| `query-shape.convex` | One validated convex, start/end transforms, and fixed orientation during the sweep | Support-map shape with stable vertex and feature order | Candidate required |
| `query-shape.compound` | Ordered validated convex children and one start/end transform pair | Child order retained; one result is selected by the declared tie rule | Candidate required |

## Query Operations

| Stable identity | Observable operation | Required output | Candidate disposition |
|---|---|---|---|
| `query.point-contents-world` | Test one point against world and selected volume shapes | Combined 32-bit contents plus ordered contributors | Candidate required |
| `query.point-contents-shape` | Test one point against one selected collideable | That shape's contents or `CONTENTS_EMPTY` | Candidate required |
| `query.ray-world` | Cast a finite ray through the selected world scope | Closest trace result | Candidate required |
| `query.ray-shape` | Cast a finite ray against one selected shape | Shape-local closest trace result mapped to world space | Candidate required |
| `query.line-world` | Trace from one finite endpoint to another through the selected world scope | Closest trace result using endpoint parameterization | Candidate required |
| `query.line-shape` | Trace one finite segment against one selected shape | Shape-local closest trace result mapped to world space | Candidate required |
| `query.aabb-sweep-world` | Sweep an AABB or player hull through the selected world scope | Closest trace result and initial-overlap state | Candidate required |
| `query.aabb-sweep-shape` | Sweep an AABB or player hull against one selected shape | Shape-local trace result mapped to world space | Candidate required |
| `query.convex-sweep-world` | Sweep one convex or compound through the selected world scope at fixed orientation | Closest trace result and initial-overlap state | Candidate required |
| `query.convex-sweep-shape` | Sweep one convex or compound against one selected shape at fixed orientations | Pair trace result | Candidate required |
| `query.relative-motion-shape` | Sweep one point, AABB, convex, or compound while one selected target shape moves from a supplied start transform to a supplied end transform | First normalized-time contact, initial-overlap state, and both snapshot/transform identities | Candidate required |
| `query.overlap-world` | Test one point, AABB, convex, or compound at one transform against the selected world scope | Ordered overlapping shape and feature identities | Candidate required |
| `query.overlap-shape` | Test one point, AABB, convex, or compound against one selected shape | Boolean overlap plus ordered intersecting feature identities | Candidate required |
| `query.contacts-world` | Request stateless geometric contacts between one convex compound and the selected world scope | Bounded ordered contact sets containing shape pair, normal, depth, points, contents, and surface identities | Blocked: persistent physics contacts and stateless collision contacts do not yet have an agreed cross-roadmap boundary |
| `query.contacts-shape` | Request stateless geometric contacts between two selected convex compounds | Bounded ordered contact set for that pair | Blocked: persistent physics contacts and stateless collision contacts do not yet have an agreed cross-roadmap boundary |
| `query.enumerate-ray` | Enumerate broad-phase dynamic candidates intersecting a swept query AABB | Stable candidate identities; never a final hit claim | Candidate required |
| `query.enumerate-aabb` | Enumerate broad-phase dynamic candidates overlapping one world AABB | Stable candidate identities; never a final overlap claim | Candidate required |
| `query.support-point` | Query a convex or compound in one finite nonzero direction | Stable extreme point and source feature identity | Candidate required |
| `query.world-bounds` | Query one shape or the complete immutable world bounds | Finite enclosing AABB with declared empty-world behavior | Candidate required |

## Scope And Filter Policies

| Stable identity | Required behavior | Candidate disposition |
|---|---|---|
| `filter.scope-everything` | Test world, static props, and eligible dynamic collideables; ordinary static props bypass entity predicates only when the selected game contract requires it | Candidate required |
| `filter.scope-world-only` | Test world geometry and exclude static props and dynamic collideables | Candidate required |
| `filter.scope-entities-only` | Test eligible dynamic collideables and exclude world geometry and static props | Candidate required |
| `filter.scope-everything-filter-props` | Test world, dynamic collideables, and static props while passing static props through the entity predicate | Candidate required |
| `filter.contents-mask` | Reject a shape or convex when its contents have no set bit in the query mask | Candidate required |
| `filter.solid-and-trigger-state` | Apply solid type, solid flags, enabled state, trigger selection, and volume-contents selection before narrow phase | Candidate required |
| `filter.ignored-identities` | Exclude an ordered caller-supplied set of stable entity, prop, model, shape, or owner identities | Candidate required |
| `filter.collision-group-predicate` | Invoke one caller-supplied symmetric game-owned group-pair predicate without interpreting game group values | Candidate required |
| `filter.custom-predicate` | Invoke one pure caller predicate after generic structural filters and before narrow phase | Candidate required |
| `filter.trigger-selection` | Select ordinary solids, triggers, or both through separate broad-phase sets without changing shape solidity | Candidate required |

## Solid Types

| Stable identity | Encoded value | Shape-selection behavior | Candidate disposition |
|---|---:|---|---|
| `solid.none` | 0 | No ordinary collision shape | Candidate required |
| `solid.bsp` | 1 | Brush-model shape | Candidate required |
| `solid.bbox` | 2 | Axis-aligned box | Candidate required |
| `solid.obb` | 3 | Fully oriented box | Candidate required |
| `solid.obb-yaw` | 4 | Yaw-constrained oriented box | Candidate required |
| `solid.custom` | 5 | Caller-owned custom query adapter | Candidate required |
| `solid.vphysics` | 6 | PHY-derived convex compound | Candidate required |

## Solid Flags

| Stable identity | Encoded value | Query effect | Candidate disposition |
|---|---:|---|---|
| `solid-flag.custom-ray-test` | `0x0001` | Route ray tests through the custom adapter | Candidate required |
| `solid-flag.custom-box-test` | `0x0002` | Route swept-box tests through the custom adapter | Candidate required |
| `solid-flag.not-solid` | `0x0004` | Exclude ordinary solid queries without erasing trigger identity | Candidate required |
| `solid-flag.trigger` | `0x0008` | Include only through declared trigger selection | Candidate required |
| `solid-flag.not-standable` | `0x0010` | Preserve for movement-owned standability decisions | Candidate required |
| `solid-flag.volume-contents` | `0x0020` | Include in entity point-contents queries | Candidate required |
| `solid-flag.force-world-aligned` | `0x0040` | Use world-aligned collision bounds | Candidate required |
| `solid-flag.use-trigger-bounds` | `0x0080` | Use separate expanded trigger bounds for trigger queries | Candidate required |
| `solid-flag.root-parent-aligned` | `0x0100` | Interpret collision bounds in the supplied root-parent transform | Candidate required |
| `solid-flag.trigger-touch-debris` | `0x0200` | Preserve for game-owned trigger subject policy | Candidate required |

## Surrounding-Bounds Policies

| Stable identity | Broad-phase bounds source | Candidate disposition |
|---|---|---|
| `bounds.collision-obb` | Collision OBB transformed to a world AABB | Candidate required |
| `bounds.best-collision` | Tightest available declared collision representation | Candidate required |
| `bounds.hitboxes` | Union of current transformed hitboxes | Candidate required |
| `bounds.specified` | Explicit caller-supplied local bounds | Candidate required |
| `bounds.game-code` | Pure caller adapter supplying finite bounds | Candidate required |
| `bounds.rotation-expanded` | Rotation-independent expansion enclosing every accepted orientation | Candidate required |
| `bounds.collision-never-physics` | Collision bounds without PHY-derived expansion | Candidate required |

The future generator must enumerate every accepted shape source, query shape, operation, filter policy, solid type, solid flag, and surrounding-bounds policy from retained authority snapshots and exact declared-build indexes. It must preserve stable order, emit encountered unknown or unsupported solid modes and shape sources, and fail on an unclassified collision source, query operation, solid selector, flag bit, bounds policy, or content-build occurrence.
