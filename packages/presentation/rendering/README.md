# Rendering

## Sample

```ts
import { createRenderer, SOURCE_PC_INTEGER_HDR } from "@playsrc/rendering"

const renderer = await createRenderer({ canvas, configuration: SOURCE_PC_INTEGER_HDR })
await renderer.loadMap({ payload: runtimePayload, payloadSha256, directionalTextures })
await renderer.render({ camera, effects, lightStyles: [{ style: 0, scalar: 1 }] })
```

## Objective

Render canonical Source world, gameplay, and replay state in browser GPU environments.

## Responsibilities

- Own scenes, views, GPU resources, draw preparation, lighting presentation, and frame pacing.
- Consume direct compiler buffers and runtime descriptors for map, model, material, visibility, particle, gameplay, and replay state without GLB translation.
- Derive presentation-only interpolation without changing authoritative state.
- Verify and decode the direct `PSMP` runtime payload into material-batched Source-space Three.js/WebGPU world buffers; an explicit debug scene exposes geometry while reporting every unavailable resolved material instead of silently substituting it.
- Upload decoded top-to-bottom RGBA planes without a browser row flip and execute supplied Material blend, alpha-test/reference, cull, depth, polygon-offset, wireframe/no-draw, and sampler state; unavailable texture inputs remain diagnostics.
- Preserve schema-3 LDR behavior: pack first-style face samples into one deterministic float atlas, decode RGBExp32 to linear light, and bind the atlas to UV channel 1 with clamped nearest sampling.
- Decode schema-4 linear HDR samples and the complete `PSHD` profile descriptor; verify its member closure, retained-resource hashes, profile-material records, consumed-input order, and cross-record ranges before GPU staging.
- Preserve radiance above one in binary32; compose supplied face styles; generate flat and three directional float atlas planes; and evaluate official-basis normal or direct-coefficient SSBump lighting. Ordinary loading rejects a missing required directional plane; diagnostic loading reports it before drawing debug output.
- Consume world lights and ambient cubes only through explicit candidate, style, leaf, sample, and weight inputs; never infer absent visibility, leaf bounds, model associations, or compiler records.
- Own immutable LDR or Source PC integer-HDR exposure, tone, sRGB output, preferred WebGPU canvas format, and alpha configuration. Lighting/blending remain linear and output transfer occurs exactly once.
- Retain sky, water, and environment requirements as typed `Missing` or `Unsupported` inputs until their complete producer contracts are supplied. Ordinary loading rejects every required non-handled input; only caller-selected diagnostic loading may draw a debug background.
- Own scene/device generations, cancellation, atomic replacement, loss recovery, resize suspension, capture, queue-safe GPU retirement, frame pacing, and deterministic idempotent disposal.
- Cull counter-clockwise world back faces while honoring only Material's explicit no-cull feature; fixed-camera canvas evidence rejects missing interior floor, ceiling, and walls.
- Apply exact StudioModel occurrence matrices; update posed model positions, normals, and tangent handedness; and draw viewmodels through a separate horizontal-4:3-FOV projection, world far plane, `[0,0.1]` depth range, and post-world pass.
- Batch PSPR v2 camera-facing sprites and trails by material while preserving current/next sheet rectangles, blend, color/alpha, roll, radius, previous position, trail fade/length bounds, and stable Particle order.
- Draw projected marks from supplied fragments and decoded alpha with Material decal blend/depth/cull state; present textures never become diagnostic quads.

## Non-Responsibilities

- Parsing Source formats or defining material semantics.
- Advancing gameplay, replay, entities, particles, or audio truth.
- Becoming the canonical owner of world data.

## Relationships

Consumes world and presentation packages plus game-owned presentation mappings; applications own UI and product composition.

## Completion

Complete when the declared Source presentation families are integrated and supported by aligned visual evidence.
- Consume immutable profile-qualified environment summaries and Rust-produced PVS surface sets; PVS changes draw eligibility only and never collision or gameplay authority.
