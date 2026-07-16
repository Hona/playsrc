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

## Non-Responsibilities

- GPU resource ownership or scene rendering.
- Entity animation decisions or TF2-specific model behavior.
- Physics simulation.

## Relationships

Uses `content` to resolve related files and supplies model data to map, game, collision, and presentation modules.

## Completion

Complete when the declared StudioModel format and semantic inventory is represented and verified across supported content.
