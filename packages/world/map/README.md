# Map

## Sample

```rust
let map = playsrc_map::compile(&bsp, lighting_profile)?;
```

## Objective

Assemble parsed Source data and semantic domain outputs into one canonical playsrc map representation.

## Responsibilities

- Coordinate map-wide references between geometry, materials, models, entities, collision, visibility, lighting, and environment data.
- Validate that a map representation is internally complete and consistent.
- Produce one canonical runtime descriptor directly from verified BSP and dependency bytes.
- Produce an optional reproducible publication descriptor for raw and derived cache objects.
- Preserve Source XYZ face/model/material identities; retain signed surfedge and face-side semantics; normalize nondegenerate draw triangles to their supplied normals; and emit direct renderer position, normal, UV, lightmap, primitive, and lighting buffers without serializing through GLB.
- Coordinate authored displacement parent orientation, complete sample/alpha/tag/neighbor/mask metadata, full-resolution Source triangle parity adapted once to the ordinary world front face, quadrant-ordered Source normals, texture/lightmap coordinates, finite visibility bounds, material/model/lighting joins, and collision producer identity in schema `5` without preconversion.
- Produce a separate full-resolution displacement Collision handoff containing every final position, every unfiltered Source-order triangle and original tag, contents, parent/material identity, and exact alpha-sum secondary-surface decision; render `0x20` filtering never mutates this topology.
- Compile one immutable render-BSP surface-lighting query that preserves node/leaf/source-face order, defers touched displacement tests, samples selected HDR average/luxel records, applies reflectivity/sky ambient, and projects one bounded 162-ray phase into `+X,-X,+Y,-Y,+Z,-Z` ambient-cube order.
- Assemble Map, Entity, Collision, and already-verified Visibility once; retain borrowed game-lump source ranges; and size each bounded deterministic native/WASM runtime payload exactly before allocation while preserving BSP, compiler, configuration, and payload SHA-256 identities.
- Accept owner-resolved Material/VTF outputs for the exact map material index and append versioned shader, feature, base-texture identity, dimensions, and RGBA planes to the direct runtime payload without resolving content inside Map.
- Select one complete `ldr` or `hdr` lighting profile without fallback. HDR compilation validates face, RGBExp32 sample, world-light, leaf-ambient, map-flag, detail-prop-lighting, and static-prop-lighting inputs before emitting output.
- Emit HDR samples as linear RGB binary32 without exposure or tone mapping, preserving flat, directional-normal, and directional-SSBump face classifications plus every style identity.
- Finalize one bounded render-neutral environment from selected-profile sky and cubemap requests, water surfaces and leaf volumes, entity decals and compiled overlays, fog/environment controllers, and exact dependency responses. Missing selected resources fail without profile, default-cubemap, or sky substitution.
- Trace each `infodecal` from one revisioned Entity-supplied world placement through an identity-matched Collision world/transformed brush snapshot, project only onto that selected model in receiver-local coordinates, and retain all three producer revisions, receiver transform/identity, visibility admission key, activation, lifetime, normal offset, and decal polygon-offset request.
- Retain leaf-water contents, clusters, areas, exact `u16` minimum distance, exact map-owned or named bottom-material joins, and each selected Material's independent optional environment/reflection/refraction bindings; undefined beneath-water environment remains `None` through surface, volume, and view-plan output. Retain master fog selection and fog transition inputs, and produce ordered cheap-water, reflection, refraction, main, and intersection plans from explicit PVS/area/frustum-qualified leaves and platform policy.
- Retain each surface's texinfo index, stored BSP plane and face side, raw texture/lightmap vectors, top-left UV origin, mapping dimensions, and oriented render normals as separate canonical facts.
- Retain model zero and every inline `*N` as ordered immutable `BrushModelGeometry` records with model-local face ranges, bounds, origin, head node, first-occurrence material indexes, ordered entity references, vertex/triangle counts, exact collision brush IDs, and ORed contents; retain every non-world entity occurrence's source class, raw transform/parent, spawn flags, `StartDisabled`, `Solidity`, and `solidbsp` fields.
- Retain one canonical source-ordered static-prop v10 model/occurrence table with normalized model dependencies, exact transforms, leaf memberships, solidity, inert padding, skins, fades, DX levels, flags, lightmap resolution, and explicit optional lighting origins without publishing partial browser draws.
- Project decals with floating mapping dimensions, stored-plane floor/wall basis, exact receiver exclusions, unit-square UV clipping, and a 0.1-unit normal offset; derive overlay V as `normal × U` and clip each standard/water overlay independently to every retained source face triangle.

