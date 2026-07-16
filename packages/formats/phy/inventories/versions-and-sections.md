# PHY Version And Section Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

Authority identity: Valve Source SDK 2013 commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`, Valve Developer Community `PHY` revision `462589`, and exact archive indexes plus source bytes for one accepted TF2, CS:S, and legacy Source 1 CS:GO content build. The three content-build inputs are Missing.

Generator command: Missing.

Candidate item count: 33. Accepted item count: 0.

Candidate dispositions: 25 Candidate required, 7 Blocked, and 1 Excluded.

`Candidate required` means the identity belongs to the manually derived target contract; it does not mean playsrc implements it or that the inventory is accepted. `Blocked` means an exact declared-build occurrence, layout, or typed contract is missing. Every row becomes a denominator item only after a checked-in generator emits this file and a denominator review accepts it.

| Stable identity | Kind | Encoded discriminator or boundary | Required representation | Declared scope | Candidate disposition |
|---|---|---|---|---|---|
| `phy.container.standalone-v16-le` | Container profile | Four little-endian signed 32-bit fields; `size == 16` | Exact header bytes, ID, positive solid count, checksum, and following solid/keydata ranges | Same-stem model `.phy` assets | Candidate required |
| `phy.container.solid-stream-i32-le` | Payload profile | Exactly the declared solid count; each body preceded by a signed 32-bit byte length | Ordered length/body ranges, exact bytes, and keydata boundary | Standalone and caller-supplied collision payloads | Candidate required |
| `phy.container.supplied-vcollide-payload` | Input profile | Separate collision bytes, keydata bytes, and solid count supplied by an owning container parser | Same solid and keydata representation as standalone PHY without inventing a file header | BSP lump 29 payloads after BSP-owned `dphysmodel_t` framing | Candidate required |
| `phy.solid.vphy-0100-poly` | Solid encoding | `VPHY`, version `0x0100`, model type `0` | 28-byte modern metadata, compact polygon surface, drag areas, and axis-map extent | New-format model and BSP collision payloads | Candidate required |
| `phy.solid.legacy-ivps` | Solid encoding | Compact surface with legacy discriminator `IVPS` | Complete legacy polygon surface and no invented modern metadata | Target occurrence Unknown | Blocked: declared-build occurrence is Missing |
| `phy.solid.legacy-zero` | Solid encoding | Compact surface with zero legacy discriminator | Complete legacy polygon surface and explicit zero-tag identity | Target occurrence Unknown | Blocked: declared-build occurrence is Missing |
| `phy.solid.vphy-0100-mopp` | Solid encoding | `VPHY`, version `0x0100`, model type `1` | Exact raw body and a typed representation only after its serialized layout is established | Target occurrence Unknown | Blocked: layout and declared-build occurrence are Missing |
| `phy.solid.vphy-0100-ball` | Solid encoding | `VPHY`, version `0x0100`, model type `2` | Exact raw body and explicit model-type identity | Target occurrence Unknown | Blocked: public serialized layout and declared-build occurrence are Missing |
| `phy.solid.vphy-0100-virtual` | Solid encoding | `VPHY`, version `0x0100`, model type `3` | Exact raw body and explicit model-type identity | Target occurrence Unknown | Blocked: public serialized layout and declared-build occurrence are Missing |
| `phy.solid.byte-swapped` | Solid encoding | Byte-swapped modern or legacy identifier, including `YHPV` or `SVPI` | Header identity and exact bytes only; no accepted geometry output | Console-specific Source asset variants | Excluded: declared browser and native-server targets consume little-endian PC assets |
| `phy.section.modern-metadata` | Binary section | First 28 body bytes of accepted modern polygon solids | Identifier, version, model type, surface size, three drag-axis-area values, and axis-map size | `phy.solid.vphy-0100-poly` | Candidate required |
| `phy.section.compact-surface` | Binary section | First 48 bytes of compact polygon data | Center, inertia, radius, deviation, byte length, root offset, reserved words, and discriminator with exact bits | Modern and accepted legacy polygon solids | Candidate required |
| `phy.section.convex-partition` | Binary section | Rooted 28-byte relative-offset node records | Ordered reached ranges, child/leaf relations, center/radius/bounds metadata, duplicates, and cycle diagnostics | Polygon solids | Candidate required |
| `phy.section.convex-piece` | Binary section | 16-byte piece header at a terminal partition reference | Point-table offset, game data, size/flags, triangle count, raw header, and source range | Polygon solids | Candidate required |
| `phy.section.triangle` | Binary section | Ordered 16-byte records following a convex-piece header | Three point references, material index, virtual flag, adjacency/flag bits, and exact raw bytes | Polygon solids | Candidate required |
| `phy.section.point` | Binary section | 16-byte point record addressed from a convex piece | Four binary32 values, exact bits, encoded identity, and derived Source-space position | Polygon solids | Candidate required |
| `phy.section.axis-map` | Binary section | `axisMapSize` bytes after the modern compact surface | Exact range and bytes; no typed entry claim until the layout is established | Modern polygon solids | Blocked: non-empty layout and occurrence are Missing |
| `phy.section.keydata-tail` | Text section | Bytes after the final standalone solid or separately supplied keydata extent; a required first NUL terminates the document | Exact syntax/trivia/token/block tree, all NUL and suffix bytes, typed accepted blocks, and unknown blocks | Standalone and BSP-supplied payloads | Candidate required |
| `phy.keydata.solid` | Keydata block | ASCII-insensitive block name `solid` | Ordered fields for solid identity and physical parameters plus every unknown entry | Rigid and ragdoll model physics | Candidate required |
| `phy.keydata.staticsolid` | Keydata block | ASCII-insensitive block name `staticsolid` | Same lossless field representation as `solid`, with block identity retained | World/BSP collision payloads | Candidate required |
| `phy.keydata.fluid` | Keydata block | ASCII-insensitive block name `fluid` | Index, surface property, surface plane, current velocity, damping, contents, and unknown entries | World/BSP collision payloads | Candidate required |
| `phy.keydata.materialtable` | Keydata block | ASCII-insensitive block name `materialtable` | Ordered surface-name/index pairs, duplicates, invalid indexes, and exact token bytes | World/BSP collision payloads | Candidate required |
| `phy.keydata.virtualterrain` | Keydata block | ASCII-insensitive block name `virtualterrain` | Complete marker block and nested data without constructing terrain | World/BSP collision payloads | Candidate required |
| `phy.keydata.ragdollconstraint` | Keydata block | ASCII-insensitive block name `ragdollconstraint` | Parent/child indexes and X/Y/Z minimum, maximum, and friction values with clockwise-limit identity | Jointed model physics | Candidate required |
| `phy.keydata.collisionrules` | Keydata block | ASCII-insensitive block name `collisionrules` | Ordered self-collision and collision-pair entries plus unknown data | Jointed model physics | Candidate required |
| `phy.keydata.animatedfriction` | Keydata block | ASCII-insensitive block name `animatedfriction` | Minimum/maximum animated friction and time-in/time-out/time-hold entries | Jointed model physics | Candidate required |
| `phy.keydata.break` | Keydata block | ASCII-insensitive block name `break` | Complete ordered gib-model data and exact unknown fields; no spawning behavior | Model physics keydata | Candidate required |
| `phy.keydata.tf2-spawn` | Keydata block | ASCII-insensitive block name `spawn` | Same lossless record shape used by the public TF2 consumer; no game behavior | TF2 model physics keydata | Candidate required |
| `phy.keydata.editparams` | Keydata block | ASCII-insensitive block name `editparams` | Exact block bytes and ordered entries; typed field set deferred | Model physics keydata | Blocked: complete public field contract and declared-build occurrence are Missing |
| `phy.keydata.custom` | Keydata block family | Any unrecognized top-level block name | Exact name, ordered nested syntax, byte spans, and `Unknown` classification | Every accepted payload | Candidate required |
| `phy.association.same-stem-mdl` | Model association | Caller supplies `.phy` and `.mdl` logical identities with the same extensionless path | Exact pair identity and deterministic mismatch diagnostic | Standalone model PHY | Candidate required |
| `phy.association.mdl-checksum` | Model association | PHY signed 32-bit `checkSum` compared bit-for-bit with supplied MDL checksum | Matched/mismatched result without rewriting either value | Standalone model PHY | Candidate required |
| `phy.association.solid-bone` | Model association | Solid name/parent tokens and constraint indexes compared with supplied skeleton and solid table | Ordered resolved, missing, duplicate, self, and out-of-range diagnostics | Jointed model PHY | Candidate required |

The future generator must enumerate every indexed `.phy` file, its exact same-stem `.mdl` candidate, and every declared BSP lump 29 payload from the three selected content builds. It must record occurrence counts and retained provenance for each stable identity, add each new discriminator/version/model type/section/block/association state as a classified row, and fail rather than omit malformed or unknown input.
