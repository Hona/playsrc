# HDR Rendering Evidence

## Deterministic Suite

`bun test packages/presentation/rendering/tests` executes 41 tests with 204 assertions. The fixed inputs cover:

- Schema-3 LDR payload framing, atlas coordinates, RGBExp32 decode, and malformed/trailing bytes.
- Schema-4 `PSHD` framing, all ten member roles, lighting closure, VTF hash and header metadata, consumed-input order, world lights, ambient indexes/samples, profile requirements, reserved bytes, and cross-record ranges.
- Linear radiance `20`, integer-HDR saturation `65535/4096`, flat and three directional atlas planes, gutters, two styles, missing/duplicate style scalars, all three normal basis vectors, and SSBump coefficients.
- Explicit world-light candidate/style input and ambient leaf/sample/weight input with no inferred leaf, visibility, or interpolation state.
- Immutable LDR/HDR profile combinations, sRGB toe/midtone/saturation values, opaque/premultiplied alpha, 16-bin exposure targets, accelerated adaptation, fixed-step bounds, and dropped-time accounting.
- Resource generation activation, queue-completion retirement, exact-once destruction, repeated disposal, and rejection of post-retirement resources.
- Model ambient-cube axes, point/directional/spot attenuation, half-Lambert, four-light bounds, explicit draw requirements, eye projections, ray/sphere intersection, pupil dilation, and ambient luminance.
- Six 2D-sky face orientations/seams/translation, per-tap RGBS decode, strict cubemap ties, six axes, authored mips, cheap/expensive Water, 25 blur taps, and fog direction/density.
- Per-face visibility, back-to-front/ignore-depth/framebuffer-copy order, complete nested view restoration, phase rollback, interpolation/discontinuities, dynamic-light/shadow records, frame pacing, and atomic replacement.
- Color/depth/normal/material-ID/primitive-ID/object-ID hashes, comparisons, and aligned-manifest rejection.

`bun run build` in `apps/web/tf2` passes Vite 8.1.5 production bundling with the rendering contracts and diagnostic adapter. This proves browser bundling only; it is not aligned target or ordinary Source-output evidence.

The retained `bun run verify:tf2-wasm -- jump_beef` payload record predates PENV v3 and is stale. The current configured input closure contains 321 objects; the prior LDR payload `56153098…156`, HDR payload `0f33e861…84a5`, and 475,511,805 presentation-byte result cannot satisfy current evidence until the bounded verifier completes with the new sky-encoding bytes.

`bun run verify:browser -- jump_beef` passed cold/warm derived caches, exact camera/visibility, independent projected-mark admission, Water/viewmodel restoration, pointer/crouch/gameplay/Particle/audio flows, performance bounds, one-interrupt cleanup, and listener release. The diagnostic run retained 18 support blockers and zero content blockers. Its fixed 1280×720 canvas SHA-256 was `0687a5ee…ac6e6`; the projectile-Particle canvas SHA-256 was `897e78c4…202f`. These hashes are browser regression evidence only.

## Exact `jump_beef` Differential

The checked `bun run verify:tf2-wasm -- jump_beef` workflow generated the canonical artifacts, and Rendering consumed those unchanged bytes.

| Input or result | LDR | HDR |
|---|---:|---:|
| Payload bytes | 42,082,929 | 78,255,264 |
| Payload SHA-256 | `56153098a867c553651f9c773bd72c4659782bae8520277c80daaaa414bdf156` | `a2326c011921f1da90480b1c5f4d3923c038e2dcf07cf6c1d69d43cb2a145a5f` |
| Schema/profile | `3` / `ldr` | `4` / `hdr` |
| Linear samples | 3,896,843 RGBExp32 records | 3,896,843 RGB binary32 records |
| Surface classifications | first-style flat path | 809 unlit, 1,473 flat, 0 directional-normal, 1,511 directional-SSBump |
| World lights | not present in schema 3 | 73 |
| Ambient indexes/samples | not present in schema 3 | 1,899 / 9,014 |
| Profile materials/input hashes | not present in schema 3 | 6 / 303 |
| Atlas | 4,096×462 flat | 4,096×527 flat plus three directional planes with one-texel gutters |

The HDR descriptor, closure, and all six retained VTF hashes passed validation. Raw sample scanning found 2,726,373 components above one and maximum `50.94902420043945`. The float atlas retained values above one and applied the declared PC integer-HDR upload boundary only when building GPU planes; its maximum was `15.999755859375`. The LDR parser retained the prior 4,096×462 atlas dimensions, 14 materials, 2,761 drawable surfaces, 10 batches, nine models, and 33 model occurrences.

## WebGPU Lifecycle And Captures

Headless Chrome 149 on macOS used WebGPU `bgra8unorm`, sRGB output, standard tone mode, opaque alpha, and a 320×180 canvas. The fixed camera was Source position `[5328,3376,-3120]`, yaw `180`, pitch `0`, vertical FOV `75`, near `1`, and far `32768`.

- Initialization published `Ready` at device generation 1 and scene generation 0.
- Resize returned exactly 320×180 without suspension.
- LDR cold load/render/capture, atomic replacement, and warm render/capture produced the same PNG SHA-256: `62c00e4800b49af0eabb79ec4893dcff0529ce2a8398e853529262ac9763bea2`. Replacement advanced scene generation from 1 to 2.
- HDR schema-4 load reported exactly 3,896,843 samples, 1,511 directional faces, 73 world lights, and 9,014 ambient samples. One fixed exposure step retained exposure 1. Its diagnostic PNG SHA-256 was `f49b132eb999732f915ccbda5066cb74f10a482fbe70af88c9bc8d10f08dfea8`.
- A synthetic schema-4 directional face supplied a hash-checked 1×1 SSBump plane and identity UV transform. The WebGPU directional node compiled and rendered without `MissingDirectionalInput`; its 128×128 PNG SHA-256 was `f765d744229ac7abe98288348509b68fa28acc9e84db1500b4e44c8462efb6f6`.
- Repeated disposal was idempotent. Each renderer published `Disposed`, and `GPUCanvasContext.getConfiguration()` returned `null` after unconfiguration.

