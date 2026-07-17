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
- Represent geometry, skeletons, sequences, animations, attachments, bodygroups, skins, LODs, and flex metadata.
- Preserve cross-file relationships and unsupported values explicitly.
- Emit one bounded batch of exact VVD, selected VTX, ANI, include-model, and PHY requests instead of invoking a provider callback from parser loops.
- Retain MDL 44–48 structural metadata and validate VVD 4/VTX 7 checksums, LODs, root tables, and bodypart cardinality before consumers use companion data.
- Assemble runtime-neutral geometry primitives from VVD fixups and VTX bodypart/model/LOD/mesh/strip-group/strip records while retaining source vertices, encoded indices, strip metadata, material slots, switch points, and derived nondegenerate triangles; GLB remains outside the runtime path.
- Decode every root animation's integer frames from inline or ANI block/section data into bone-local translations and quaternions while retaining descriptor and source index metadata; sequence selection and presentation blending remain external.
- Supply LOD-0 bodypart-model-0 primitives and material candidate identities to the direct TF2 browser payload for exact dynamic-prop, rocket/sticky, and Soldier/Demoman viewmodel geometry.

## Non-Responsibilities

- GPU resource ownership or scene rendering.
- Entity animation decisions or TF2-specific model behavior.
- Physics simulation.

## Relationships

Uses `content` to resolve related files and supplies model data to map, game, collision, and presentation modules.

## Completion

Complete when the declared StudioModel format and semantic inventory is represented and verified across supported content.
