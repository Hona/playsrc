# Rendering

## Sample

```ts
import { createRenderer } from "@playsrc/rendering"

const renderer = await createRenderer(canvas)
await renderer.loadMap(runtimePayload, payloadSha256, true)
await renderer.render({ camera, effects })
```

## Objective

Render canonical Source world, gameplay, and replay state in browser GPU environments.

## Responsibilities

- Own scenes, views, GPU resources, draw preparation, lighting presentation, and frame pacing.
- Consume direct compiler buffers and runtime descriptors for map, model, material, visibility, particle, gameplay, and replay state without GLB translation.
- Derive presentation-only interpolation without changing authoritative state.
- Verify and decode the direct `PSMP` runtime payload into material-batched Source-space Three.js/WebGPU world buffers; an explicit debug scene exposes geometry while reporting every unavailable resolved material instead of silently substituting it.

## Non-Responsibilities

- Parsing Source formats or defining material semantics.
- Advancing gameplay, replay, entities, particles, or audio truth.
- Becoming the canonical owner of world data.

## Relationships

Consumes world and presentation packages plus game-owned presentation mappings; applications own UI and product composition.

## Completion

Complete when the declared Source presentation families are integrated and supported by aligned visual evidence.
