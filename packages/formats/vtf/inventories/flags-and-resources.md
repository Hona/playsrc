# VTF Flags And Resources Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

Authority identity: Valve Source SDK 2013 `src/public/vtf/vtf.h` at commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`, public Alien Swarm `src/public/vtf/vtf.h` at commit `bc047d62ea6529c92d37b2544ed971cd73dc7ad7`, the public VTF resource contract at revision `Unknown`, and three exact declared-game content indexes. The indexes are Missing.

Generator command: Missing.

Candidate item count: 41: 32 texture-flag bit positions, 8 resource-tag families, and 1 recognized resource-entry flag. Accepted item count: 0.

The parser emits `rawFlags` unchanged and separately identifies dialect meaning and version filtering. Versions 7.0 through 7.3 ignore bits `0x00000400`, `0x00080000`, `0x00100000`, `0x00200000`, `0x00400000`, `0x01000000`, `0x10000000`, `0x40000000`, and `0x80000000`; the raw bits remain observable.

## Texture Flags

| Bit | `source-2013-pc` meaning | `asw-pc` meaning | 7.0-7.3 rule | VTF output contract |
|---:|---|---|---|---|
| `0x00000001` | Point sampling | Point sampling | Retained | Preserve classified bit; sampler effect belongs to rendering |
| `0x00000002` | Trilinear sampling | Trilinear sampling | Retained | Preserve classified bit; sampler effect belongs to rendering |
| `0x00000004` | Clamp S | Clamp S | Retained | Preserve classified bit; sampler effect belongs to rendering |
| `0x00000008` | Clamp T | Clamp T | Retained | Preserve classified bit; sampler effect belongs to rendering |
| `0x00000010` | Anisotropic sampling | Anisotropic sampling | Retained | Preserve classified bit; sampler effect belongs to rendering |
| `0x00000020` | Hint DXT5 | Hint DXT5 | Retained | Preserve classified bit; no format substitution |
| `0x00000040` | sRGB | PWL corrected | Retained | Emit dialect-qualified color label; never apply transfer during decode |
| `0x00000080` | Normal map | Normal map | Retained | Preserve classified bit; no normal reconstruction |
| `0x00000100` | No mip sampling | No mip sampling | Retained | Preserve classified bit; declared mip bytes remain selectable |
| `0x00000200` | No LOD | No LOD | Retained | Preserve classified bit; LOD effect belongs to rendering |
| `0x00000400` | All mips | All mips | Ignored | Preserve raw bit and mark version-ignored |
| `0x00000800` | Procedural | Procedural | Retained | Preserve classified bit; no procedural generation |
| `0x00001000` | One-bit alpha present | One-bit alpha present | Retained | Preserve alpha hint separately from encoded format samples |
| `0x00002000` | Eight-bit alpha present | Eight-bit alpha present | Retained | Preserve alpha hint separately from encoded format samples |
| `0x00004000` | Environment map | Environment map | Retained | Select cubemap topology and validate square/depth invariants |
| `0x00008000` | Render target | Render target | Retained | Preserve classified bit; GPU target behavior is excluded |
| `0x00010000` | Depth render target | Depth render target | Retained | Preserve classified bit; GPU target behavior is excluded |
| `0x00020000` | No debug override | No debug override | Retained | Preserve classified bit |
| `0x00040000` | Single copy | Single copy | Retained | Preserve classified bit; memory policy is excluded |
| `0x00080000` | Staging memory | Pre-sRGB | Ignored | Preserve raw bit; emit dialect meaning and version-ignored state |
| `0x00100000` | Immediate cleanup | Unused | Ignored | Preserve raw bit; emit dialect meaning and version-ignored state |
| `0x00200000` | Ignore picmip | Unused | Ignored | Preserve raw bit; emit dialect meaning and version-ignored state |
| `0x00400000` | Unused | Unused | Ignored | Preserve raw bit and version-ignored state |
| `0x00800000` | No depth buffer | No depth buffer | Retained | Preserve classified bit; GPU depth behavior is excluded |
| `0x01000000` | Unused | Unused | Ignored | Preserve raw bit and version-ignored state |
| `0x02000000` | Clamp U | Clamp U | Retained | Preserve classified bit; sampler effect belongs to rendering |
| `0x04000000` | Vertex texture | Vertex texture | Retained | Preserve classified bit; GPU binding is excluded |
| `0x08000000` | SSBump | SSBump | Retained | Preserve classified bit; material interpretation is excluded |
| `0x10000000` | Unused | Unused | Ignored | Preserve raw bit and version-ignored state |
| `0x20000000` | Border clamp | Border clamp | Retained | Preserve classified bit; sampler effect belongs to rendering |
| `0x40000000` | Streamable coarse mips | Unused | Ignored | Preserve raw bit; streaming effect is excluded |
| `0x80000000` | Streamable fine mips | Unused | Ignored | Preserve raw bit; streaming effect is excluded |

## Resource Tags

Tags are three exact bytes. Version 7.3+ directories are strictly ascending by the little-endian 24-bit tag value. Low/high image resources omit the four-byte external payload-length prefix; every other external resource includes it.

| Tag bytes | Identity | Allowed storage | VTF-owned output | Adjacent owner |
|---|---|---|---|---|
| `01 00 00` | Low-resolution image | External only; exact image bytes | Optional `LowResolution` descriptor and decoded plane | Material/rendering decide whether to consume it |
| `10 00 00` | Particle sheet | External only; length-prefixed | Typed version 0/1 sheet sequences: sequence ID, clamp, total duration, frame duration, and one/four UV rectangles | `packages/presentation/particle` owns sequence use |
| `30 00 00` | High-resolution image | External only; exact concatenated image bytes | Ordered high-resolution subresource descriptors | Material/rendering own use |
| `43 52 43` (`CRC`) | Source-input CRC | Inline `u32` or external four-byte payload | Exact `u32`; it is not asserted to checksum VTF bytes | Tools decide whether to compare source inputs |
| `4c 4f 44` (`LOD`) | Texture LOD settings | Inline `u32` or external four-byte payload | Four ordered clamp bytes | Rendering owns LOD selection |
| `54 53 30` (`TS0`) | Extended texture settings | Inline `u32` or external four-byte payload | Four exact flag/reserved bytes | Dialect consumer owns future meanings |
| `53 54 52` (`STR`) | Texture stream settings | Inline `u32` or external four-byte payload | First available mip, last available mip, and two reserved bytes | Content/rendering own streaming policy |
| Any other 3-byte value | Custom resource | Inline `u32` or bounded external bytes | Exact tag, flags, and value/bytes; no inferred schema | The accepting consumer must declare semantics before use |

## Resource Entry Flags

| Flag | Meaning | Validation |
|---:|---|---|
| `0x02` | The entry's four-byte value is inline data and no external chunk exists | Valid for non-image resources; invalid for low/high image resources; every other flag bit is Malformed |

Known fixed-size resources are Malformed when their decoded payload is not exactly four bytes. A `SHEET` payload is Malformed on unsupported version, duplicate/out-of-range sequence ID, excessive declared count, non-finite duration, rectangle truncation, or bytes left outside its declared payload.
