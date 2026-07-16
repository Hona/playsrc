# BSP Container Profile Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

Authority identity: Valve Source SDK 2013 `src/public/bspfile.h` at commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; AlliedModders `hl2sdk` CS:GO `public/bspfile.h` at commit `9cf2f325ea273559c7cae27b4f98518c18b8e322`; Valve Developer Community `BSP (Source)` revision Unknown; exact declared-build map indexes Missing.

Generator command: Missing. Candidate item count: 3. Accepted item count: 0.

## Fixed Header

All three profiles use this little-endian 1,036-byte header.

| Byte range | Representation | Contract |
|---|---|---|
| `0..4` | 4 bytes | Exactly `56 42 53 50`, the ASCII bytes `VBSP`. |
| `4..8` | signed 32-bit integer | Container version selected by the profile table. |
| `8..1032` | 64 records × 16 bytes | Slot-ordered directory; each record contains signed 32-bit offset, signed 32-bit encoded length, signed 32-bit lump version, and four profile-dependent raw bytes. |
| `1032..1036` | signed 32-bit integer | Map revision. Every bit pattern is retained; the parser does not convert it into an interface version. |

## Profiles

| Stable identity | Declared games | Container version | Fourth directory field | Compression contract | Authority result |
|---|---|---:|---|---|---|
| `source-2013-v19` | TF2 and CS:S maps accepted by the Source-2013 loader contract | 19 | Raw little-endian signed 32-bit declared decoded size; zero means uncompressed | A nonzero value requires the 17-byte Source LZMA envelope and exact decoded-size agreement | Candidate; declared-build occurrence Missing |
| `source-2013-v20` | TF2 and CS:S | 20 | Raw little-endian signed 32-bit declared decoded size; zero means uncompressed | A nonzero value requires the 17-byte Source LZMA envelope and exact decoded-size agreement | Candidate; declared-build occurrence Missing |
| `legacy-csgo-v21` | Legacy Source 1 CS:GO | 21 | Four raw bytes historically named `fourCC`; no compression meaning is inferred | Payload bytes remain uncompressed unless a separately established profile contract says otherwise | Candidate; declared-build occurrence Missing |

Profile identity is mandatory input. Container version alone cannot select lump 15/54 world-light records, lump 19 brush-side fields, slots 22–25 and 49, or game-lump static-prop layouts.

Versions below 19, above 21, tuple-encoded versions, `PSBV`, Xbox `.360.bsp`, and Source 2 map containers are outside this inventory.
