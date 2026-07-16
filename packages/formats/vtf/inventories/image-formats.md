# VTF Image-Format Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

Authority identity: Valve Source SDK 2013 `src/public/bitmap/imageformat.h` at commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`, public Alien Swarm `src/public/bitmap/imageformat.h` at commit `bc047d62ea6529c92d37b2544ed971cd73dc7ad7`, Microsoft BC1-BC5 contracts, IEEE 754, and three exact declared-game content indexes. The indexes are Missing.

Generator command: Missing.

Candidate item count: 70 dialect-qualified code mappings. Accepted item count: 0.

Target dispositions: 51 mappings decode to canonical native-precision planes; 19 mappings return deterministic Unsupported because VTF supplies no palette or the checked authorities do not establish a portable source-file sample contract. Unsupported mappings allocate no decoded plane.

`B4×4` means one 4×4 block. Packed integer fields are little-endian. A `U16 component code` is the zero-extended encoded field value: R5/B5 remain in `0..31`, G6 remains in `0..63`, A4 remains in `0..15`, R10/G10/B10 remain in `0..1023`, and A2 remains in `0..3`. Synthetic opaque alpha is `65535`. `Opaque` means no encoded alpha is exposed. Color transfer is not applied during decoding.

| Code scope | Format | Encoded storage | Canonical result | Alpha/color contract | Target disposition |
|---|---|---|---|---|---|
| Both dialects 0 | RGBA8888 | 4 bytes/texel: R, G, B, A | `RGBA/U8` | Encoded 8-bit alpha; dialect-qualified color label | Decode |
| Both dialects 1 | ABGR8888 | 4 bytes/texel: A, B, G, R | `RGBA/U8` | Encoded 8-bit alpha; dialect-qualified color label | Decode |
| Both dialects 2 | RGB888 | 3 bytes/texel: R, G, B | `RGBA/U8` | Opaque; dialect-qualified color label | Decode |
| Both dialects 3 | BGR888 | 3 bytes/texel: B, G, R | `RGBA/U8` | Opaque; dialect-qualified color label | Decode |
| Both dialects 4 | RGB565 | 2 bytes/texel: low R5, G6, high B5 | `RGBA/U16` component codes | Opaque; color samples | Decode |
| Both dialects 5 | I8 | 1 byte/texel | `Intensity/U8` | No alpha; data/color use is consumer-owned | Decode |
| Both dialects 6 | IA88 | 2 bytes/texel: intensity, alpha | `IntensityAlpha/U8` | Encoded 8-bit alpha | Decode |
| Both dialects 7 | P8 | 1 byte palette index; no palette is stored in VTF | No plane | Palette identity and colors are unavailable | Deterministic Unsupported |
| Both dialects 8 | A8 | 1 byte/texel | `Alpha/U8` | Encoded 8-bit alpha; no invented RGB | Decode |
| Both dialects 9 | RGB888_BLUESCREEN | 3 bytes/texel: R, G, B | `RGBA/U8` | `00 00 FF` becomes transparent black; every other texel is opaque | Decode |
| Both dialects 10 | BGR888_BLUESCREEN | 3 bytes/texel: B, G, R | `RGBA/U8` | `FF 00 00` storage becomes transparent black; every other texel is opaque | Decode |
| Both dialects 11 | ARGB8888 | 4 bytes/texel: A, R, G, B | `RGBA/U8` | Encoded 8-bit alpha | Decode |
| Both dialects 12 | BGRA8888 | 4 bytes/texel: B, G, R, A | `RGBA/U8` | Encoded 8-bit alpha; shader-compressed HDR interpretation is not applied | Decode |
| Both dialects 13 | DXT1 / BC1 | 8 bytes/B4×4 | `RGBA/U8` | Endpoint ordering selects four-color or transparent three-color mode | Decode |
| Both dialects 14 | DXT3 / BC2 | 16 bytes/B4×4 | `RGBA/U8` | Explicit 4-bit alpha expanded to U8 | Decode |
| Both dialects 15 | DXT5 / BC3 | 16 bytes/B4×4 | `RGBA/U8` | Interpolated 8-bit alpha | Decode |
| Both dialects 16 | BGRX8888 | 4 bytes/texel: B, G, R, ignored X | `RGBA/U8` | Opaque | Decode |
| Both dialects 17 | BGR565 | 2 bytes/texel: low B5, G6, high R5 | `RGBA/U16` component codes | Opaque; color samples | Decode |
| Both dialects 18 | BGRX5551 | 2 bytes/texel: low B5, G5, R5, ignored X1 | `RGBA/U16` component codes | Opaque | Decode |
| Both dialects 19 | BGRA4444 | 2 bytes/texel: low B4, G4, R4, high A4 | `RGBA/U16` component codes | Encoded 4-bit alpha retained as an integer code | Decode |
| Both dialects 20 | DXT1_ONEBITALPHA | 8 bytes/B4×4 | `RGBA/U8` | BC1 transparent selector is one-bit alpha | Decode |
| Both dialects 21 | BGRA5551 | 2 bytes/texel: low B5, G5, R5, high A1 | `RGBA/U16` component codes | Encoded one-bit alpha | Decode |
| Both dialects 22 | UV88 | 2 bytes/texel: U, V | `RG/U8` | NotColor; no B, A, or normal Z is invented | Decode |
| Both dialects 23 | UVWQ8888 | 4 bytes/texel: U, V, W, Q | `RGBA/U8` | NotColor | Decode |
| Both dialects 24 | RGBA16161616F | 8 bytes/texel: four binary16 values | `RGBA/F16` | Alpha bits preserved; no clamp or transfer | Decode |
| Both dialects 25 | RGBA16161616 | 8 bytes/texel: four `u16` values | `RGBA/U16` | Alpha bits preserved; no 8-bit conversion | Decode |
| Both dialects 26 | UVLX8888 | 4 bytes/texel: U, V, L, X | `RGBA/U8` | NotColor | Decode |
| Both dialects 27 | R32F | 4 bytes/texel: binary32 R | `R/F32` | No alpha; bits preserved | Decode |
| Both dialects 28 | RGB323232F | 12 bytes/texel: binary32 R, G, B | `RGB/F32` | Opaque; bits preserved | Decode |
| Both dialects 29 | RGBA32323232F | 16 bytes/texel: binary32 R, G, B, A | `RGBA/F32` | Alpha bits preserved | Decode |
| `source-2013-pc` 30 | NV_DST16 | 2 bytes/texel | No plane | Vendor depth sample contract is not portable | Deterministic Unsupported |
| `source-2013-pc` 31 | NV_DST24 | 4 bytes/texel | No plane | Vendor depth sample contract is not portable | Deterministic Unsupported |
| `source-2013-pc` 32 | NV_INTZ | 4 bytes/texel | No plane | Vendor depth/stencil sample contract is not portable | Deterministic Unsupported |
| `source-2013-pc` 33 | NV_RAWZ | 4 bytes/texel | No plane | Vendor depth sample contract is not portable | Deterministic Unsupported |
| `source-2013-pc` 34 | ATI_DST16 | 2 bytes/texel | No plane | Vendor depth sample contract is not portable | Deterministic Unsupported |
| `source-2013-pc` 35 | ATI_DST24 | 4 bytes/texel | No plane | Vendor depth sample contract is not portable | Deterministic Unsupported |
| `source-2013-pc` 36 | NV_NULL | Source-file payload contract is Unknown | No plane | Dummy GPU format has no canonical source texel | Deterministic Unsupported |
| `source-2013-pc` 37 | ATI2N / BC5 | 16 bytes/B4×4 | `RG/U8` | NotColor; no normal Z or alpha is invented | Decode |
| `source-2013-pc` 38 | ATI1N / BC4 | 8 bytes/B4×4 | `R/U8` | NotColor | Decode |
| `asw-pc` 30 | RG1616F | 4 bytes/texel: binary16 R, G | `RG/F16` | Bits preserved | Decode |
| `asw-pc` 31 | RG3232F | 8 bytes/texel: binary32 R, G | `RG/F32` | Bits preserved | Decode |
| `asw-pc` 32 | RGBX8888 | 4 bytes/texel: R, G, B, ignored X | `RGBA/U8` | Opaque | Decode |
| `asw-pc` 33 | NULL | Source-file payload contract is Unknown | No plane | Dummy GPU format has no canonical source texel | Deterministic Unsupported |
| `asw-pc` 34 | ATI2N / BC5 | 16 bytes/B4×4 | `RG/U8` | NotColor; no normal Z or alpha is invented | Decode |
| `asw-pc` 35 | ATI1N / BC4 | 8 bytes/B4×4 | `R/U8` | NotColor | Decode |
| `asw-pc` 36 | RGBA1010102 | 4 bytes/texel: R10, G10, B10, A2 | `RGBA/U16` component codes | Encoded 2-bit alpha retained as an integer code | Decode |
| `asw-pc` 37 | BGRA1010102 | 4 bytes/texel: B10, G10, R10, A2 | `RGBA/U16` component codes | Encoded 2-bit alpha retained as an integer code | Decode |
| `asw-pc` 38 | R16F | 2 bytes/texel: binary16 R | `R/F16` | Bits preserved | Decode |
| `asw-pc` 39 | D16 | 2 bytes/texel | No plane | Depth sample use is outside declared source-texture output | Deterministic Unsupported |
| `asw-pc` 40 | D15S1 | 2 bytes/texel | No plane | Depth/stencil sample use is outside declared source-texture output | Deterministic Unsupported |
| `asw-pc` 41 | D32 | 4 bytes/texel | No plane | Depth sample use is outside declared source-texture output | Deterministic Unsupported |
| `asw-pc` 42 | D24S8 | 4 bytes/texel | No plane | Depth/stencil sample use is outside declared source-texture output | Deterministic Unsupported |
| `asw-pc` 43 | LINEAR_D24S8 | 4 bytes/texel | No plane | Depth/stencil sample use is outside declared source-texture output | Deterministic Unsupported |
| `asw-pc` 44 | D24X8 | 4 bytes/texel | No plane | Depth sample use is outside declared source-texture output | Deterministic Unsupported |
| `asw-pc` 45 | D24X4S4 | 4 bytes/texel | No plane | Depth/stencil sample use is outside declared source-texture output | Deterministic Unsupported |
| `asw-pc` 46 | D24FS8 | 4 bytes/texel | No plane | Floating-depth/stencil sample use is outside declared source-texture output | Deterministic Unsupported |
| `asw-pc` 47 | D16_SHADOW | 2 bytes/texel | No plane | Shadow-depth sample use is outside declared source-texture output | Deterministic Unsupported |
| `asw-pc` 48 | D24X8_SHADOW | 4 bytes/texel | No plane | Shadow-depth sample use is outside declared source-texture output | Deterministic Unsupported |
| `asw-pc` 49 | LINEAR_BGRX8888 | 4 bytes/texel: B, G, R, ignored X | `RGBA/U8` | Opaque; Linear | Decode |
| `asw-pc` 50 | LINEAR_RGBA8888 | 4 bytes/texel: R, G, B, A | `RGBA/U8` | Encoded alpha; Linear | Decode |
| `asw-pc` 51 | LINEAR_ABGR8888 | 4 bytes/texel: A, B, G, R | `RGBA/U8` | Encoded alpha; Linear | Decode |
| `asw-pc` 52 | LINEAR_ARGB8888 | 4 bytes/texel: A, R, G, B | `RGBA/U8` | Encoded alpha; Linear | Decode |
| `asw-pc` 53 | LINEAR_BGRA8888 | 4 bytes/texel: B, G, R, A | `RGBA/U8` | Encoded alpha; Linear | Decode |
| `asw-pc` 54 | LINEAR_RGB888 | 3 bytes/texel: R, G, B | `RGBA/U8` | Opaque; Linear | Decode |
| `asw-pc` 55 | LINEAR_BGR888 | 3 bytes/texel: B, G, R | `RGBA/U8` | Opaque; Linear | Decode |
| `asw-pc` 56 | LINEAR_BGRX5551 | 2 bytes/texel: low B5, G5, R5, ignored X1 | `RGBA/U16` component codes | Opaque; Linear | Decode |
| `asw-pc` 57 | LINEAR_I8 | 1 byte/texel | `Intensity/U8` | No alpha; Linear | Decode |
| `asw-pc` 58 | LINEAR_RGBA16161616 | 8 bytes/texel: four `u16` values | `RGBA/U16` | Encoded alpha; Linear | Decode |
| `asw-pc` 59 | LE_BGRX8888 | 4 little-endian bytes/texel: B, G, R, ignored X | `RGBA/U8` | Opaque | Decode |
| `asw-pc` 60 | LE_BGRA8888 | 4 little-endian bytes/texel: B, G, R, A | `RGBA/U8` | Encoded alpha | Decode |

The exact legacy CS:GO binding of the `asw-pc` table is Blocked. A code encountered under that target remains Unknown until the dialect authority is accepted; it is not reinterpreted through `source-2013-pc`.
