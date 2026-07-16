# StudioModel

## Sample

```ts
import { loadStudioModel } from "@playsrc/studio-model"

const model = await loadStudioModel(content, "models/player/soldier.mdl")
```

```rust
let model = playsrc_studio_model::load(&content, "models/player/soldier.mdl")?;
```

## Objective

Parse the Source 1 StudioModel file family into a coherent model representation.

## Responsibilities

- Read coordinated MDL, VVD, VTX, and ANI data with explicit version and range validation.
- Represent geometry, skeletons, sequences, animations, attachments, bodygroups, skins, LODs, and flex metadata.
- Preserve cross-file relationships and unsupported values explicitly.

## Non-Responsibilities

- GPU resource ownership or scene rendering.
- Entity animation decisions or TF2-specific model behavior.
- Physics simulation.

## Relationships

Uses `content` to resolve related files and supplies model data to map, game, collision, and presentation modules.

## Completion

Complete when the declared StudioModel format and semantic inventory is represented and verified across supported content.
