# VTF Version Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

Authority identity: Valve Source SDK 2013 commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`, public Alien Swarm SDK header snapshot commit `bc047d62ea6529c92d37b2544ed971cd73dc7ad7`, the public VTF contract at revision `Unknown`, and exact TF2, CS:S, and legacy Source 1 CS:GO content-build indexes. The three indexes are Missing.

Generator command: Missing.

Candidate item count: 6. Accepted item count: 0.

All rows use signature `VTF\0`, little-endian fields, and `.vtf` logical identities. `resourceCount` is `N` in `0..=32`. Header size includes resource entries.

| Version | Dialect candidate | Exact header contract | Added or changed storage | Cubemap candidate | Declared-target authority |
|---|---|---|---|---|---|
| 7.0 | `source-2013-pc`; `asw-pc` reader compatibility | 64 bytes; depth is absent and equals 1; no resource directory | Low-resolution bytes then high-resolution bytes | Six faces; sphere maps were not introduced | Public VTF version history; exact declared-content presence is Missing |
| 7.1 | `source-2013-pc`; `asw-pc` reader compatibility | 64 bytes; depth is absent and equals 1; no resource directory | Same sequential layout as 7.0 | Public contracts introduce an optional seventh sphere face; sentinel behavior is Blocked | Official SDK face contract conflicts with public sentinel documentation |
| 7.2 | `source-2013-pc`; `asw-pc` reader compatibility | 80 bytes; bytes 63..64 add `u16 depth`; no resource directory | Volume textures become representable; sequential low/high layout remains | Pre-7.5 sphere rule remains Blocked | Official SDK header and public VTF contract; exact declared-content presence is Missing |
| 7.3 | `source-2013-pc`; `asw-pc` reader compatibility | `80 + 8N` bytes; bytes 65..67 padding, bytes 68..71 resource count, bytes 72..79 alignment padding | Sorted resource directory locates low/high images and metadata chunks; version mask ignores nine later flag bits | Pre-7.5 sphere rule remains Blocked | Official SDK header and public VTF contract; exact declared-content presence is Missing |
| 7.4 | `source-2013-pc`; candidate `asw-pc` backward input | `80 + 8N` bytes | Binary container layout equals 7.3; flag meanings differ between dialects | `source-2013-pc` exposes the fallback sphere contract; exact sentinel behavior is Blocked | Official Source SDK 2013 current writer version; content indexes Missing |
| 7.5 | `asw-pc`; candidate legacy CS:GO dialect | `80 + 8N` bytes | Container layout equals 7.4; image-format codes 30+ and flag meanings use the `asw-pc` table | Six faces; sphere face is obsolete | Public Alien Swarm header and public VTF contract; authoritative legacy CS:GO binding is Missing |

Excluded identities:

- `VTFX` and `0x0360.8` console textures are excluded because Xbox 360 is not a declared platform.
- VTF 7.6 compression/resources and BC6H/BC7 extensions are excluded because no declared Valve game target uses that third-party extension.
- Source 2 compiled texture resources are excluded categorically.
