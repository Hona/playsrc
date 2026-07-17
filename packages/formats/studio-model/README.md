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
- Assemble runtime-neutral geometry primitives from VVD fixups and VTX bodypart/model/LOD/mesh/strip-group/strip records while retaining source vertices, encoded indices, strip metadata, material slots, switch points, and derived nondegenerate triangles; GLB remains outside the runtime path.
- Decode every root/include animation's integer frames from inline or ANI block/section data into bone-local translations and quaternions while retaining section, codec, descriptor, source-index, and local-to-root bone-map metadata; entity-state selection remains external.
- Retain an empty `STUDIO_OVERRIDE` sequence as a forward declaration only when its blend grid and every child count are zero; include composition may replace it by label, while direct sampling remains invalid until replacement.
- Supply LOD-0 bodypart-model-0 primitives and material candidate identities to the direct TF2 browser payload for exact dynamic-prop, rocket/sticky, and Soldier/Demoman viewmodel geometry.
- Compile one deterministic, content-addressable `PSMP` v2 artifact below 64 MiB from an explicitly selected world or viewmodel profile, exact model/material/texture dependencies, composed include-model animation data, geometry, skins/bodygroups, skeletons, hitboxes, poses, sequences/activities/events/autolayers, attachments, PHY disposition, and explicit unsupported-feature classifications.
- Retain dense integer animation frames unchanged when they fit; otherwise encode the same frames as bounded per-bone constant-or-sampled translation/quaternion channels inside the current artifact format. Sampling and decoding accept both canonical frame-block tags without expanding the compact block persistently.
- Retain every dependency occurrence descriptor while charging immutable source bytes once per exact logical-path/SHA-256/byte-length identity.
- Preserve authored Source positions, normals, tangent-S/handedness, and UVs without axis swaps or texture-coordinate flips; apply supplied entity origin plus pitch/yaw/roll through the Source forward/left/up column matrix.
- Sample a caller-selected base sequence, pose-parameter vector, time, and ordered animation layers into model-space skinning matrices and attachment transforms, including sequence-authored local/non-local/pose/spline/crossfade/no-blend autolayers, exact sequence FPS/CPS/duration, static-prop bone-zero behavior, world-aligned attachments, and ordered presentation event crossings.
- Emit categorical world and viewmodel descriptors. The viewmodel descriptor carries TF2's configurable horizontal-4:3 FOV default `54`, near plane `1`, supplied world far plane, post-world opaque/translucent ordering, `[0, 0.1]` depth range, and optional view-space-Y handedness reflection; gameplay/rendering supply current state and execute GPU policy.

## Non-Responsibilities

- GPU resource ownership, scene rendering, or reinterpretation of authored model-space vertex attributes.
- Entity animation/activity decisions, current FOV/far plane/handedness choice, or other TF2-specific model state.
- Material-script parsing or shader/texture-role interpretation; the VMT and material owners supply an exact resolved dependency manifest at the StudioModel artifact boundary.
- Physics simulation.

## Relationships

Uses `content` to resolve related files and supplies model data to map, game, collision, and presentation modules.

## Completion

Complete when the declared StudioModel format and semantic inventory is represented and verified across supported content.
