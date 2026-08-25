# VTF

## Sample

```rust
let decoder = playsrc_vtf::Decoder::new(&bytes, dialect, limits)?;
let metadata = decoder.metadata();
let plane = decoder.decode(selector)?;
let authored_planes = decoder.decode_high_resolution()?;
let sampling = playsrc_vtf::sampling_state(metadata, sampling_environment);
```

## Objective

Parse and decode Source 1 VTF texture resources.

## Responsibilities

- Validate headers, versions, resources, dimensions, frames, faces, slices, and mip levels.
- Select and decode declared texture subresources.
- Preserve texture metadata required by material and presentation modules.
- Retain raw flags, float bits, ordered resources, custom resource bytes, and every mip/frame/face/slice range before selection; inspect immutable source bytes once and decode the complete authored high-resolution chain in disk order under one aggregate decoded-byte budget.
- Decode selected BGR888, BGRA8888, BC1/DXT1, one-bit-alpha BC1, BC3/DXT5, and native RGBA16F planes required by `jump_beef` without gamma transfer, row flipping, normal reconstruction, precision loss, or PNG packaging.
- Label row zero as top, retain one-bit/eight-bit alpha flags independently from decoded channels, and resolve border/clamp/repeat plus point/linear/mip/trilinear/anisotropic sampling from one explicit PC environment.
- Classify every other image-format code as `Unsupported` or `Unknown` instead of guessing storage or pixels.

## Configured Evidence

`bun packages/world/material/scripts/water-content.ts` verifies the configured TF2 build-`24245096` source hashes and checks all 540 BGR888 shared-normal planes, 270 BC1 stock-water-normal planes, and nine BC3 stock-water-base planes with exact disk ordering, color classification, sampling, alpha flags, and aggregate decoded-byte rejection.

## Non-Responsibilities

- Resolving VMT material behavior.
- Owning GPU resources or browser texture lifetime.
- Finding textures by logical path.

## Relationships

Consumes exact bytes from `content`; supplies texture data to `material` and presentation modules.

## Completion

Complete when the declared VTF versions, formats, and subresources are represented and verified without renderer assumptions.
