# Rendering

## Sample

```ts
import { createRenderer, SOURCE_PC_INTEGER_HDR } from "@playsrc/rendering"

const renderer = await createRenderer({ canvas, configuration: SOURCE_PC_INTEGER_HDR })
await renderer.loadMap({ payload: runtimePayload, payloadSha256, directionalTextures, diagnostic: true })
await renderer.render({ camera, effects, lightStyles: [{ style: 0, scalar: 1 }] })
```

## Objective

Render canonical Source world, gameplay, and replay state in browser GPU environments.

## Responsibilities

- Own scenes, views, GPU resources, draw preparation, lighting presentation, and frame pacing.
- Consume direct compiler buffers and runtime descriptors for map, model, material, visibility, particle, gameplay, and replay state without GLB translation.
- Isolate the post-world viewmodel from world depth with a depth-only clear, then apply its supplied projection and `[0,0.1]` viewport depth range while retaining hand/item self-depth.
- Derive presentation-only interpolation without changing authoritative state.
- Verify and decode the direct `PSMP` runtime payload into material-batched Source-space Three.js/WebGPU world buffers; an explicit debug scene exposes geometry while reporting every unavailable resolved material instead of silently substituting it.
- Decode schema-5 displacement metadata, geometry and vertex alpha; blend WorldVertexTransition base textures by normalized authored alpha; execute supplied two-axis mode-0 detail modulation; and exclude `UnlitGeneric` StudioModels from ambient/local-light requirements.
- Upload decoded top-to-bottom RGBA planes without a browser row flip and execute supplied Material blend, alpha-test/reference, cull, depth, polygon-offset, wireframe/no-draw, and sampler state. A mipmapped texture supplied with mip zero only is `MissingTextureMips`; ordinary loading rejects it and diagnostic loading never promotes generated mips to Source output.
- Validate typed VertexLitGeneric/EyeRefract/Eyes records and upload every authored model mip with exact scalar encoding, color interpretation, wrap/filter state, and anisotropy. Explicit model draw preparation consumes one StudioModel-produced lighting record and primitive-keyed eye rows, evaluates the Source ambient/local-light equations, and rejects every missing framebuffer/environment/proxy/texture input. Diagnostic model output preserves supplied authored base textures and uses identity color only when that texture is missing; base-only or generated-mip output never satisfies a Source draw.
- Preserve schema-3 LDR behavior: pack first-style face samples into one deterministic float atlas, decode RGBExp32 to linear light, and bind the atlas to UV channel 1 with clamped nearest sampling.
- Decode schema-4 linear HDR samples and the complete `PSHD` profile descriptor; verify its member closure, retained-resource hashes, profile-material records, consumed-input order, and cross-record ranges before GPU staging.
- Preserve radiance above one in binary32; compose supplied face styles; generate flat and three directional float atlas planes; and evaluate official-basis normal or direct-coefficient SSBump lighting. Ordinary loading rejects a missing required directional plane; diagnostic loading reports it before drawing debug output.
- Consume world lights and ambient cubes only through explicit candidate, style, leaf, sample, and weight inputs; never infer absent visibility, leaf bounds, model associations, or compiler records.
- Own immutable LDR or Source PC integer-HDR exposure, tone, sRGB output, preferred WebGPU canvas format, and alpha configuration. Lighting/blending remain linear and output transfer occurs exactly once.
- Consume PENV v3 profile-selected sky encoding, sky/cubemap inputs, and authored Water texture chains plus PVIS v3 PVS/frustum and above/below/reflection/refraction/main/intersection plans. Exact modules build Source-axis 2D-sky faces/seams, RGBS decode taps, authored six-face cubemap mips/sampling, cheap/expensive Water and fog probes, and Source-ordered Water views. The configured HDR sky supplies one `$hdrcompressedTexture` RGBS face per semantic side. Water tangent/projected-depth/overlay material inputs remain missing, so the diagnostic adapter never substitutes a default cubemap or approximate Water shader.
- Own scene/device generations, cancellation, atomic replacement, loss recovery, resize suspension, capture, queue-safe GPU retirement, frame pacing, and deterministic idempotent disposal.
- Cull counter-clockwise world back faces while honoring only Material's explicit no-cull feature; StudioModel draws consume encoded-clockwise front faces and Three/WebGPU determinant parity without per-model index reversal or double-sided repair.
- Require one unique exact StudioModel matrix for every map occurrence; reject posed material/template mismatches; update posed positions, normals, and tangent handedness; and draw both composed hand/item viewmodel parts through one separate horizontal-4:3-FOV projection, world far plane, WebGPU viewport depth range `[0,0.1]`, restored world range `[0,1]`, and post-world pass.
- Batch consecutive PSPR v2 camera-facing sprites and trails with identical material and blend state into retained power-of-two dynamic buffers, one indexed draw, and one cached material while preserving current/next sheet rectangles, color/alpha, roll, radius, previous position, trail fade/length bounds, and stable Particle order. Repeated views of the same immutable model/brush/Particle request retain staged models and brush instances while updating camera-facing Particle vertices; a position-identical no-Water main view retains its immutable visibility indices and uses the current display camera. The WebGPU adapter retains immutable model-zero batches in one render bundle, invalidates that bundle only when selected index ranges or attachments change, and uses Three.js's initialized synchronous render operation without changing world, Water, dynamic, or viewmodel pass order.
- Draw projected marks from supplied model-local fragments and authored alpha/mips with categorical decal bias, current receiver transforms, collision/placement revisions, and world/brush visibility; present textures never become diagnostic quads.
- Admit world triangles per face rather than per material batch; expose deterministic back-to-front transparent ordering with immediate framebuffer-copy operations and terminal ignore-depth work.
- Expose bounded nested view-state restoration, fixed frame phases with reverse rollback, pair-only interpolation/discontinuities, queued-opportunity pacing records, atomic replacement resources, and complete dynamic-light/shadow requirement validators.
- Create and compare immutable color/depth/normal/material-ID/primitive-ID/object-ID captures under a strict aligned-manifest contract. Missing native target planes remain Missing.
- Treat the current Three.js WebGPU adapter as diagnostic-only. Ordinary output requires an explicit one-command-encoder backend; diagnostic frames return no queue-submission serial and make no Source parity claim.

## Non-Responsibilities

- Parsing Source formats or defining material semantics.
- Advancing gameplay, replay, entities, particles, or audio truth.
- Becoming the canonical owner of world data.

## Relationships

Consumes world and presentation packages plus game-owned presentation mappings; applications own UI and product composition.

## Completion

Complete when the declared Source presentation families are integrated and supported by aligned visual evidence.
- Consume immutable profile-qualified environment summaries and Rust-produced PVS surface sets; PVS changes draw eligibility only and never collision or gameplay authority.

`src/source-camera.ts`, `src/model-lighting.ts`, `src/source-environment.ts`, and the dynamic-light/shadow records in `src/frame-foundations.ts` include behavior adapted from Valve Source SDK 2013. [`SOURCE-1-SDK-LICENSE.txt`](SOURCE-1-SDK-LICENSE.txt) applies; the repository copy of `thirdpartylegalnotices.txt` is retained under `packages/presentation/particle`.
