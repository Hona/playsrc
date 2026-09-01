# StudioModel

## Sample

```rust
let phase = playsrc_studio_model::load(
    logical_path,
    profile,
    vtx_variant,
    mdl_bytes,
    &dependency_responses,
    limits,
)?;
```

## Objective

Parse the Source 1 StudioModel file family into one runtime-neutral model representation.

## Responsibilities

- Read coordinated MDL, VVD, VTX, and ANI data with explicit version and range validation.
- Represent geometry, skeletons, hitboxes, sequences, sequence events/autolayers, animations, attachments, bodygroups, skins, LODs, and flex metadata.
- Preserve cross-file relationships and unsupported values explicitly.
- Emit one bounded batch of exact VVD, selected VTX, ANI, include-model, and PHY requests instead of invoking a provider callback from parser loops.
- Resolve include-model identities only as canonical root-relative `models/*.mdl` paths; join authored material directories at one normalized boundary; request ANI only when a descriptor or section references a nonzero external block.
- Retain MDL 44–48 structural metadata and validate VVD 4/VTX 7 checksums, LODs, root tables, and bodypart cardinality before consumers use companion data.
- Retain MDL root-LOD limits and unresolved optional hitbox-name offsets, and join map-owned typed VHV meshes to root-and-lower-LOD hardware strip groups by checksum, LOD, mesh order, and exact vertex count while referencing unchanged BGRA ranges.
- Assemble runtime-neutral geometry primitives from VVD fixups and VTX bodypart/model/LOD/mesh/strip-group/strip records while retaining source vertices, encoded indices, strip metadata, material slots, switch points, and derived nondegenerate triangles; GLB remains outside the runtime path.
- Decode every root/include animation's integer frames from inline or ANI block/section data into bone-local translations and quaternions while retaining section, codec, descriptor, source-index, and local-to-root bone-map metadata; entity-state selection remains external.
- Retain an empty `STUDIO_OVERRIDE` sequence as a forward declaration only when its blend grid and every child count are zero; include composition may replace it by label, while direct sampling remains invalid until replacement.
- Supply LOD-0 bodypart-model-0 primitives and material candidate identities to the direct TF2 browser payload for exact dynamic-prop, rocket/sticky, and Soldier/Demoman viewmodel geometry.
- Preserve selected model-material shader identity so `UnlitGeneric` banner models remain categorically lighting-excluded rather than entering the VertexLitGeneric lighting path.
- Compile one deterministic, content-addressable `PSMP` v4 artifact below 64 MiB from an explicitly selected world or viewmodel profile, authored collision hull bounds, exact model/material/texture dependencies, composed include-model animation data, geometry, skins/bodygroups, skeletons, hitboxes, poses, sequences/activities/events/autolayers, attachments, PHY disposition, and explicit unsupported-feature classifications.
- Retain dense integer animation frames unchanged when they fit; otherwise encode the same frames as bounded per-bone constant-or-sampled translation/quaternion channels inside the current artifact format. Sampling and decoding accept both canonical frame-block tags without expanding the compact block persistently.
- Retain every dependency occurrence descriptor while charging immutable source bytes once per exact logical-path/SHA-256/byte-length identity.
- Preserve authored Source positions, normals, tangent-S/handedness, and UVs without axis swaps or texture-coordinate flips; apply supplied entity origin plus pitch/yaw/roll through the Source forward/left/up column matrix.
- Preserve VTX list order and Source triangle-strip parity. Presentation descriptors declare clockwise encoded front faces, back-face culling, flex-before-linear-skinning with unchanged topology, and determinant-sign reversal; zero/non-finite determinants fail and no model identity changes culling.
- Sample a caller-selected base sequence, pose-parameter vector, time, and ordered animation layers into model-space skinning matrices and attachment transforms, including sequence-authored local/non-local/pose/spline/crossfade/no-blend autolayers, exact sequence FPS/CPS/duration, static-prop bone-zero behavior, world-aligned attachments, and ordered presentation event crossings.
- Emit categorical world and viewmodel descriptors. The viewmodel descriptor carries TF2's configurable horizontal-4:3 FOV default `54`, near plane `1`, supplied world far plane, post-world opaque/translucent ordering, `[0, 0.1]` depth range, and optional view-space-Y handedness reflection; gameplay/rendering supply current state and execute GPU policy.
- Retain complete Studio eyeball records and secondary-header illumination/eye fields; derive per-eye origin and iris/glint projection rows only from supplied bone-to-world matrices, world eye target, view basis, and explicit eye configuration.
- Validate renderer-neutral model lighting as one lighting origin, six `+X,-X,+Y,-Y,+Z,-Z` ambient colors, at most four point/directional/spot lights, camera position, optional local environment identity, and static-light facts.
- Compose an explicitly selected hand-viewmodel activity with a parented item model by copying every ASCII-insensitive matching hand-bone matrix, reconstructing unmatched item bones under their authored hierarchy, and selecting one shared numeric skin plus explicit bodygroups/LOD for both draw parts.
- Produce one complete renderer-neutral viewmodel frame from an explicit draw/fire/reload-start/reload-insert-or-loop/reload-finish/idle phase, translated activity, sequence interval, pose, skin, bodygroups, LOD, material opacity, draw eligibility, occurrence orientation, and optional reflection. Output retains all crossed authored events, applicable C-model bodygroup mutations, hand/item/ammunition-bearing geometry and bones, timing, draw partitions/order, and effective facing.
- Retain the configured resupply locker's exact `idle`, `open`, and `close` sequences plus its single `Body` submodel; callers supply the current sequence, cycle, and bodygroups from game/entity state.

## Non-Responsibilities

- GPU resource ownership, scene rendering, or reinterpretation of authored model-space vertex attributes.
- Entity animation/activity decisions, current FOV/far plane/handedness choice, or other TF2-specific model state.
- TF2 item-schema, class-hand, activity-remap, team-skin, and bodygroup selection. Callers supply those resolved facts to the generic composition operation.
- Material-script parsing or shader/texture-role interpretation; the VMT and material owners supply an exact resolved dependency manifest at the StudioModel artifact boundary.
- Physics simulation.

## Relationships

Uses `content` to resolve related files and supplies model data to map, game, collision, and presentation modules.

## Completion

Complete when the declared StudioModel format and semantic inventory is represented and verified across supported content.

## Licensing

`rust/src/viewmodel.rs` includes behavior adapted from Valve's Source SDK 2013 and is governed by the Source 1 SDK License retained in this repository. All other original package material remains under the repository MIT license unless identified otherwise.
