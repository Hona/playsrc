# Collision Contents And Masks Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

Authority identity: Valve Source SDK 2013 `src/public/bspflags.h`, `trace.h`, and `const.h`; `src/game/shared/tf/tf_shareddefs.h` for game-owned aliases and collision-group extensions.

Authority revision: SDK commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; exact TF2, CS:S, and legacy Source 1 CS:GO content-build indexes Missing.

Generator command: Missing. The future command is owned by `tools/playsrc`.

Output path: `packages/world/collision/inventories/contents-and-masks.md`

Item count: 98 candidate items; 0 accepted items. All 98 candidate items are Candidate required.

Every contents field and mask is an unsigned 32-bit bitset. Unknown set bits remain observable. A mask selects a candidate only when `(candidateContents & queryMask) != 0`; aliases never create new bits.

## Contents Values

| Stable identity | SDK identity | Value | Required representation |
|---|---|---:|---|
| `contents.empty` | `CONTENTS_EMPTY` | `0x00000000` | Empty set |
| `contents.solid` | `CONTENTS_SOLID` | `0x00000001` | Named bit 0 |
| `contents.window` | `CONTENTS_WINDOW` | `0x00000002` | Named bit 1 |
| `contents.aux` | `CONTENTS_AUX` | `0x00000004` | Named bit 2 |
| `contents.grate` | `CONTENTS_GRATE` | `0x00000008` | Named bit 3 |
| `contents.slime` | `CONTENTS_SLIME` | `0x00000010` | Named bit 4 |
| `contents.water` | `CONTENTS_WATER` | `0x00000020` | Named bit 5 |
| `contents.block-los` | `CONTENTS_BLOCKLOS` | `0x00000040` | Named bit 6 |
| `contents.opaque` | `CONTENTS_OPAQUE` | `0x00000080` | Named bit 7 |
| `contents.test-fog-volume` | `CONTENTS_TESTFOGVOLUME` | `0x00000100` | Named bit 8 |
| `contents.unused-9` | `CONTENTS_UNUSED` | `0x00000200` | Retained named reserved bit 9 |
| `contents.unused-10` | `CONTENTS_UNUSED6` | `0x00000400` | Retained named reserved bit 10 |
| `contents.team-1` | `CONTENTS_TEAM1` | `0x00000800` | Named bit 11; game-owned aliases do not change it |
| `contents.team-2` | `CONTENTS_TEAM2` | `0x00001000` | Named bit 12; game-owned aliases do not change it |
| `contents.ignore-nodraw-opaque` | `CONTENTS_IGNORE_NODRAW_OPAQUE` | `0x00002000` | Query-control bit 13 retained in masks and results |
| `contents.moveable` | `CONTENTS_MOVEABLE` | `0x00004000` | Named bit 14 |
| `contents.areaportal` | `CONTENTS_AREAPORTAL` | `0x00008000` | Named bit 15 |
| `contents.player-clip` | `CONTENTS_PLAYERCLIP` | `0x00010000` | Named bit 16 |
| `contents.monster-clip` | `CONTENTS_MONSTERCLIP` | `0x00020000` | Named bit 17 |
| `contents.current-0` | `CONTENTS_CURRENT_0` | `0x00040000` | Named bit 18 |
| `contents.current-90` | `CONTENTS_CURRENT_90` | `0x00080000` | Named bit 19 |
| `contents.current-180` | `CONTENTS_CURRENT_180` | `0x00100000` | Named bit 20 |
| `contents.current-270` | `CONTENTS_CURRENT_270` | `0x00200000` | Named bit 21 |
| `contents.current-up` | `CONTENTS_CURRENT_UP` | `0x00400000` | Named bit 22 |
| `contents.current-down` | `CONTENTS_CURRENT_DOWN` | `0x00800000` | Named bit 23 |
| `contents.origin` | `CONTENTS_ORIGIN` | `0x01000000` | Named bit 24 |
| `contents.monster` | `CONTENTS_MONSTER` | `0x02000000` | Named bit 25 |
| `contents.debris` | `CONTENTS_DEBRIS` | `0x04000000` | Named bit 26 |
| `contents.detail` | `CONTENTS_DETAIL` | `0x08000000` | Named bit 27 |
| `contents.translucent` | `CONTENTS_TRANSLUCENT` | `0x10000000` | Named bit 28 |
| `contents.ladder` | `CONTENTS_LADDER` | `0x20000000` | Named bit 29 |
| `contents.hitbox` | `CONTENTS_HITBOX` | `0x40000000` | Named bit 30 |
| `contents.unassigned-31` | No named SDK contents identity | `0x80000000` | Retained Unknown bit 31; never discarded or assigned inferred meaning |

## Contents Boundaries And Masks

