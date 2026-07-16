# BSP Standard Lump Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

Authority identity: Valve Source SDK 2013 `src/public/bspfile.h` and `src/utils/common/bsplib.cpp` at commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; AlliedModders `hl2sdk` CS:GO `public/bspfile.h` at commit `9cf2f325ea273559c7cae27b4f98518c18b8e322`; srctools v2.6.1 tag `814d2cb2f897c99507b9e246c61228c93368134f`; exact declared-build BSP headers Missing.

Generator command: Missing. Candidate item count: 64. Accepted item count: 0.

Every row is owned structurally by BSP. The final column assigns semantic use after parsing; it does not transfer byte parsing to that owner. `Opaque` means the BSP result retains the complete encoded and optional decoded byte sequence, version, range, compression metadata, and classification without inventing fields.

| Slot | Profile identity | Accepted encoded representation | BSP typed representation | Downstream semantic owner |
|---:|---|---|---|---|
| 0 | `ENTITIES` | Ordered entity text bytes; no record-size claim | Exact bytes including terminator and tail | `packages/world/entity` |
| 1 | `PLANES` | v0; 20-byte records: 3×f32 normal, f32 distance, i32 type | Ordered plane records with raw float bits | Map assembly: `packages/world/map`; collision and visibility queries: their named world packages |
| 2 | `TEXDATA` | v0; 32-byte records: 3×f32 reflectivity, i32 name index, i32 width, height, view width, view height | Ordered texture-data records | `packages/world/material` |
| 3 | `VERTEXES` | v0; 12-byte records: 3×f32 position | Ordered vertex records | `packages/world/map` |
| 4 | `VISIBILITY` | v0; i32 cluster count, `clusterCount` pairs of i32 PVS/PAS offsets, remaining compressed bytes | Header, offset pairs, and exact compressed-byte tail | `packages/world/visibility` |
| 5 | `NODES` | v0; 32-byte records: i32 plane, 2×i32 children, 6×i16 bounds, u16 first face/count, i16 area | Ordered node records | `packages/world/map`; traversal policy belongs to collision or visibility |
| 6 | `TEXINFO` | v0; 72-byte records: 16×f32 texture/lightmap vectors, i32 flags, i32 texdata index | Ordered texture-info records | `packages/world/material` |
| 7 | `FACES` | v1; 56-byte face records | Ordered face records retaining packed primitive/shadow bits and all scalar raw bits | `packages/world/map`; lighting and draw interpretation belong to rendering |
| 8 | `LIGHTING` | v1; 4-byte RGB-exponent samples | Ordered raw sample records | `packages/presentation/rendering` |
| 9 | `OCCLUSION` | v2; i32 occluder count + 40-byte occluders, i32 polygon count + 12-byte polygons, i32 vertex-index count + i32 indices | Three bounded counted sections | `packages/presentation/rendering` |
| 10 | `LEAFS` | v0: 56-byte records with embedded 24-byte ambient cube; v1: 32-byte records | Profile-selected ordered leaf records | Map assembly: `packages/world/map`; solid and visibility meaning: collision and visibility |
| 11 | `FACEIDS` | v0; u16 Hammer face IDs | Ordered u16 records | `packages/world/map` and map tools |
| 12 | `EDGES` | v0; 4-byte records containing 2×u16 vertex indices | Ordered edge records | `packages/world/map` |
| 13 | `SURFEDGES` | v0; i32 signed edge references | Ordered i32 records | `packages/world/map` |
| 14 | `MODELS` | v0; 48-byte records: bounds, origin, head node, face range | Ordered brush-model records | `packages/world/map` |
| 15 | `WORLDLIGHTS` | Source profiles v0: 88-byte records; legacy CS:GO v21 v1: 100-byte records adding a 3×f32 shadow-cast offset | Profile-selected ordered world-light records | `packages/presentation/rendering` |
| 16 | `LEAFFACES` | v0; u16 face indices | Ordered u16 records | `packages/world/map` |
| 17 | `LEAFBRUSHES` | v0; u16 brush indices | Ordered u16 records | `packages/world/collision` |
| 18 | `BRUSHES` | v0; 12-byte records: i32 first side, side count, contents | Ordered brush records | `packages/world/collision` |
| 19 | `BRUSHSIDES` | Source profiles: 8 bytes ending in i16 bevel; legacy CS:GO v21: 8 bytes ending in u8 bevel and u8 thin | Profile-selected ordered brush-side records | `packages/world/collision` |
| 20 | `AREAS` | v0; 8-byte records: i32 portal count and first portal | Ordered area records | `packages/world/visibility` |
| 21 | `AREAPORTALS` | v0; 12-byte records: 4×u16 portal fields and i32 plane | Ordered area-portal records | `packages/world/visibility` |
| 22 | Source profiles `UNUSED0`; legacy CS:GO `PROPCOLLISION` | Source profiles: Opaque, normally empty; v21: 8-byte records containing i32 hull count/start | Profile-selected opaque or prop-collision records | v21 collision meaning: `packages/world/collision` |
| 23 | Source profiles `UNUSED1`; legacy CS:GO `PROPHULLS` | Source profiles: Opaque, normally empty; v21: 16-byte records containing i32 vertex count/start, i32 surface property, u32 contents | Profile-selected opaque or prop-hull records | `packages/world/collision` |
| 24 | Source profiles `UNUSED2`; legacy CS:GO `PROPHULLVERTS` | Source profiles: Opaque, normally empty; v21: 12-byte 3×f32 vertices | Profile-selected opaque or prop-hull vertices | `packages/world/collision` |
| 25 | Source profiles `UNUSED3`; legacy CS:GO `PROPTRIS` | Source profiles: Opaque, normally empty; v21: 8-byte records containing i32 index start/count | Profile-selected opaque or prop-triangle ranges | `packages/world/collision` |
| 26 | `DISPINFO` | v0; 176-byte displacement-info records with edge/corner neighbor records and ten u32 allowed-vertex words | Ordered displacement-info records retaining all packed values | `packages/world/map` |
| 27 | `ORIGINALFACES` | v0; 56-byte face records | Ordered original-face records | `packages/world/map` and map tools |
| 28 | `PHYSDISP` | v0; u16 displacement count, that many u16 blob lengths where `0xffff` means absent, then concatenated blobs | Count, optional lengths, and bounded opaque blobs | Framing: BSP; shape meaning: `packages/runtime/physics` |
| 29 | `PHYSCOLLIDE` | v0; repeated 16-byte model headers with i32 model/data/key-data/solid counts, framed solid bytes and key-data bytes, terminated by a non-positive data size | Bounded model-block framing and opaque solid/key-data payloads | `packages/runtime/physics` |
| 30 | `VERTNORMALS` | v0; 12-byte 3×f32 vectors | Ordered normal records | `packages/world/map`; draw use belongs to rendering |
| 31 | `VERTNORMALINDICES` | v0; u16 normal indices | Ordered u16 records | `packages/world/map` |
| 32 | `DISP_LIGHTMAP_ALPHAS` | v0; deprecated byte stream | Opaque bytes | `packages/presentation/rendering` |
| 33 | `DISP_VERTS` | v0; 20-byte records: 3×f32 vector, f32 distance, f32 alpha | Ordered displacement-vertex records | `packages/world/map` |
| 34 | `DISP_LIGHTMAP_SAMPLE_POSITIONS` | v0; per sample: one index byte, an additional index byte when the first is 255, then three barycentric bytes; segment starts come from displacement records | Ordered raw stream plus structurally bounded sample records when referenced | `packages/presentation/rendering` |
| 35 | `GAME_LUMP` | v0; counted 16-byte child directory and child payloads defined by [`game-lumps.md`](game-lumps.md) | Ordered game-lump directory and payloads | Selected game module; map, collision, physics, and rendering consume their assigned semantics |
| 36 | `LEAFWATERDATA` | v0; 12-byte padded records: 2×f32 heights, i16 surface texinfo, 2 retained padding bytes | Ordered water records with padding | `packages/world/map` |
| 37 | `PRIMITIVES` | v0; 10-byte records: u8 type, retained pad byte, 4×u16 ranges | Ordered primitive records | `packages/world/map` |
| 38 | `PRIMVERTS` | v0; 12-byte 3×f32 vertices | Ordered primitive vertices | `packages/world/map` |
| 39 | `PRIMINDICES` | v0; u16 indices | Ordered u16 records | `packages/world/map` |
| 40 | `PAKFILE` | v0; ZIP32 local headers, central directory, end record, methods 0 and 14 | Lossless bounded embedded PAK value | Entry lookup: `packages/content`; asset semantics: each format owner |
| 41 | `CLIPPORTALVERTS` | v0; 12-byte 3×f32 vectors | Ordered clip-portal vertices | `packages/world/visibility` |
| 42 | `CUBEMAPS` | v0; 16-byte padded records: 3×i32 origin, u8 size, 3 padding bytes | Ordered cubemap records with padding | `packages/presentation/rendering` |
| 43 | `TEXDATA_STRING_DATA` | v0; exact NUL-terminated string-data bytes | Raw bytes and bounded NUL slices; invalid UTF encoding remains raw | `packages/world/material` |
| 44 | `TEXDATA_STRING_TABLE` | v0; i32 byte offsets into slot 43 | Ordered i32 offsets | `packages/world/material` |
| 45 | `OVERLAYS` | v0; 352-byte records with packed face count/render order, 64 i32 face slots, UV ranges, four UV points, origin, normal | Ordered overlay records retaining unused face slots | `packages/presentation/rendering` |
| 46 | `LEAFMINDISTTOWATER` | v0; u16 values | Ordered u16 records | `packages/world/visibility` |
| 47 | `FACE_MACRO_TEXTURE_INFO` | v0; u16 texture-name indices | Ordered u16 records | `packages/world/material` |
| 48 | `DISP_TRIS` | v0; u16 tag bitsets | Ordered displacement-triangle tags | `packages/world/map` |
| 49 | Source profiles `PHYSCOLLIDESURFACE`; legacy CS:GO `PROP_BLOB` | Source profiles: deprecated Opaque bytes; v21: static-prop triangle/string blob with exact internal record contract Unknown | Opaque bytes; non-empty v21 remains Blocked for complete typed support | `packages/world/collision` |
| 50 | `WATEROVERLAYS` | v0; 1,120-byte records with packed face count/render order, 256 i32 face slots, UV ranges, four UV points, origin, normal | Ordered water-overlay records retaining unused face slots | `packages/presentation/rendering` |
| 51 | `LEAF_AMBIENT_INDEX_HDR` | v0; 4-byte records: u16 sample count/start | Ordered ambient index records | `packages/presentation/rendering` |
| 52 | `LEAF_AMBIENT_INDEX` | v0; 4-byte records: u16 sample count/start | Ordered ambient index records | `packages/presentation/rendering` |
| 53 | `LIGHTING_HDR` | v1; 4-byte RGB-exponent samples | Ordered raw sample records | `packages/presentation/rendering` |
| 54 | `WORLDLIGHTS_HDR` | Source profiles v0: 88-byte records; legacy CS:GO v21 v1: 100-byte records | Profile-selected ordered world-light records | `packages/presentation/rendering` |
| 55 | `LEAF_AMBIENT_LIGHTING_HDR` | v1 with index: 28-byte records containing six RGB-exponent samples and four position/pad bytes; legacy no-index form: 24-byte light cubes | Profile- and companion-selected ordered ambient records | `packages/presentation/rendering` |
| 56 | `LEAF_AMBIENT_LIGHTING` | v1 with index: 28-byte records; legacy no-index form: 24-byte light cubes | Profile- and companion-selected ordered ambient records | `packages/presentation/rendering` |
| 57 | `XZIPPAKFILE` | Deprecated Xbox XZIP bytes | Opaque bytes classified Intentionally inert for declared platforms | None; Xbox is excluded |
| 58 | `FACES_HDR` | v1; 56-byte face records | Ordered HDR face records | `packages/world/map`; HDR draw use belongs to rendering |
| 59 | `MAP_FLAGS` | v0; one u32 level-flags record | Raw u32 bitset | Selected game module and `packages/presentation/rendering` |
| 60 | `OVERLAY_FADES` | v0; 8-byte records containing two f32 squared distances | Ordered fade records | `packages/presentation/rendering` |
| 61 | Source profiles unnamed; legacy CS:GO `OVERLAY_SYSTEM_LEVELS` | Source profiles: Opaque, normally empty; v21: 4-byte records containing min/max CPU and GPU levels | Profile-selected opaque or system-level records | `packages/presentation/rendering` |
| 62 | Source profiles unnamed; legacy CS:GO `PHYSLEVEL` | Source profiles: Opaque, normally empty; v21 exact non-empty record contract Unknown | Opaque bytes; non-empty v21 remains Blocked for complete typed support | `packages/runtime/physics` |
| 63 | Source profiles unnamed; legacy CS:GO `DISP_MULTIBLEND` | Source profiles: Opaque, normally empty; v21: 80-byte records containing two 4×f32 vectors and four 3×f32 color vectors | Profile-selected opaque or displacement-multiblend records | `packages/presentation/rendering` |

An empty slot remains a represented inventory item. A non-empty slot with an unaccepted version or layout is retained as `Unsupported` or `Unknown`; it never falls back to another profile's record size.
