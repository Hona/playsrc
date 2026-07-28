# Source VHV Format Roadmap

[`../../../docs/roadmap-contract.md`](../../../docs/roadmap-contract.md) defines the normative roadmap schema. This leaf owns the canonical little-endian PC VHV version-2 vertex-color format required by the configured `pl_upward` BSP PAK. Console VHV, compressed console payloads, hardware texel `.ppl` data, model parsing, map occurrence association, provider policy, rendering, and GPU resources are excluded.

## Completion Denominator

The denominator contains exactly 8 behavior rows. It is Not accepted because no independent denominator review records reviewer identity, review date, reviewed commit, and all required review predicates.

## Inputs

- Valve Source SDK 2013 commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`: `src/public/materialsystem/hardwareverts.h`, `src/utils/vrad/vradstaticprops.cpp`, `src/utils/vrad/lightmap.cpp`, and `src/utils/common/bsplib.cpp`.
- Configured TF2 content build `24245096`, patch `10828683`, map SHA-256 `15cbf91981b0d9902c645d1992d196b7e630742aa85111ed834d231f3c3a5709`, its exact BSP PAK index, static-prop version-10 records, and provider-plan model bytes.
- Caller-supplied expected MDL checksum and explicit parse limits.

## Outputs

- One immutable VHV value containing profile, source SHA-256/length, header, alignment ranges, ordered mesh records, typed Source vertex-light BGRA8 access, and empty/populated disposition.
- Structured failures containing classification, exact source range, and mesh ordinal when applicable.
- A generated configured inventory under `sourceCacheDir`; no Source bytes or machine paths enter the public repository.

## Invariants

- The selected profile has no magic, version 2, a packed 40-byte header, packed 28-byte mesh records, little-endian integer fields, `VERTEX_COLOR == 0x0004`, four-byte BGRA records, and 512-byte payload/file alignment.
- File and mesh reserved words, alignment padding, and every alpha byte are zero/zero/255 respectively.
- Mesh data ranges are source-relative, sequential, nonoverlapping, and ordered exactly as their mesh headers. Mesh vertex counts sum exactly to the file-header total.
- A VHV mesh identifies only flattened ordinal, LOD, local vertex ordinal, count, and byte range. Body, model, Studio mesh, strip group, original mesh vertex, static-prop origin, and instance identity are absent and never inferred.
- Encoded RGB channels use Source vertex-light transfer space. Normalizing by 255 does not convert them to linear light.
- Every count, multiplication, addition, range, retained allocation, and source length passes caller limits and checked arithmetic before allocation or access.

## Behavior Families

| Source/TF2 behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| VHV version 2 has exact no-magic header identity, checksum, vertex flags/stride, totals, mesh count, and reserved fields. | Explicit profile and typed header validate every field and retain its exact range. | Golden header plus version, checksum, flag, stride, count, reserved, truncation, and byte-order mutations pass. | Ready |
| Mesh records retain flattened order, LOD, vertex count, source-relative offset, and reserved fields. | Ordered typed mesh records preserve ordinals and exact header/data ranges without inventing model hierarchy IDs. | Multi-LOD/multi-mesh golden vector plus count, LOD, offset, reserved, range, and ordering mutations pass. | Ready |
| Vertex lighting is four-byte BGRA in Source vertex-light encoding with opaque alpha. | Typed color access exposes encoded channels, exact normalized encoded values, alpha meaning, local ordinal, and source range. | Channel-boundary golden values and per-channel/alpha mutations pass; all 2,480 configured objects admit opaque alpha only. | Ready |
| Writer output places one sequential stream after 512-byte header alignment and aligns the exact file extent to 512 bytes. | Parser rejects nonzero padding, gaps, duplicate/overlapping ranges, misalignment, truncation, and trailing bytes. | One mutation per padding, gap, duplicate, overlap, start/end alignment, truncation, and trailing invariant passes; every configured object is canonical. | Ready |
| Empty output remains distinguishable from populated lighting. | Canonical zero-mesh/zero-vertex output is `Empty`; all other accepted output is `Populated`. | Exact 512-byte empty vector and contradictory empty/count mutations pass; the configured set contains zero empty objects. | Ready |
| Every admitted parse obeys caller file, mesh, vertex, LOD, and retained-allocation limits. | Limit validation and checked arithmetic precede source retention and record allocation. | Equal/below/above and invalid-limit vectors for every limit pass. | Ready |
| Parsing is deterministic, immutable, and source-identifiable. | Repeated parsing returns equal values, retains SHA-256/length and exact ranges, and cannot mutate caller bytes. | Repeated parse equality, unchanged-input comparison, source hash, and range reconstruction pass. | Ready |
| The configured map has no silently skipped VHV family and every file checksum joins its exact static-prop model occurrence. | One checked command inventories every indexed VHV, classifies every parse result, verifies LDR/HDR occurrence closure and MDL checksums, and reproduces identical JSON. | All 2,480 objects parse as one profile; 2,480 checksum joins and repeated report SHA-256 `7da487336e78d9b88c025af5e2b62f71e0ca133e3818847576705b1c7811d720` pass. | Ready |

## Configured Inventory

`bun packages/formats/vhv/scripts/verify-configured.ts` writes the generated report to `sourceCacheDir/evidence/vhv/pl_upward-inventory.json` and requires two byte-identical generations before publication.

| Field | Exact result |
|---|---|
| Content identity | TF2 build `24245096`, patch `10828683`; `maps/pl_upward.bsp` SHA-256 `15cbf91981b0d9902c645d1992d196b7e630742aa85111ed834d231f3c3a5709`; `tf2_misc_dir.vpk` SHA-256 `63f7db0d1c509e303ca9002fee9e3d805e9220ea5afdd639d8a6b68b8a3710b9` |
| VHV closure | 2,480 objects, 14,022,656 bytes, one `source-pc-v2-color-bgra8888` family, zero unsupported objects |
| Static-prop join | 234 model dictionary rows; 1,244 occurrences; 1,240 lit occurrences; 4 `NO_PER_VERTEX_LIGHTING` occurrences; 2,480 successful MDL-checksum joins |
| File bounds | 1,024..102,912 bytes; 32..25,578 total vertices; 1..6 meshes |
| Mesh bounds | LOD 0..3; 32..9,765 vertices; source offset 512..89,840 |
| Byte identities | 1,271 distinct SHA-256 values; 1,207 repeated-identity groups; 1,209 repeated objects |
| Inventory identity | SHA-256 `1d50693b4ead6c185ac03166d530c1c70b12562cc33f18a7375a6e6b5729699c` over sorted logical path, decimal byte length, object SHA-256, and LF records |
| Repeated report identity | 1,574,972 bytes; SHA-256 `7da487336e78d9b88c025af5e2b62f71e0ca133e3818847576705b1c7811d720` |

## Exit Criteria

- All 8 behavior rows are Ready.
- Rust format, test, and Clippy with warnings denied pass for the package and workspace tests pass.
- The checked configured command emits byte-identical reports twice and parses or explicitly classifies exactly 2,480 indexed VHV objects with no skip.
- Public changes contain no Source assets, machine paths, private observations, or raw transcripts.
- No BSP, Map, StudioModel, Rendering, tool, application, or game integration is introduced.

## Blockers

- Denominator review is Missing. Implementation evidence does not self-accept the denominator.
