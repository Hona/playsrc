# VTF

## Sample

```ts
import { decodeVtf } from "@playsrc/vtf"

const texture = decodeVtf(bytes)
```

```rust
let texture = playsrc_vtf::decode(&bytes)?;
```

## Objective

Parse and decode Source 1 VTF texture resources.

## Responsibilities

- Validate headers, versions, resources, dimensions, frames, faces, slices, and mip levels.
- Select and decode declared texture subresources.
- Preserve texture metadata required by material and presentation modules.

## Non-Responsibilities

- Resolving VMT material behavior.
- Owning GPU resources or browser texture lifetime.
- Finding textures by logical path.

## Relationships

Consumes exact bytes from `content`; supplies texture data to `material` and presentation modules.

## Completion

Complete when the declared VTF versions, formats, and subresources are represented and verified without renderer assumptions.
