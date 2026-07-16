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

## Non-Responsibilities

- Assembling a playsrc map package.
- Implementing entity behavior, collision queries, visibility policy, or rendering.
- Applying game-specific interpretation.

## Relationships

Consumes BSP bytes obtained through `content`; supplies parsed data to packages in `world`.

## Completion

Complete when the declared BSP format and lump inventory is bounded, represented, and verified without renderer or game dependencies.
