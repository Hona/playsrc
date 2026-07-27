# BSP Roadmap

[`../../../docs/roadmap-contract.md`](../../../docs/roadmap-contract.md) defines the normative roadmap schema and denominator review gate. [`../../../TERMINOLOGY.md`](../../../TERMINOLOGY.md) defines delivery status, coverage classification, inventory, and completion terms.

## Completion Denominator

This leaf roadmap contains exactly 11 behavior rows. The candidate inventories contain 3 container profiles, 64 standard lump slots, and 10 game-lump layout profiles. They contribute 0 denominator items until a checked-in generator emits them and a denominator review accepts them. The current denominator is therefore 11 behavior rows. Acceptance of all 77 candidate inventory items would make the denominator 88 items.

The denominator covers bounded parsing and lossless typed representation of little-endian Valve BSP files required by Team Fortress 2, Counter-Strike: Source, and legacy Source 1 Counter-Strike: Global Offensive. It includes the 1,036-byte header, all 64 standard lump slots, game-lump directories and accepted payloads, embedded PAK ZIP data, Source LZMA envelopes, and external lump-file records. It excludes Source 2 and console-specific BSP containers.

The denominator is Not accepted. The prerequisite format inventory is Candidate and the three BSP inventories have no generator or review record.

## Inputs