| Stable identity | SDK identity | Expanded unsigned value |
|---|---|---:|
| `contents-boundary.last-visible` | `LAST_VISIBLE_CONTENTS` | `0x00000080` |
| `contents-mask.all-visible` | `ALL_VISIBLE_CONTENTS` | `0x000000ff` |
| `mask.all` | `MASK_ALL` | `0xffffffff` |
| `mask.solid` | `MASK_SOLID` | `0x0200400b` |
| `mask.player-solid` | `MASK_PLAYERSOLID` | `0x0201400b` |
| `mask.npc-solid` | `MASK_NPCSOLID` | `0x0202400b` |
| `mask.water` | `MASK_WATER` | `0x00004030` |
| `mask.opaque` | `MASK_OPAQUE` | `0x00004081` |
| `mask.opaque-and-npcs` | `MASK_OPAQUE_AND_NPCS` | `0x02004081` |
| `mask.block-los` | `MASK_BLOCKLOS` | `0x00004041` |
| `mask.block-los-and-npcs` | `MASK_BLOCKLOS_AND_NPCS` | `0x02004041` |
| `mask.visible` | `MASK_VISIBLE` | `0x00006081` |
| `mask.visible-and-npcs` | `MASK_VISIBLE_AND_NPCS` | `0x02006081` |
| `mask.shot` | `MASK_SHOT` | `0x46004003` |
| `mask.shot-hull` | `MASK_SHOT_HULL` | `0x0600400b` |
| `mask.shot-portal` | `MASK_SHOT_PORTAL` | `0x02004003` |
| `mask.solid-brush-only` | `MASK_SOLID_BRUSHONLY` | `0x0000400b` |
| `mask.player-solid-brush-only` | `MASK_PLAYERSOLID_BRUSHONLY` | `0x0001400b` |
| `mask.npc-solid-brush-only` | `MASK_NPCSOLID_BRUSHONLY` | `0x0002400b` |
| `mask.npc-world-static` | `MASK_NPCWORLDSTATIC` | `0x0002000b` |
| `mask.split-areaportal` | `MASK_SPLITAREAPORTAL` | `0x00000030` |
| `mask.current` | `MASK_CURRENT` | `0x00fc0000` |
| `mask.dead-solid` | `MASK_DEADSOLID` | `0x0001000b` |

## Surface Flags

| Stable identity | SDK identity | Value | Collision result requirement |
|---|---|---:|---|
| `surface.light` | `SURF_LIGHT` | `0x0001` | Preserve in 16-bit flags |
| `surface.sky-2d` | `SURF_SKY2D` | `0x0002` | Preserve in 16-bit flags |
| `surface.sky` | `SURF_SKY` | `0x0004` | Preserve in 16-bit flags |
| `surface.warp` | `SURF_WARP` | `0x0008` | Preserve in 16-bit flags |
| `surface.translucent` | `SURF_TRANS` | `0x0010` | Preserve in 16-bit flags |
| `surface.no-portal` | `SURF_NOPORTAL` | `0x0020` | Preserve in 16-bit flags |
| `surface.trigger` | `SURF_TRIGGER` | `0x0040` | Preserve in 16-bit flags |
| `surface.no-draw` | `SURF_NODRAW` | `0x0080` | Preserve and apply only the declared nodraw-opaque mask rule |
| `surface.hint` | `SURF_HINT` | `0x0100` | Preserve in 16-bit flags |
| `surface.skip` | `SURF_SKIP` | `0x0200` | Preserve in 16-bit flags |
| `surface.no-light` | `SURF_NOLIGHT` | `0x0400` | Preserve in 16-bit flags |
| `surface.bump-light` | `SURF_BUMPLIGHT` | `0x0800` | Preserve in 16-bit flags |
| `surface.no-shadows` | `SURF_NOSHADOWS` | `0x1000` | Preserve in 16-bit flags |
| `surface.no-decals` | `SURF_NODECALS` | `0x2000` | Preserve in 16-bit flags |
| `surface.no-chop` | `SURF_NOCHOP` | `0x4000` | Preserve in 16-bit flags |
| `surface.hitbox` | `SURF_HITBOX` | `0x8000` | Preserve and emit for StudioModel hitbox hits |

## Displacement Triangle Flags

