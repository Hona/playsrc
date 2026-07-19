# Demo Protocol And Command Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

Authority identity: Valve Source SDK 2013 commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; Valve `csgo-demoinfo` commit `049f8dbf49099d3cc544ec5061a7f7252cce7b82`; AlliedModders CS:S commit `64895cf48c68f5cef0470ee3f13ec449babc30ea`; AlliedModders legacy-CS:GO commit `9cf2f325ea273559c7cae27b4f98518c18b8e322`; Valve Developer Community DEM revision `oldid=471439`; `NeKzor/dem` commit `2f48097f94cb14dea95d11e5040758b409a72e9a`; and one controlled DEM capture from an exact accepted content build of each declared game. The three captures are Missing.

Generator command: Missing.

Candidate item count: 20. Accepted item count: 0.

Candidate composition: 3 target-game profiles and 17 profile-command assignments. Nineteen items have a candidate assignment; the legacy-CS:GO profile is Blocked by its Unknown network-protocol integer.

Active checkpoint selection: `tf2-demo3-net24` and all eight `demo3:*` assignments. The implementation is Ready and does not accept the broader three-game denominator; originating recording-server build identity remains Missing.

## Target-Game Profiles

Every profile uses a 1,072-byte little-endian header beginning with the exact eight bytes `HL2DEMO\0`. `Packet info bytes` excludes the following two signed 32-bit sequence fields and signed 32-bit payload length.

| Stable identity | Target game | Demo protocol | Network protocol | Game-directory prefix | Command header | Slots | Packet info bytes | Command set | Container compression | Result |
|---|---|---:|---:|---|---|---:|---:|---|---|---|
| `tf2-demo3-net24` | Team Fortress 2 | 3 | 24 | `tf` | command `u8`; tick `i32` | 1 implicit slot | 76 | `demo3` | None | Candidate assignment from the official SDK; controlled target-build capture Missing |
| `css-demo3-net24` | Counter-Strike: Source | 3 | 24 | `cstrike` | command `u8`; tick `i32` | 1 implicit slot | 76 | `demo3` | None | Candidate assignment from the public CS:S SDK snapshot; controlled target-build capture Missing |
| `legacy-csgo-demo4-net-unknown` | Legacy Source 1 Counter-Strike: Global Offensive | 4 | Unknown | `csgo` | command `u8`; tick `i32`; slot `u8` | 2, encoded as 0 or 1 | 152 | `demo4` | None in checked contracts; target-build occurrence Missing | Blocked: exact network-protocol integer and controlled target-build capture are Missing |

Demo protocol 2 and Source-2013 network protocols 12, 14, and 17 through 23 are compatibility identities, not accepted target profiles. They enter this inventory only if an exact accepted target-build capture requires them and the same change adds their complete command and payload contract. Source 2 demo signatures, commands, protobuf containers, and compression flags are excluded.

## Profile-Command Assignments

The complete profile command header precedes every type-specific body below except the terminal protocol-3 SourceTV stream-flush form. That form is exactly command byte 7 plus three retained low tick bytes at EOF and exposes no complete `i32` tick. Every `i32` is little-endian and signed. Every accepted length is non-negative and must fit the remaining input and caller limits. `bytes[length]` remains encoded data for the owner named by the Demo roadmap.

