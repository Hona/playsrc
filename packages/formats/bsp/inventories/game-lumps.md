# BSP Game-Lump Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

Authority identity: Valve Source SDK 2013 `src/public/bspfile.h`, `gamebspfile.h`, `src/utils/vbsp/staticprop.cpp`, `detailobjects.cpp`, `src/utils/vrad/vraddetailprops.cpp`, and client detail-prop loader at commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; srctools v2.6.1 tag `814d2cb2f897c99507b9e246c61228c93368134f`; public legacy CS:GO BSP documentation; exact declared-build game-lump directories Missing.

Generator command: Missing. Candidate identity count: 4. Candidate layout-profile count: 10. Accepted item count: 0.

## Directory Contract

Slot 35 begins with a signed 32-bit child count followed by that many 16-byte entries in encoded order.

| Entry byte range | Representation |
|---|---|
| `0..4` | Four-byte ID serialized as a little-endian integer. |
| `4..6` | u16 flags; bit `0x0001` declares Source LZMA compression. Other bits are retained and classified. |
| `6..8` | u16 payload version. |
| `8..12` | signed 32-bit absolute payload offset within the BSP. |
| `12..16` | signed 32-bit decoded length for compressed children; encoded length for uncompressed children. |

For a compressed child, the encoded extent is the distance to the next strictly later in-range directory offset, or to the end of slot 35 for the final extent. The extent must contain one complete Source LZMA envelope. Entry order and duplicate IDs are retained. The parser never selects one duplicate as authoritative.

Valve's symbolic four-character literals serialize to reversed byte order on little-endian files. Both forms below identify the same u32 value.

| Symbolic ID | On-disk bytes | Payload family |
|---|---|---|
| `dprp` | `70 72 70 64` (`prpd`) | Detail props |
| `dplt` | `74 6c 70 64` (`tlpd`) | LDR detail-prop lighting |
| `sprp` | `70 72 70 73` (`prps`) | Static props |
| `dplh` | `68 6c 70 64` (`hlpd`) | HDR detail-prop lighting |

## Accepted Layout Profiles

| Stable identity | ID | Version | Record contract | Declared profile | Semantic owner |
|---|---|---:|---|---|---|
| `detail-props-v4` | `dprp` | 4 | i32 model-name count + 128-byte names; i32 sprite count + 32-byte sprite records; i32 object count + 52-byte object records | Source-2013 v19/v20; legacy CS:GO occurrence requires generated confirmation | Placement: `packages/world/map`; draw use: rendering |
| `detail-lighting-ldr-v0` | `dplt` | 0 | i32 count + 5-byte records containing four RGB-exponent bytes and one style byte | All accepted profiles subject to generated confirmation | `packages/presentation/rendering` |
| `detail-lighting-hdr-v0` | `dplh` | 0 | i32 count + 5-byte records containing four RGB-exponent bytes and one style byte | All accepted profiles subject to generated confirmation | `packages/presentation/rendering` |
| `static-props-v4-56` | `sprp` | 4 | i32 dictionary count + 128-byte names; i32 u16-leaf count + leaves; i32 instance count + 56-byte instances | Source-2013 compatibility | Placement: map; collision use: collision; draw use: rendering |
| `static-props-v5-60` | `sprp` | 5 | v4 plus f32 forced-fade scale | Source-2013 compatibility | Map, collision, rendering |
| `static-props-v6-64` | `sprp` | 6 | v5 plus u16 minimum and maximum DX levels; legacy u8 flags remain at byte 31 | Source-2013 compatibility | Map, collision, rendering |
| `static-props-source-v7-72` | `sprp` | 7 | Source-2013 lightmapped layout: u8 solid, pad byte, i32 skin, distances/origin/scale, u16 DX levels, u32 flags, 2×u16 lightmap resolution | TF2 compatibility; occurrence Missing | Map, collision, rendering, `games/tf2` |
| `static-props-source-v10-72` | `sprp` | 10 | Same 72-byte Source-2013 lightmapped record selected by profile, not version alone | TF2 and current CS:S; occurrence Missing | Map, collision, rendering, selected game |
| `static-props-csgo-v10-76` | `sprp` | 10 | 76-byte record with legacy u8 flags, four CPU/GPU bytes, four color bytes, u32 disable-Xbox value, and u32 extended flags | Legacy CS:GO; occurrence Missing | Map, collision, rendering, `games/csgo` |
| `static-props-csgo-v11-80` | `sprp` | 11 | 80-byte record with legacy u8 flags, four CPU/GPU bytes, four color bytes, u32 extended flags, f32 uniform scale, and retained trailing i32 | Legacy CS:GO; occurrence Missing | Map, collision, rendering, `games/csgo` |

## Structural Invariants

- Every count is non-negative, checked for multiplication and addition overflow, charged to the caller's limits, and fully contained by the child payload.
- Every fixed 128-byte name retains all bytes. A NUL-terminated text view never discards suffix bytes or invalid text encoding.
- Static-prop instance size is selected from profile, game-lump version, and accepted layout identity. Total remaining bytes never infer an unaccepted layout.
- Static-prop dictionary indices and leaf-list ranges remain encoded values. BSP may report out-of-range structural references; map, collision, rendering, and game owners decide semantic behavior.
- Detail and static record padding, unknown bits, the v11 trailing integer, compressed bytes, and trailing payload bytes remain observable.
- An unknown ID or unaccepted ID/version/profile tuple is retained exactly and classified `Unknown` or `Unsupported`. It is never parsed as `sprp` because its byte length happens to divide by a known record size.
