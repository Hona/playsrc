# Demo Roadmap

[`../../../docs/roadmap-contract.md`](../../../docs/roadmap-contract.md) defines the normative roadmap schema and denominator review gate. [`../../../TERMINOLOGY.md`](../../../TERMINOLOGY.md) defines delivery status, coverage classification, inventory, and completion terms.

## Completion Denominator

This leaf roadmap contains exactly 13 behavior rows. The candidate inventory contains 3 target-game protocol profiles and 17 profile-command assignments. Those 20 candidate items contribute 0 denominator items until a checked-in generator emits the inventory and a denominator review accepts it. The current denominator is therefore 13 behavior rows. Acceptance of all 20 current candidate items would make the denominator 33 items.

The denominator covers bounded, lossless parsing of little-endian `HL2DEMO\0` DEM containers for Team Fortress 2, Counter-Strike: Source, and legacy Source 1 Counter-Strike: Global Offensive. It covers the fixed header, sequential outer command records, command ticks and slots, packet metadata, sequence numbers, and encoded command-payload extents. It does not decode a network message, construct recorded state, advance a replay timeline, execute a console or user command, resimulate gameplay, or render presentation state.

The denominator is Not accepted. The prerequisite format universe is Candidate and Not accepted, the inventory has no generator or review record, and the exact legacy CS:GO network-protocol identity is Unknown.

## Active Recorded-TF2 Checkpoint

This checkpoint selects profile `tf2-demo3-net24` and its eight `demo3:*` command assignments. It implements the fixed header, complete sequential command framing, packet metadata, exact payload ranges, lossless source retention, bounded whole-buffer and chunk-scheduled parsing, and one checked controlled-capture acquisition/verification workflow. CS:S, legacy CS:GO, DEM mutation, and network-payload interpretation remain outside this checkpoint. The selected implementation is Ready. The controlled capture's originating server Steam build ID remains Missing; its immutable bytes, provider record, and exact configured verification build are retained separately and never conflated.

## Inputs

