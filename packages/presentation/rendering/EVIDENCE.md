# HDR Rendering Evidence

## Deterministic Suite

`bun test packages/presentation/rendering/tests` executes 15 tests with 86 assertions. The fixed inputs cover:

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
| Payload bytes | 39,814,462 | 75,972,411 |
| Payload SHA-256 | `d0576dff06413848d8712ab6218c8c6f34078a1b347795c5a7a694a108c29725` | `22d668cbeac17167d1826efa7fc45e218640ce739cb48bd37f76fd67d289cb80` |
| Schema/profile | `3` / `ldr` | `4` / `hdr` |
| Linear samples | 3,896,843 RGBExp32 records | 3,896,843 RGB binary32 records |
| Surface classifications | first-style flat path | 809 unlit, 1,473 flat, 0 directional-normal, 1,511 directional-SSBump |
| World lights | not present in schema 3 | 73 |
| Ambient indexes/samples | not present in schema 3 | 1,899 / 9,014 |
| Profile materials/input hashes | not present in schema 3 | 6 / 131 |
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
