# HDR Rendering Evidence

## Deterministic Suite

`bun test packages/presentation/rendering/tests` executes 17 tests with 92 assertions. The fixed inputs cover:

- Schema-3 LDR payload framing, atlas coordinates, RGBExp32 decode, and malformed/trailing bytes.
- Schema-4 `PSHD` framing, all ten member roles, lighting closure, VTF hash and header metadata, consumed-input order, world lights, ambient indexes/samples, profile requirements, reserved bytes, and cross-record ranges.
- Linear radiance `20`, integer-HDR saturation `65535/4096`, flat and three directional atlas planes, gutters, two styles, missing/duplicate style scalars, all three normal basis vectors, and SSBump coefficients.
- Explicit world-light candidate/style input and ambient leaf/sample/weight input with no inferred leaf, visibility, or interpolation state.
- Immutable LDR/HDR profile combinations, sRGB toe/midtone/saturation values, opaque/premultiplied alpha, 16-bin exposure targets, accelerated adaptation, fixed-step bounds, and dropped-time accounting.
- Resource generation activation, queue-completion retirement, exact-once destruction, repeated disposal, and rejection of post-retirement resources.

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
- All 13 present decal materials supply source-alpha/one-minus-source-alpha blending, back-face culling, depth test with no depth write, nearer-or-equal comparison, and decal polygon offset. The fixed spawn visibility result excludes all 63 projected fragments by receiver face; Rendering no longer draws those marks merely because another face in the same world-material batch is visible.
- The headed viewmodel pass reports descriptor depth `[0,0.10000000149011612]`, applies it through the WebGPU viewport after world rendering, and restores `[0,1]`. Projection-element depth mutation is absent. Team skin zero/one selects the matching template and a posed material mismatch fails.
- PTF2 v6 contains 55 typed model materials, exactly three EyeRefract states, 52 VertexLitGeneric states, 71 unique authored model textures, and 2,761 decoded authored subresources. Rendering validates exact topology, dimensions, scalar/color interpretation, wrap/min/mag state, and anisotropy before uploading the selected frame/face/slice chain; it never generates model mips. The remaining 25 application-level mip blockers are world/environment records outside this model checkpoint.
- `bun run verify:browser jump_beef` passed cold/warm caches, camera/matrix/decal/depth predicates, pointer input, crouch, configured Collision/mover results, random/audio selection, composed hand/item viewmodels, Particle, one-interrupt cleanup, and listener release. This macOS host rejected all six declared Windows font range faces and suppressed VGUI paint/input without retaining a fallback raster. Cold/warm world-only region SHA-256 values are ceiling `9c500815da0c2202595daa755c993a82e5b80f8b51b74a3529dd83bb4a55eb57`, forward wall `53d18590266fe3202101ad298dcf76d51827b2a4cc76da0f5d76c9dd9ed6364b`, floor `25a0bcecd5d7bcd84cb27dc7c8e008fc8d37776f5f4929df0a1e8ea0f7e6b3b0`, and right wall `05c0283da13773cd31477b4b266ae680b37e18f54a7ba875a07a07ac93086b7f`.

These captures are deterministic browser regression evidence, not visual-parity evidence. Stock hand/`c_model` composition and authored model texture chains are present. Current model lightcache selections, game-owned eye targets, world/environment mips, decoded sky/cubemap resources, Water/fog state and auxiliary views, complete visibility candidates, native Windows glyph-raster capture, and an aligned controlled target capture remain Missing.

## Environment Rendering Stop Evidence

The current public producer-to-consumer audit covers `packages/world/map/rust/src/environment.rs`, `packages/world/material/rust/src/lib.rs`, `packages/world/visibility/rust/src/lib.rs`, `games/tf2/wasm/src/lib.rs`, `games/tf2/browser/src/{artifacts.ts,client.ts}`, `apps/web/tf2/src/runtime.ts`, and this package.

- Map owns complete semantic sky, cubemap, Water, mark, and controller outputs. The current `PENV` transport reduces them to sky material hashes, cubemap hashes/dimensions, water bounds/sample facts, projected fragment geometry, and controller variant identities. It omits decoded sky/cubemap planes and mips, complete Water state/resources/planes, WaterLOD values, current fog transitions, auxiliary views, and complete mark identity/lifetime/pass state.
- Visibility owns origin leaves/clusters, outside mode, merged PVS, areas, sky classification, front-to-back leaves, world surfaces, and generic candidates. The current `PVIS` transport emits only world/cache hashes and world-surface IDs. Rendering consequently cannot gate sky or water views and currently admits all triangles in a material batch when any member face is present.
- Fresh `bun run verify:tf2-wasm -- jump_beef` passed native/WASM identity with 303 inputs, LDR 42,082,929 bytes at `56153098a867c553651f9c773bd72c4659782bae8520277c80daaaa414bdf156`, HDR 78,255,264 bytes at `a2326c011921f1da90480b1c5f4d3923c038e2dcf07cf6c1d69d43cb2a145a5f`, six HDR profile materials, a 444,566,074-byte PTF2 v6 descriptor, and exactly 91 fixed-view surface IDs.
- No checked target capture command, immutable target/browser manifest, target color/depth/ID output set, or accepted comparison tolerance exists. Browser PNG hashes cannot satisfy aligned evidence.

This evidence blocks `REN-026`–`REN-031`, complete `REN-032`, `REN-044`, `REN-046`, and `REN-054`. It is not a visual-parity claim and does not authorize Rendering-side VTF parsing or a partial visual substitute.