- One immutable byte sequence resolved at an exact `.bsp` logical path by `packages/content`, with content build, provider identity, source location, and SHA-256 hash.
- One explicit profile from [`inventories/versions.md`](inventories/versions.md): `source-2013-v19`, `source-2013-v20`, or `legacy-csgo-v21`. The parser never guesses a profile from lump contents.
- One required parse-limit value containing maximum input bytes, maximum encoded bytes retained, maximum total decoded bytes, maximum decoded bytes per lump, maximum game-lump entries, maximum PAK entries, maximum decoded PAK bytes, maximum decoded bytes per PAK entry, and maximum compression expansion ratio. Every field is a non-negative integer and every allocation is charged before it occurs.
- Optional external lump-file bytes supplied with their exact `<map>_l_<index>.lmp` logical identity, index from 0 through 127, expected base-map revision, provenance, and content hash.
- Valve Source SDK 2013 commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`: `src/public/bspfile.h`, `gamebspfile.h`, `lumpfiles.*`, `zip_uncompressed.h`, `zip_utils.*`, `tier1/lzmaDecoder.h`, and `src/utils/common/bsplib.cpp`.
- The public legacy CS:GO header snapshot in AlliedModders `hl2sdk` commit `9cf2f325ea273559c7cae27b4f98518c18b8e322`, branch path `public/bspfile.h`, plus srctools v2.6.1 tag `814d2cb2f897c99507b9e246c61228c93368134f` for branch and static-prop layout discrimination.
- The public Valve Developer Community `BSP (Source)` and `BSP (Source)/Game-Specific` contracts. Their immutable revision identifiers are Unknown.

## Outputs

- An immutable BSP container value retaining the exact source bytes, file identifier, selected profile, container version, signed 32-bit map revision, all 64 raw 16-byte directory entries, and one ordered lump value per slot.
- Each lump value retains its slot, profile identity, signed 32-bit version, raw fourth directory field, encoded range, encoded bytes, compression metadata, optional bounded decoded bytes, typed records when the selected profile defines them, unconsumed tail bytes, overlap relations, and coverage classification.
- An ordered game-lump directory retaining every four-byte identifier, flags, version, absolute encoded offset, declared decoded length, derived encoded extent, encoded bytes, optional decoded bytes, typed payload, and classification. Duplicate identifiers remain distinct entries.
- An embedded PAK value retaining ZIP local headers, central-directory headers, end record, comments, extra fields, ordered entries, raw path bytes, encoded payloads, optional decoded payloads, compression method, CRC-32, and classification.
- An external lump-file value retaining its five signed 32-bit header fields, replacement bytes, source identity, and base-map identity check.
- A structured error naming the exact byte range, slot or child identifier, violated invariant, limit value, declared value, and `Malformed`, `Unsupported`, `Unknown`, or `Missing` classification.

## Invariants

- All accepted numeric fields are decoded little-endian. Raw bytes remain available for exact re-emission, including padding and unknown bit patterns.
- An accepted BSP header is exactly 1,036 bytes before payload data: four identifier bytes, one signed 32-bit container version, 64 directory entries of 16 bytes, and one signed 32-bit map revision.
- The selected profile and the encoded container version must match [`inventories/versions.md`](inventories/versions.md). Version equality never substitutes for profile identity where record layouts differ by game branch.
- Every non-empty encoded range uses widened checked addition and lies within the supplied byte sequence. Negative offsets, negative lengths, arithmetic overflow, and truncation are Malformed.
- Standard lumps may appear in any file order and may be unaligned. Partial overlaps and exact duplicate ranges are retained and reported; they are never merged, copied from a neighboring descriptor, or silently rejected solely for overlapping.
- A zero-length lump retains its complete directory entry. Its offset, version, and fourth field are not normalized.
- Four-byte alignment is an emitter convention, not a parser acceptance condition. Alignment residue is observable metadata.
- `source-2013-v19` and `source-2013-v20` interpret a nonzero fourth directory field as declared decoded size and require a complete Source LZMA envelope whose identifier, decoded size, encoded size, five property bytes, payload extent, decoder result, and configured limits agree. `legacy-csgo-v21` preserves the field as four raw bytes and does not infer compression from payload magic.
- Typed fixed-record lumps require exact divisibility by the selected profile's record size. Counted and offset-based records require checked table sizes and in-range child extents. Semantic indexes are retained as encoded; their world, game, physics, visibility, or presentation owner decides their meaning.
- Unknown standard-lump versions, unknown game-lump identifiers, unknown flag bits, and unsupported PAK methods retain exact bytes and receive a coverage classification. No accepted byte range is omitted from output.
- Parsing and lossless serialization are inverse operations over an unmodified container: serializing a successful parse returns bytes identical to the input.
- Decoding is lazy or budget-charged. No header count, declared decoded size, PAK size, game-lump size, or record count causes allocation before overflow, range, and caller-supplied limit checks pass.
- A parser error returns no partially authoritative container. Diagnostic metadata may retain validated identities and ranges but cannot be consumed as parsed map data.

## Ownership Exclusions

- Provider ordering, exact logical-path resolution, BSP selection, PAK entry lookup, and external lump-file selection belong to `packages/content`.
- Canonical map assembly, face topology, displacement surface construction, brush-model assembly, area topology, water volumes, static-prop placement, and detail-prop placement belong to `packages/world/map`.
- Material-name interpretation, texture transforms as material inputs, surface-property meaning, and material dependency resolution belong to `packages/world/material`.
- Entity-lump tokenization, ordered key/value semantics, entity classes, entity I/O, spawning, and entity behavior belong to `packages/world/entity` and the selected game module.
- Brush solidity, contents meaning, trace acceleration, collision queries, static-prop collision use, and prop-hull use belong to `packages/world/collision`.
- PVS/PAS expansion, cluster membership, area-portal state, visibility queries, and culling policy belong to `packages/world/visibility`.
- VPhysics payload decoding, collision-shape construction, material properties, mass, constraints, and simulation belong to `packages/runtime/physics`.
- Lightmap interpretation, world-light evaluation, ambient-light use, overlay projection, cubemap use, prop lighting, and GPU-resource construction belong to `packages/presentation/rendering`.
- Studio model paths and instances remain BSP records; MDL/VVD/VTX/ANI/VHV parsing belongs to `packages/formats/studio-model`.
- TF2-, CS:S-, and legacy CS:GO-specific record meaning, flags, entities, and gameplay effects belong to `games/tf2`, `games/css`, and `games/csgo` respectively.
- BSP creation, repacking, mutation, external-lump application, and map compilation belong to `tools/*`; immutable compiled map artifacts belong to `packages/asset-store`.

## Behavior Families

The active HDR compiler profile checkpoint consumes the exact retained descriptors and bytes for selected faces, RGBExp32 lighting, world lights, leaf-ambient indexes and samples, map flags, and game-lump detail/static-prop lighting inputs. BSP remains the byte/container authority; Map owns profile completeness, cross-record sample validation, linear radiance projection, and artifact serialization. The checkpoint must prove that selected slot identities reach the output without LDR/HDR fallback.

| Target behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| A caller-selected accepted profile validates `VBSP`, container version, the fixed 64-entry directory, and the signed map revision without branch guessing. | The Rust parser requires the explicit `source-2013-v20` profile, validates `VBSP` and version 20, retains all 64 descriptors and signed revision, and returns typed failures. V19/V21 profiles and accepted inventories remain absent. | Synthetic exact, bad-identifier, version-mismatch, and truncated-header vectors pass; the exact `jump_beef` BSP parses as version 20, revision 731, with 64 lumps. | Partial |
| Every standard lump descriptor is range-checked while preserving file order independence, unaligned offsets, partial overlaps, exact duplicate ranges, zero-length metadata, and the raw fourth field. | All descriptors retain signed version, raw fourth bytes, exact encoded range, alignment residue, and sorted overlap identities; negative, overflow, truncation, and encoded-budget failures publish no BSP. The complete boundary matrix remains absent. | Synthetic unaligned, zero-length/nonzero-metadata, partial-overlap, exact-overlap, negative, truncated, and budget vectors pass; byte-exact source retention is asserted. | Partial |
| Source-profile compressed standard lumps decode the 17-byte Source LZMA envelope only when the directory declares compression, and retain both encoded and decoded bytes. | A nonzero fourth field on a nonempty lump requires a complete Source LZMA envelope; sizes, five properties, payload extent, per-lump/total budget, ratio, decode result, encoded bytes, and decoded bytes are retained or rejected. Broader malformed-envelope mutation evidence remains absent. | A generated fixed LZMA vector compares encoded and decoded bytes; wrong magic/declaration and limit vectors fail. The exact `jump_beef` BSP reports zero compressed standard lumps. | Partial |
| All 64 standard slots produce the typed or explicitly opaque representations in [`inventories/lumps.md`](inventories/lumps.md), including entity bytes, geometry, displacements, overlays, lighting, visibility bytes, leaf data, physics framing, and branch-specific slots. | Source-2013 v20 records consumed by the declared targets are typed: entities, planes, texture data, vertices, visibility framing, nodes, texture info, faces, lighting, leaves, edges, surface edges, models, leaf-face and leaf-brush indexes, brushes, brush sides, 176-byte displacement infos with complete neighbor/corner/allowed-vertex fields, 20-byte displacement vertices, triangle tags, vertex normals and indexes, compiled primitives/vertices/indices, PAK, cubemaps, texture string data and offsets, HDR lighting, and HDR faces. Every non-empty unimplemented slot remains exact opaque bytes classified `Unknown`; an implemented slot with an unhandled version is `Unsupported`. | Fixed record vectors plus configured `pl_upward` compare 558 infos, 14,174 vertices, 18,240 tags, 554 power-2 patches and four power-3 patches. Complete accepted-profile inventory coverage remains required. | Partial |
| Counted, indexed, and variable records validate only structural arithmetic owned by BSP and retain semantic references for their downstream owner. | Fixed records, displacement sample streams, and the visibility cluster count, ordered PVS/PAS offset pairs, compressed-byte tail, range, and record budget are implemented. Occlusion sections, physics blocks, game-lump semantic payloads, and remaining variable records are not implemented. | Visibility and displacement vectors cover exact framing, truncation, negative count, record size and count budgets. | Partial |
| The game-lump parent validates its count and directory, derives compressed extents without overflow, preserves entry order and duplicates, and exposes unknown entries losslessly. | No BSP implementation exists. | Game-lump vectors with zero and maximum configured counts, unsorted entries, duplicate IDs, duplicate ranges, unknown flags, compressed children, and final-child extents; compare every directory field and retained byte exactly. | Not started |
| The four required game-lump identities and ten accepted layout profiles in [`inventories/game-lumps.md`](inventories/game-lumps.md) decode exact dictionaries, leaf lists, records, counts, padding, flags, and tails. | No BSP implementation exists; declared-build indexes are Missing and public sources disagree when static-prop version numbers share different record sizes. | Fixed TF2, CS:S, and legacy CS:GO maps plus one vector per accepted layout: compare counts, record sizes, every field's raw bits, and lossless bytes; require version-plus-profile-plus-size discrimination and exact Unsupported results for unaccepted combinations. | Blocked |
| Lump 40 parses bounded ZIP32 PAK data with methods 0 and 14 while retaining archive order, raw names, duplicate names, comments, extras, local/central metadata, encoded payloads, and CRC results. | ZIP32 local, central, and end records retain source order, raw names, extras, comments, local and encoded ranges, method, flags, sizes, and CRC. Methods 0 and 14 decode under per-entry, cumulative, count, and ratio limits; every other method remains `Unsupported`. Multi-disk, encrypted, duplicate-local-offset, inconsistent local/central, truncated, corrupt, or over-limit input fails. Complete malformed and duplicate-name evidence remains absent. | Stored and generated ZIP-LZMA vectors compare decoded bytes; stored vectors retain distinct local/central extras and comments. CRC corruption, entry-count, and decoded-size limits fail. The exact map yields 13 ordered stored entries with valid CRCs. | Partial |
| External files `<map>_l_<index>.lmp` for indexes 0 through 127 parse their five-field header and replacement bytes and validate supplied base-map revision and lump ID without filesystem discovery. | No BSP implementation exists. | One fixed file for each boundary index and vectors for revision mismatch, invalid slot, negative range, overflow, truncation, and trailing bytes; compare header values, replacement bytes, provenance, and deterministic error fields. | Not started |
| Every parse and decode operation checks caller-supplied allocation and expansion limits before allocation and classifies every encountered identity or failure without silent fallback. | Input, cumulative encoded, per-lump decoded, cumulative decoded, standard-lump ratio, records per lump, PAK entries, PAK bytes per entry, cumulative PAK decoded bytes, and PAK ratio limits are checked. Every standard lump is `Handled`, `IntentionallyInert`, `Unsupported`, or `Unknown`. Game lumps and complete allocation instrumentation remain absent. | Fixed input/range/encoded/decoded/ratio/record/PAK vectors pass with stable error fields; full limit-minus/equal/plus evidence remains required. | Partial |
| Successful parsing is lossless and independently consumable by content, map, material, entity, collision, visibility, physics, rendering, tools, and the selected game without creating a second parsing authority. | The container exposes immutable source bytes, descriptors, encoded bytes, and decoded lump bytes without world dependencies. No semantic consumer or WASM binding exists yet. | Synthetic byte-exact retention and isolated real `jump_beef` parsing pass; producer/consumer and duplicate-reader audits remain required. | Partial |

## Generated Inventories

No generated inventory is accepted. Accepted item count: 0.

| Output | Authority identity | Authority revision | Generator command | Candidate items | Accepted items |
|---|---|---|---|---:|---:|
| [`inventories/versions.md`](inventories/versions.md) | Source SDK BSP header; public legacy CS:GO BSP header; exact TF2, CS:S, and legacy CS:GO map indexes | SDK `88fa198fba3fb85d46d4c95018254693fdc3af0a`; public CS:GO header `9cf2f325ea273559c7cae27b4f98518c18b8e322`; map indexes Missing | Missing | 3 | 0 |
| [`inventories/lumps.md`](inventories/lumps.md) | Source SDK `bspfile.h` and `bsplib.cpp`; public legacy CS:GO BSP header; declared-build BSP headers | Same fixed source revisions; declared-build indexes Missing | Missing | 64 | 0 |
| [`inventories/game-lumps.md`](inventories/game-lumps.md) | Source SDK `gamebspfile.h` and game/tool loaders; srctools v2.6.1 static-prop profiles; declared-build game-lump directories | SDK `88fa198fba3fb85d46d4c95018254693fdc3af0a`; srctools `814d2cb2f897c99507b9e246c61228c93368134f`; declared-build indexes Missing | Missing | 10 | 0 |

The future generator is owned by `tools/playsrc`. It must consume retained authority snapshots and exact archive indexes from configured content builds, emit all three files in stable identity order, retain blocked and empty discoveries, and fail on an unclassified container version, standard slot, lump version, four-byte game-lump ID, game-lump layout, nonzero branch slot, or PAK compression method. No command name is declared before that implementation exists.

## Exit Criteria

The BSP package is Complete only when all of these predicates pass:

- All 11 behavior rows are Ready.
- The format-universe inventory and all three BSP inventories have Accepted denominator reviews with current authority revisions, generator command, output paths, item counts, reviewer identity, review date, and reviewed commit.
- The current inventories contain exactly 3 accepted container profiles, 64 standard slots, and the complete generated game-lump profile set for the three declared content builds; no required item is Blocked, Unsupported, Unknown, or Missing.
- Every fixed-record layout, variable table, compressed envelope, game-lump child, PAK entry, external lump file, unknown value, duplicate region, overlapping region, malformed range, integer overflow, truncation, and allocation boundary satisfies its declared evidence predicate.
- Every successful fixed-map parse re-emits bytes identical to its input and retains the same provenance and content hash.
- Every named producer and consumer uses the current BSP interface, and no duplicate reader, fallback, compatibility layer, legacy path, or stale generated inventory remains.
- BSP has no dependency on world, runtime, presentation, game, application, service, or renderer packages.

## Blockers

- **Format-universe prerequisite:** [`../ROADMAP.md`](../ROADMAP.md) and [`../inventories/formats.md`](../inventories/formats.md) remain Candidate and Not accepted. BSP cannot accept its child denominator before the parent assigns the current identities through the required review gate.
- **Inventory generator:** no checked-in command emits the three assigned BSP inventories. Checked `tools/`, root package manifests, and the current BSP package tree.
- **Declared content builds:** exact TF2, CS:S, and legacy Source 1 CS:GO archive indexes are Missing because `playsrc.local.json` and retained map indexes are absent. The generator cannot establish which container, lump, and game-lump profiles occur in the declared builds.
- **Legacy CS:GO slots 49 and 62:** the exact non-empty record contract for `LUMP_PROP_BLOB` and `LUMP_PHYSLEVEL` is Unknown. Checked Source SDK `src/public/bspfile.h`, AlliedModders `hl2sdk` CS:GO `public/bspfile.h` at `9cf2f325ea273559c7cae27b4f98518c18b8e322`, srctools v2.6.1 BSP documentation, and the Valve Developer Community BSP contract.
- **Static-prop profile occurrence:** public contracts establish incompatible records under overlapping `sprp` versions, including 72-byte Source-2013 v10, 76-byte legacy CS:GO v10, and 80-byte legacy CS:GO v11. Exact occurrence by declared content build is Missing. Checked Source SDK `gamebspfile.h`, SDK issue 365, srctools v2.6.1, public BSP branch documentation, and the absent declared-build indexes.
- **Public documentation revision:** the checked Valve Developer Community BSP pages expose no immutable revision through the available contract. Their revision is Unknown until a retained snapshot records a revision ID or SHA-256 content hash.