## Non-Responsibilities

- Reimplementing parsers or the semantics of adjacent world packages.
- Adding TF2, ruleset, application, or renderer behavior.
- Storing content-addressed objects.
- Creating reflection/refraction render targets, applying fog/exposure/tone mapping, drawing sky/water/marks, or selecting application presentation policy.

## Relationships

Composes format and world packages in native and WASM environments. Browsers may compile the representation on first load and cache it in IndexedDB; tools may publish the same derived objects through `asset-store`.

## HDR Runtime Contract

`PSMP` schema `4` is the HDR-only runtime payload. All integers and binary32 values are little-endian. Every sized field is `u32 byteLength` followed by exactly that many bytes. The common header, materials, surfaces, entity source, resolved world materials, models, and model occurrences retain schema `3` order. The selected-lighting region after surfaces contains exactly `lightingSampleCount × 3` finite linear RGB binary32 components; schema `3` LDR continues to contain `lightingSampleCount × 4` unchanged RGBExp32 bytes.

Schema `4` then appends exactly one `PSHD` descriptor:

1. `u32 version = 1`, `u8 encoding = 1` (`linear-rgb-f32`), and three zero bytes.
2. Sized UTF-8 output role and compiler identity; 32-byte BSP SHA-256, configuration SHA-256, and selected-lighting closure SHA-256.
3. A `u32` member count followed by role-ordered face, sample, world-light, ambient-index, ambient-sample, map-flag, game-lump-directory, detail-prop, selected detail-lighting, and static-prop records. Each record retains source kind/slot or four-byte game-lump ID/version, encoded and decoded byte lengths, both SHA-256 hashes, and item count. An absent optional game member has source kind zero, zero lengths/hashes, and zero items.
4. Lightmapped and directional face counts, then one 20-byte record per BSP face: source face index; kind `0` unlit, `1` flat, `2` directional-normal, or `3` directional-SSBump; style count; layer count `0`, `1`, or `4`; one zero byte; sample start; samples per layer; and all four style bytes.
5. Complete 88-byte world-light records, four-byte ambient-index records, and 76-byte ambient samples containing six linear RGB binary32 directions plus three fractional-position bytes and one zero byte.
6. Detail-prop count, selected detail-style-sample count, static-prop count, and map flags.
7. Profile materials in `rt`, `lf`, `bk`, `ft`, `up`, `dn` order. Each record contains material path, selected shader/features/texture role, selected VTF path/dimensions/format, VTF SHA-256, and unchanged bounded VTF bytes.
8. Role/path/hash-sorted consumed input records. Each contains role, three zero bytes, sized logical path, and SHA-256.

The payload limit is 512 MiB. HDR compilation also limits faces to 1,000,000, RGBExp32 samples to 16,777,216, world lights to 1,000,000, ambient samples to 4,000,000, game-lump entries and dependency hashes to 4,096, profile materials to 64, logical paths to 1,024 bytes, and each retained profile VTF to 64 MiB.

The derived identity is SHA-256 over the ASCII domain `playsrc-derived-map-v1`, profile byte, length-prefixed output role and compiler identity, BSP/configuration/lighting-closure hashes, role/path/hash-sorted consumed inputs, and payload SHA-256. LDR and HDR therefore cannot share a derived identity even when other inputs match.

## Completion

Complete when every declared map-domain output is integrated, validated, and consumed without duplicate authorities.

Complete Map parity evidence runs through `bun packages/world/map/scripts/verify-parity.ts` and requires the repository-root local configuration plus the declared `jump_beef` BSP and source bundle. The narrower brush presentation workflow remains `bun packages/world/map/scripts/verify-brush-model-presentation.ts`.
