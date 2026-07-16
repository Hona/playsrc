# Rendering

## Sample

```ts
import { createRenderer } from "@playsrc/rendering"

const renderer = createRenderer(canvas)
await renderer.loadMap(map)
renderer.render(snapshot)
```

## Objective

Render canonical Source world, gameplay, and replay state in browser GPU environments.

## Responsibilities

- Own scenes, views, GPU resources, draw preparation, lighting presentation, and frame pacing.
- Consume map, model, material, visibility, particle, gameplay, and replay state.
- Derive presentation-only interpolation without changing authoritative state.

## Non-Responsibilities

- Parsing Source formats or defining material semantics.
- Advancing gameplay, replay, entities, particles, or audio truth.
- Becoming the canonical owner of world data.

## Relationships

Consumes world and presentation packages plus game-owned presentation mappings; applications own UI and product composition.

## Completion

Complete when the declared Source presentation families are integrated and supported by aligned visual evidence.