| Stable identity | Command set | Encoded ID | Command | Type-specific body after profile command header | Payload owner after Demo framing | Result |
|---|---|---:|---|---|---|---|
| `demo3:01-signon` | `demo3` | 1 | `dem_signon` | 76 command-info bytes; incoming sequence `i32`; outgoing acknowledged sequence `i32`; length `i32`; packet bytes | `packages/runtime/networking` | Candidate assignment |
| `demo3:02-packet` | `demo3` | 2 | `dem_packet` | 76 command-info bytes; incoming sequence `i32`; outgoing acknowledged sequence `i32`; length `i32`; packet bytes | `packages/runtime/networking` | Candidate assignment |
| `demo3:03-synctick` | `demo3` | 3 | `dem_synctick` | No body | `packages/runtime/replay` owns clock effects | Candidate assignment |
| `demo3:04-consolecmd` | `demo3` | 4 | `dem_consolecmd` | Length `i32`; exact command bytes | `packages/runtime/replay` and the selected game own effects | Candidate assignment |
| `demo3:05-usercmd` | `demo3` | 5 | `dem_usercmd` | Command sequence `i32`; length `i32`; encoded user-command bytes | Selected game module | Candidate assignment |
| `demo3:06-datatables` | `demo3` | 6 | `dem_datatables` | Length `i32`; encoded data-table bytes | `packages/runtime/networking` | Candidate assignment |
| `demo3:07-stop` | `demo3` | 7 | `dem_stop` | Ordinary complete command header, or exact four-byte SourceTV stream-flush terminal encoding; must end input | `packages/runtime/replay` owns playback termination | Implemented; originating capture-server build identity Missing |
| `demo3:08-stringtables` | `demo3` | 8 | `dem_stringtables` | Length `i32`; encoded string-table bytes | `packages/runtime/networking` | Candidate assignment |
| `demo4:01-signon` | `demo4` | 1 | `dem_signon` | 152 command-info bytes; incoming sequence `i32`; outgoing acknowledged sequence `i32`; length `i32`; packet bytes | `packages/runtime/networking` | Candidate assignment |
| `demo4:02-packet` | `demo4` | 2 | `dem_packet` | 152 command-info bytes; incoming sequence `i32`; outgoing acknowledged sequence `i32`; length `i32`; packet bytes | `packages/runtime/networking` | Candidate assignment |
| `demo4:03-synctick` | `demo4` | 3 | `dem_synctick` | No body | `packages/runtime/replay` owns clock effects | Candidate assignment |
| `demo4:04-consolecmd` | `demo4` | 4 | `dem_consolecmd` | Length `i32`; exact command bytes | `packages/runtime/replay` and `games/csgo` own effects | Candidate assignment |
| `demo4:05-usercmd` | `demo4` | 5 | `dem_usercmd` | Command sequence `i32`; length `i32`; encoded user-command bytes | `games/csgo` | Candidate assignment |
| `demo4:06-datatables` | `demo4` | 6 | `dem_datatables` | Length `i32`; encoded data-table bytes | `packages/runtime/networking` | Candidate assignment |
| `demo4:07-stop` | `demo4` | 7 | `dem_stop` | No body; must end the input | `packages/runtime/replay` owns playback termination | Candidate assignment |
| `demo4:08-customdata` | `demo4` | 8 | `dem_customdata` | Selector `i32`; length `i32`; exact custom bytes | `games/csgo` for game-specific meaning | Candidate assignment; target-build occurrence Missing |
| `demo4:09-stringtables` | `demo4` | 9 | `dem_stringtables` | Length `i32`; encoded string-table bytes | `packages/runtime/networking` | Candidate assignment |

## Command-Info Record

Each 76-byte command-info block has this fixed layout. Demo protocol 3 stores one block. Demo protocol 4 stores two consecutive blocks, one per split-screen slot.

| Field | Encoding | Bytes |
|---|---|---:|
| Flags | signed little-endian integer; accepted bits are use-origin-2, use-angles-2, and no-interpolation | 4 |
| View origin | three float32 bit patterns | 12 |
| View angles | three float32 bit patterns | 12 |
| Local view angles | three float32 bit patterns | 12 |
| Resampled view origin | three float32 bit patterns | 12 |
| Resampled view angles | three float32 bit patterns | 12 |
| Resampled local view angles | three float32 bit patterns | 12 |
| **Total** |  | **76** |

Unknown flag bits remain exact and receive an Unknown classification. Camera selection, interpolation, and presentation use belong to replay and presentation owners.

## Generation Contract

The future checked-in generator must emit this file from the named immutable headers, public contract snapshots, and three controlled target-build captures. It sorts target profiles and command identities lexically, records the exact network protocol and observed command bytes for each capture, and fails on an unclassified signature, protocol pair, game directory, command byte, slot count, packet-info size, body layout, stop suffix, or compression identity. It retains Missing, Unknown, Unsupported, and Malformed discoveries instead of omitting them.