- One immutable DEM byte sequence or ordered byte-chunk stream with caller-supplied source identity, byte length, and lowercase SHA-256 content hash.
- One explicit profile identity from [`inventories/protocols-and-commands.md`](inventories/protocols-and-commands.md). The parser never infers a profile from the game-directory field, command bytes, packet payload, file name, or extension.
- One required parse-limit value containing maximum input bytes, maximum command records, maximum encoded bytes per command, maximum total encoded command bytes, maximum retained diagnostic bytes, and maximum streaming-buffer bytes. Every field is a non-negative integer and every allocation is charged before it occurs.
- Valve Source SDK 2013 commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`: `src/public/demofile/demoformat.h`, `src/common/proto_version.h`, `src/public/cdll_int.h`, `src/public/inetmessage.h`, and `src/public/inetmsghandler.h`.
- Valve's public `csgo-demoinfo` commit `049f8dbf49099d3cc544ec5061a7f7252cce7b82`: `demoinfogo/demofile.h`, `demofile.cpp`, and `demofiledump.cpp`.
- AlliedModders public HL2SDK snapshots: CS:S commit `64895cf48c68f5cef0470ee3f13ec449babc30ea` and legacy CS:GO commit `9cf2f325ea273559c7cae27b4f98518c18b8e322`, each at `public/demofile/demoformat.h`, plus the CS:S `common/proto_version.h`.
- The public Valve Developer Community `DEM (file format)` revision `oldid=471439` and `NeKzor/dem` commit `2f48097f94cb14dea95d11e5040758b409a72e9a` for command-payload boundaries not declared by the official headers.

## Outputs

- An immutable DEM header value retaining all 1,072 source bytes, the eight-byte stamp, signed 32-bit demo and network protocols, four raw 260-byte name fields, each field's first-NUL byte prefix and termination state, playback-time float bits, signed playback ticks, signed playback frames, and signed sign-on length.
- An ordered command list. Every command retains its zero-based ordinal, absolute source range, encoded identifier, complete signed tick or exact SourceTV terminal low-three-byte tick encoding, optional slot, profile identity, typed outer metadata, encoded payload range, exact payload bytes, and coverage classification.
- Sign-on and packet records retaining one or two 76-byte command-info blocks according to profile, every command-info flag and float bit pattern, two signed sequence values, declared payload length, and exact packet payload bytes.
- Console-command, user-command, data-table, string-table, and custom-data records retaining their signed scalar fields, declared length, exact encoded bytes, and source ranges without interpreting the payload.
- A parse summary containing command counts by stable inventory identity, observed packet count, sign-on boundary, terminal-stop range, total encoded payload bytes, and every non-Handled classification.
- A structured error naming the absolute byte range, profile, command ordinal, field path, violated invariant, declared value, available value, limit value, and `Malformed`, `Unsupported`, `Unknown`, or `Missing` classification. A failed parse yields no authoritative DEM value.

## Invariants

- Every accepted numeric field is little-endian. Signed integers and IEEE-754 float bit patterns are preserved exactly; parser control flow never depends on playback time, playback ticks, or playback frames.
- The header is exactly 1,072 bytes: 8 stamp bytes, 2 signed 32-bit protocol fields, 4 raw 260-byte name fields, 1 float32 playback-time field, and 3 signed 32-bit count or length fields.
- The stamp must equal the eight bytes `HL2DEMO\0`. The explicit profile's demo protocol, network protocol, and game-directory byte prefix must match the header. Profile selection never falls back to another protocol.
- Name fields remain byte strings. NUL termination and bytes after the first NUL are observable; the Demo package performs no locale, Unicode, path, map, server, or player-name interpretation.
- Sign-on length is non-negative, lies within the command-stream bytes, and ends on a parsed command boundary. It partitions the stream for observation and never causes the parser to skip validation.
- Every non-stop command begins with the profile's complete command header: command byte and signed tick for demo protocol 3; command byte, signed tick, and unsigned slot for demo protocol 4. Demo-protocol-4 slots are 0 or 1. A different slot is Unknown.
- A well-formed TF2 container has exactly one terminal `dem_stop` and no bytes after it. It is either the ordinary five-byte command byte plus signed tick or the observed SourceTV four-byte stream-flush encoding containing command byte 7 plus the exact low three tick bytes and no complete signed tick. The four-byte form is accepted only as the final four source bytes. Every other truncation is Malformed. Command byte 0 is not an implicit stop command.
- `dem_signon` and `dem_packet` contain exactly the profile's command-info bytes, two signed sequence fields, one non-negative signed 32-bit payload length, and that many payload bytes. Command-info unknown flag bits remain exact and receive an Unknown classification.
- Every declared payload length is checked with widened arithmetic before slicing, retaining, or allocating. A negative length, overflow, range beyond supplied input, total-budget excess, or record-count excess is deterministic and never triggers a resynchronization scan.
- An unassigned command byte is Unknown at its exact byte position. Because its body extent is not established, the parser stops and does not guess the next command boundary.
- The three current candidate profiles declare no DEM-container compression. A command byte or envelope claiming an unaccepted compressed representation is Unknown and is never decompressed.
- Whole-buffer and streaming parses of the same bytes, profile, and limits produce identical header values, command values, classifications, and error positions for every chunk partition.
- Every successful output accounts for every source byte exactly once as header, command header, typed outer field, or encoded payload. Demo parsing has no dependency on networking, replay, simulation, presentation, game, application, service, or renderer packages.

## Ownership Exclusions

- Exact logical-path resolution, provider ordering, and source acquisition belong to `packages/content`; uploaded-file selection belongs to the consuming application.
- Packet bitstreams, network-message identifiers, protobuf or bit-buffer framing, data-table definitions, string-table contents, entity deltas, and live transport semantics belong to `packages/runtime/networking`.
- Recorded-state construction, tick advancement, event ordering, playback timing, indexing, seeking, snapshots, interpolation policy, and replay authority belong to `packages/runtime/replay`.
- TF2-, CS:S-, and legacy-CS:GO-specific user-command data, custom-data meaning, entity state, event state, and payload interpretation belong to `games/tf2`, `games/css`, and `games/csgo` respectively.
- Whether a recorded console command affects replay state belongs to `packages/runtime/replay`; game-specific command effects belong to the selected game module. The Demo package exposes command bytes and never executes them.
- Rendering, particles, and audio presentation belong to `packages/presentation/rendering`, `packages/presentation/particle`, and `packages/presentation/audio`.
- Recording, editing, capture, and inspection command orchestration belongs to `tools/*`. No declared target requires playsrc to mutate or emit DEM bytes, so mutation and emission are Excluded from this denominator.
- `.vdm` demo-action documents do not share the DEM container interface. The candidate format universe currently assigns them internally to Demo, while this assignment excludes action-list parsing and replay-editing semantics. Denominator acceptance requires moving `.vdm` parsing to an accepted format owner; `packages/formats/demo-action` is the proposed owner, and action execution remains with `packages/runtime/replay`.

## Behavior Families

| Target behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| A caller-selected target-game profile validates the exact stamp, demo protocol, network protocol, game directory, command-header shape, slot count, and command set without guessing. | No Demo implementation exists; the parent format inventory is Not accepted and the legacy-CS:GO network protocol is Unknown. | One controlled capture from each declared target build: compare selected profile and every identity field with an independent fixed-offset extractor and the named public contracts; require exact error offsets for every single-field mismatch. | Blocked |
| The complete 1,072-byte header preserves protocol fields, all four fixed name arrays, playback-time bits, tick count, frame count, and sign-on length without using advisory metadata as allocation authority. | `playsrc-demo` retains every fixed array, first-NUL state, scalar, bit pattern, and source range. | Header vectors plus the checked controlled capture compare exact raw fields and SHA-256. | Ready |
| Sign-on length identifies an in-range command boundary while every byte in the sign-on prefix is parsed through the ordinary command rules. | Parsing validates `1072 + signonLength` against the complete boundary set after ordinary sequential framing. | Boundary, in-field, beyond-input, and controlled-capture vectors require exact errors and the observed boundary `1071502`. | Ready |
| Sequential command parsing preserves file order, signed ticks, source ranges, duplicate/decreasing ticks, and zero-length payloads until one terminal stop. | Protocol-3 commands retain ordinal, tick domain, ranges, body metadata, payload ranges, and the exact terminal encoding without resynchronization. | All-command mixed vectors, every truncation, arbitrary chunk schedules, and 13,298-record controlled framing. | Ready |
| `dem_signon` and `dem_packet` expose profile-sized command-info blocks, flags, camera float bits, sequence information, and one exact packet-payload boundary without decoding network messages. | Protocol-3 packet values preserve all 76 command-info bytes as flags and 18 float bit patterns, both sequence integers, declared length, and payload extent. | Synthetic bit-pattern vectors and 13,294 controlled signon/packet payloads consumed by the shared networking codec. | Ready |
| `dem_synctick` has no body and `dem_stop` terminates through either the complete protocol header or the exact terminal SourceTV stream-flush encoding. | `playsrc-demo` preserves both terminal forms, accepts the four-byte form only at EOF, and rejects every other truncation or suffix. | Adjacent control-record vectors, every truncation point, and controlled SourceTV capture SHA-256 `beed83610c65008a502b502eb6371d3329b225b1d73c9de622ca42924346ab8d`; require exact stop range and no implicit command-0 stop. | Ready |
| `dem_consolecmd` exposes a non-negative signed length and exact command bytes without text normalization or execution. | Length and bytes remain uninterpreted; negative, over-bound, overflowed, and truncated values fail before slicing. | Empty, binary, negative, truncation, and per-command/total budget vectors. | Ready |
| `dem_usercmd` exposes its signed command sequence and exact length-delimited bytes without decoding game input. | Sequence, declared length, source range, and bytes are retained without a game-input dependency. | Signed sequence and payload-boundary vectors. | Ready |
| `dem_datatables` and `dem_stringtables` preserve their distinct identifiers and exact length-delimited network payloads without parsing tables. | Distinct typed bodies hand exact ranges to Networking; Demo never imports Networking. | Synthetic payload vectors and controlled payload handoff decode 518 tables, 363 classes, and 20 string tables through the downstream crate. | Ready |
| Demo-protocol-4 `dem_customdata` preserves its signed selector, non-negative length, and exact uninterpreted bytes. | No Demo implementation exists and no retained declared-build capture establishes occurrence. | Custom-data vectors covering selector boundaries, empty and bounded payloads, malformed lengths, and adjacent records; compare exact outer fields while requiring zero game-specific interpretation in Demo. | Not started |
| Accepted profiles either decode one explicitly inventoried container-compression envelope or categorically reject compression without fallback. | Checked public contracts declare no container compression for the two established profiles; the final legacy-CS:GO build profile is not retained. | Scan controlled captures from all three declared target builds for every raw command byte and envelope; require the inventory to name each compression identity or record exact Unknown rejection at the command byte. | Blocked |
| Malformed lengths, truncation, unknown commands, unknown flags, and all caller limits produce one deterministic classification and absolute error range under bounded streaming. | Required limits charge input, record count, per-command bytes, total payload bytes, diagnostics, and streaming buffer before growth; whole/chunk parses share one parser. | Every truncation except the established SourceTV terminal form, bound edges, unknown IDs/flags, and chunk sizes 1 through 97 pass deterministic comparisons. | Ready |
| The parsed representation is lossless and every required producer and consumer uses one Demo interface without acquiring network, replay, game, or presentation authority. | Source bytes are retained once behind `Arc`; all fields reference exact ranges; the Networking fixed-capture verifier consumes the current interface as a development dependency. | Header plus ordered command ranges concatenate byte-identically; package dependency audit finds no runtime Networking, Replay, Simulation, game, or presentation dependency. | Ready |

## Generated Inventories

No generated inventory is accepted. Accepted item count: 0.

| Output | Authority identity | Authority revision | Generator command | Candidate items | Accepted items |
|---|---|---|---|---:|---:|
| [`inventories/protocols-and-commands.md`](inventories/protocols-and-commands.md) | Official SDK demo and protocol headers; Valve's public CS:GO demo tool; public CS:S and legacy-CS:GO headers; `NeKzor/dem`; fixed target-build DEM captures | SDK `88fa198fba3fb85d46d4c95018254693fdc3af0a`; CS:GO tool `049f8dbf49099d3cc544ec5061a7f7252cce7b82`; CS:S `64895cf48c68f5cef0470ee3f13ec449babc30ea`; legacy CS:GO `9cf2f325ea273559c7cae27b4f98518c18b8e322`; DEM docs `2f48097f94cb14dea95d11e5040758b409a72e9a`; target-build captures Missing | Missing | 20 | 0 |

The future generator is owned by `tools/playsrc`. It must consume retained authority snapshots and one controlled DEM capture with content-build provenance for each declared game, emit stable profile and profile-command identities in lexical order, retain blocked and absent discoveries, and fail on an unclassified stamp, demo protocol, network protocol, game directory, command byte, command-header shape, slot count, packet-info size, payload envelope, or compression identity. No command name is declared before that implementation exists.

## Exit Criteria

The Demo package is Complete only when all of these predicates pass:

- All 13 behavior rows are Ready.
- The format-universe inventory and Demo inventory have Accepted denominator reviews with current authority revisions, generator command, output path, item count, reviewer identity, review date, and reviewed commit.
- The generated inventory contains exactly 3 accepted target-game profiles and 17 accepted profile-command assignments, including an exact legacy-CS:GO network protocol; no required item is Blocked, Unsupported, Unknown, or Missing.
- One retained controlled capture from the accepted TF2, CS:S, and legacy-CS:GO content build passes header, command-boundary, stop, payload, streaming, and lossless-range comparisons.
- Every malformed length, integer overflow, truncation point, unknown command, unknown slot, unknown flag, compression claim, record-count bound, byte bound, and chunk boundary satisfies its declared evidence predicate.
- Every named producer and consumer uses the current Demo interface; no duplicate DEM outer reader remains; network payload decoding exists only in `packages/runtime/networking`; and no fallback, compatibility layer, legacy path, or stale generated inventory remains.
- Demo has no dependency on networking, replay, simulation, presentation, game, application, service, or renderer packages.

## Blockers

- **Format-universe prerequisite:** [`../ROADMAP.md`](../ROADMAP.md) and [`../inventories/formats.md`](../inventories/formats.md) remain Candidate and Not accepted. Demo cannot accept its child denominator before the parent assigns the current DEM identity through the required review gate.
- **Inventory generator:** no checked-in command emits `inventories/protocols-and-commands.md`. Checked `tools/`, root package manifests, and the current Demo package tree.
- **Controlled target captures:** [`evidence/controlled-tf2-dem.json`](evidence/controlled-tf2-dem.json) retains TF2 bytes, provider metadata, SHA-256, expected framing/state counts, and exact configured verification build `24207079` / `10822003`. The provider record and DEM do not encode the originating server Steam build ID, so that identity remains Missing rather than inferred. CS:S and legacy CS:GO captures remain Missing. The checked command is `bun packages/formats/demo/scripts/verify-controlled-tf2-demo.ts`; filesystem discovery is never used.
- **Legacy CS:GO network protocol:** demo protocol 4, command IDs 1 through 9, two command-info blocks, and the slot byte are established, but the exact network-protocol integer for the selected final Source 1 CS:GO content build is Unknown. Checked Valve `csgo-demoinfo` commit `049f8dbf49099d3cc544ec5061a7f7252cce7b82`, AlliedModders CS:GO commit `9cf2f325ea273559c7cae27b4f98518c18b8e322`, Valve Developer Community DEM revision `471439`, and the absent controlled capture.
- **Compression occurrence:** the checked Source 1 contracts declare no DEM-container compression, but no final legacy-CS:GO capture is retained to prove that the accepted build adds none. Container-compression acceptance remains Blocked rather than inferred from unrelated formats.
- **Demo-action owner conflict:** the parent candidate inventory assigns `.vdm` action lists internally to Demo, but `.vdm` has a separate replay-editing interface and is absent from this assignment's DEM-container denominator. The parent inventory and root Owner Registry must accept a separate format owner before either denominator review can pass.
- **Legacy CS:GO payload-owner conflict:** the parent candidate inventory treats the command-to-protobuf mapping as a Demo ownership question. This roadmap assigns outer DEM framing to Demo and every network-message/protobuf mapping to `packages/runtime/networking`; the parent row must be reassigned before acceptance.
