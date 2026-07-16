# Map

## Sample

```ts
import { compileMap } from "@playsrc/map"

const map = await compileMap(content, "maps/pl_upward.bsp")
```

```rust
let map = playsrc_map::compile(&content, "maps/pl_upward.bsp")?;
```

## Objective

Assemble parsed Source data and semantic domain outputs into one canonical playsrc map representation.

## Responsibilities

- Coordinate map-wide references between geometry, materials, models, entities, collision, visibility, lighting, and environment data.
- Validate that a map representation is internally complete and consistent.
- Produce one canonical runtime descriptor directly from verified BSP and dependency bytes.
- Produce an optional reproducible publication descriptor for raw and derived cache objects.

## Non-Responsibilities

- Reimplementing parsers or the semantics of adjacent world packages.
- Adding TF2, ruleset, application, or renderer behavior.
- Storing content-addressed objects.

## Relationships

Composes format and world packages in native and WASM environments. Browsers may compile the representation on first load and cache it in IndexedDB; tools may publish the same derived objects through `asset-store`.

## Completion

Complete when every declared map-domain output is integrated, validated, and consumed without duplicate authorities.
