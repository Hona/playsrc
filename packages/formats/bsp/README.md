# BSP

## Sample

```ts
import { parseBsp } from "@playsrc/bsp"

const bsp = parseBsp(bytes)
```

```rust
let bsp = playsrc_bsp::parse(&bytes)?;
```

## Objective

Parse compiled Source 1 BSP files into bounded typed map data.

## Responsibilities

- Validate the BSP header, lump directory, versions, ranges, and encoded records.
- Decode supported map lumps without discarding unknown or unsupported values.
- Expose format data needed by map, collision, visibility, entity, material, and lighting consumers.
- Preserve all 64 raw descriptors, exact source ranges, overlaps, map revision, and bounded declared Source LZMA data before semantic interpretation.
- Decode Source-2013 v20 entity, plane, texture-data, vertex, visibility-table, node, texture-info, face, light-sample, leaf, edge, surface-edge, model, leaf-index, brush, brush-side, vertex-normal, compiled-primitive, cubemap, and texture-string records while preserving float bits and padding.
- Frame embedded ZIP32 PAK local and central records, comments, extras, encoded ranges, stored and ZIP-LZMA payloads, CRC-32 results, and unsupported method identities under caller limits.
- Accept complete byte inputs and bounded immutable HTTP-range inputs through one parser contract.

## Non-Responsibilities

- Assembling canonical map/runtime representations or derived cache descriptors.
- Implementing entity behavior, collision queries, visibility policy, or rendering.
- Applying game-specific interpretation.

## Relationships

Consumes BSP bytes obtained through `content`; supplies parsed data to packages in `world`.

## Completion

Complete when the declared BSP format and lump inventory is bounded, represented, and verified without renderer or game dependencies.