| Stable identity | SDK identity | Value | Collision behavior |
|---|---|---:|---|
| `displacement.surface` | `DISPTRI_TAG_SURFACE` / `DISPSURF_FLAG_SURFACE` | `0x0001` | Retain on trace hits |
| `displacement.walkable` | `DISPTRI_TAG_WALKABLE` / `DISPSURF_FLAG_WALKABLE` | `0x0002` | Retain on trace hits; movement decides standability |
| `displacement.buildable` | `DISPTRI_TAG_BUILDABLE` / `DISPSURF_FLAG_BUILDABLE` | `0x0004` | Retain on trace hits |
| `displacement.surface-property-1` | `DISPTRI_FLAG_SURFPROP1` / `DISPSURF_FLAG_SURFPROP1` | `0x0008` | Retain and select the supplied displacement surface identity |
| `displacement.surface-property-2` | `DISPTRI_FLAG_SURFPROP2` / `DISPSURF_FLAG_SURFPROP2` | `0x0010` | Retain and select the supplied displacement surface identity |
| `displacement.remove` | `DISPTRI_TAG_REMOVE` | `0x0020` | Exclude the tagged compiled triangle while retaining its source identity and classification |

## Shared Collision-Group Identities

The collision package retains these shared integer identities and passes them to a caller-supplied group-pair predicate. It does not own the pair matrix.

| Stable identity | SDK identity | Value |
|---|---|---:|
| `collision-group.none` | `COLLISION_GROUP_NONE` | 0 |
| `collision-group.debris` | `COLLISION_GROUP_DEBRIS` | 1 |
| `collision-group.debris-trigger` | `COLLISION_GROUP_DEBRIS_TRIGGER` | 2 |
| `collision-group.interactive-debris` | `COLLISION_GROUP_INTERACTIVE_DEBRIS` | 3 |
| `collision-group.interactive` | `COLLISION_GROUP_INTERACTIVE` | 4 |
| `collision-group.player` | `COLLISION_GROUP_PLAYER` | 5 |
| `collision-group.breakable-glass` | `COLLISION_GROUP_BREAKABLE_GLASS` | 6 |
| `collision-group.vehicle` | `COLLISION_GROUP_VEHICLE` | 7 |
| `collision-group.player-movement` | `COLLISION_GROUP_PLAYER_MOVEMENT` | 8 |
| `collision-group.npc` | `COLLISION_GROUP_NPC` | 9 |
| `collision-group.in-vehicle` | `COLLISION_GROUP_IN_VEHICLE` | 10 |
| `collision-group.weapon` | `COLLISION_GROUP_WEAPON` | 11 |
| `collision-group.vehicle-clip` | `COLLISION_GROUP_VEHICLE_CLIP` | 12 |
| `collision-group.projectile` | `COLLISION_GROUP_PROJECTILE` | 13 |
| `collision-group.door-blocker` | `COLLISION_GROUP_DOOR_BLOCKER` | 14 |
| `collision-group.passable-door` | `COLLISION_GROUP_PASSABLE_DOOR` | 15 |
| `collision-group.dissolving` | `COLLISION_GROUP_DISSOLVING` | 16 |
| `collision-group.pushaway` | `COLLISION_GROUP_PUSHAWAY` | 17 |
| `collision-group.npc-actor` | `COLLISION_GROUP_NPC_ACTOR` | 18 |
| `collision-group.npc-scripted` | `COLLISION_GROUP_NPC_SCRIPTED` | 19 |

## Game-Owned Exclusions

These aliases and extensions are enumerated to close the package boundary. They are not collision inventory items and do not contribute to this roadmap's item count.

| SDK identity | Value | Sole owner |
|---|---:|---|
| `CONTENTS_REDTEAM` | Alias of `CONTENTS_TEAM1` | `games/tf2` |
| `CONTENTS_BLUETEAM` | Alias of `CONTENTS_TEAM2` | `games/tf2` |
| `TF_COLLISIONGROUP_GRENADES` | 20 | `games/tf2` |
| `TFCOLLISION_GROUP_OBJECT` | 21 | `games/tf2` |
| `TFCOLLISION_GROUP_OBJECT_SOLIDTOPLAYERMOVEMENT` | 22 | `games/tf2` |
| `TFCOLLISION_GROUP_COMBATOBJECT` | 23 | `games/tf2` |
| `TFCOLLISION_GROUP_ROCKETS` | 24 | `games/tf2` |
| `TFCOLLISION_GROUP_RESPAWNROOMS` | 25 | `games/tf2` |
| `TFCOLLISION_GROUP_TANK` | 26 | `games/tf2` |
| `TFCOLLISION_GROUP_ROCKET_BUT_NOT_WITH_OTHER_ROCKETS` | 27 | `games/tf2` |

Every TF2 group-pair decision and corresponding CS:S and legacy CS:GO alias, extension, and pair decision belongs to its game module.

The future generator must evaluate every named constant from retained SDK snapshots, verify each expanded value, enumerate all 32 contents bit positions, and compare encountered declared-build brush, model, bone, convex, and query-policy values. It must fail on a changed expansion, an unclassified set bit, a duplicate stable identity, an unassigned shared group value below `LAST_SHARED_COLLISION_GROUP`, or a game-owned alias incorrectly assigned to this package.