The HDR capture is diagnostic evidence for descriptor consumption, float GPU allocation, Source exposure/output, capture, and cleanup. It is not visual-parity evidence: the integrated payload does not contain the required SSBump texture plane, water/environment associations, complete model materials, or a supported sky decode/orientation contract. Ordinary scene loading rejects missing world material or directional inputs; diagnostic loading reports them and never classifies its pixels as an ordinary substitute.

## TF2 Visual Consumer Audit

- Unit vectors convert TF2's default 75-degree and viewmodel 54-degree horizontal-4:3 FOVs to `59.84044400898544` and `41.82812169855287` vertical degrees. The fixed browser camera is `[5328,3376,-3067.96875]`, yaw `180`, pitch `-1`, near Z `7`, and binary32 map-extent far Z `28377.919921875`.
- Rendering admits exactly 33 unique supplied occurrence matrices. Browser vectors match the official entity transform for cow `[2336,2328,-3136]/[0,90.5,0]`, two frog transforms, resupply locker `[5512,3440,-2800]/[0,179.5,0]`, and Soldier `[-5632,2896,-1136]/[0,180,0]`. Missing, duplicate, non-finite, or model-mismatched matrices fail; no angle fallback remains.
- All 13 present decal materials supply source-alpha/one-minus-source-alpha blending, back-face culling, depth test with no depth write, nearer-or-equal comparison, and categorical decal offset. PENV v3 carries 73 collision-selected fragments; the fixed spawn admits exactly 13 by world/brush receiver visibility and current transforms.
- The headed viewmodel pass reports descriptor depth `[0,0.10000000149011612]`, applies it through the WebGPU viewport after world rendering, and restores `[0,1]`. Projection-element depth mutation is absent. Team skin zero/one selects the matching template and a posed material mismatch fails.
- PTF2 v10 contains 55 typed model materials, exactly three EyeRefract states, 52 VertexLitGeneric states, 71 unique authored model textures, source-backed BC/direct planes, decoded channel-converted planes, plus complete selected alpha and Water texture chains. Rendering validates topology, dimensions, scalar/color interpretation, wrap/min/mag state, and anisotropy; it never generates model, alpha, or Water mips. Five opaque world materials still expose mip-zero-only diagnostics.
- `bun run verify:browser jump_beef` passed cold/warm caches, camera/matrix/alpha/decal/depth predicates, pointer input, crouch, configured Collision/movers, random/audio, composed hand/item frames, projectile Particle timelines, performance records, one-interrupt cleanup, and listener release. Cold/warm world-region SHA-256 values are ceiling `e90fc77ac5aa945b663f04420c74a5a01c16417954cef92bae37e17776ce30cd`, forward wall `c84156a41ec392134c2e1c39333991786e7f7fd86e9b702b6408ffbac9b23eb4`, and floor `25a0bcecd5d7bcd84cb27dc7c8e008fc8d37776f5f4929df0a1e8ea0f7e6b3b0`. Full-canvas hashes are retained per run but are not compared because authored viewmodel and Particle time advances independently.

These captures are deterministic browser regression evidence, not visual-parity evidence. Current model lightcache selections, game-owned eye targets, five opaque world mip chains, 2D-sky/cheap-cubemap shader execution, native glyph-raster capture, and an aligned controlled target capture remain Missing.

## Environment Rendering Integration Evidence

The current public producer-to-consumer audit covers `packages/world/map/rust/src/environment.rs`, `packages/world/material/rust/src/lib.rs`, `packages/world/visibility/rust/src/lib.rs`, `games/tf2/wasm/src/lib.rs`, `games/tf2/browser/src/{artifacts.ts,client.ts}`, `apps/web/tf2/src/runtime.ts`, and this package.

- PENV v3 carries selected sky encoding plus complete mark receiver/revision/lifetime/pass state, Water surface/bottom bindings, controller values, leaf distances, and decoded selected alpha/Water/sky/cubemap subresources. PMST v2 carries typed alpha ownership and discard.
- PVIS v3 carries unchanged PVS candidates, separate perspective-frustum world draws, origin leaf/leaves/areas/sky state, evaluated Water frame/transform/WaterLOD, above/below/intersection classification, reflection/refraction/main/intersection plans, clips/fog, and one surface set per auxiliary view. Rendering executes bounded Water targets and restores camera, target, clipping, background, entity, and visibility state.
- The prior native/WASM run passed fixed 91-surface spawn PVS, frame-30 above/below Water plans, near-plane intersection, stock/Original launch, held cadence, and configured brush solidity under PENV v2. PENV v3 invalidates its retained payload byte/hash values; no current payload identity is claimed until the bounded verifier completes.
- No checked target capture command, immutable target/browser manifest, target color/depth/ID output set, or accepted comparison tolerance exists. Browser PNG hashes cannot satisfy aligned evidence.

This evidence retains exact blockers for `REN-026`, `REN-028`, complete aligned evidence for `REN-027`–`REN-032`, `REN-044`, `REN-046`, and `REN-054`. It is not a visual-parity claim and does not authorize Rendering-side Source parsing or a partial visual substitute.
