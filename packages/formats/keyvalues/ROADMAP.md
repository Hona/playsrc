# KeyValues Roadmap

[`../../../docs/roadmap-contract.md`](../../../docs/roadmap-contract.md) defines the normative roadmap schema and denominator review gate. [`../../../TERMINOLOGY.md`](../../../TERMINOLOGY.md) defines delivery status, coverage classification, inventory, and completion terms.

## Completion Denominator

This leaf roadmap contains exactly 16 behavior rows. The candidate dialect inventory contains exactly 4 rows and contributes 0 denominator items until the Source format-universe denominator is Accepted, a checked-in generator emits the dialect inventory, and a denominator review accepts this roadmap. The current completion denominator is therefore the 16 behavior rows. It is Not accepted.

The target is Source 1 KeyValues 1 text, the native binary KeyValues tree, the KVPacker binary tree if its declared playsrc consumer and wire profile are established, and the VGUI resource syntax profile assigned by the parent format inventory. Source 2 KV3 and DMX text encodings are excluded.

## Inputs

- Immutable KeyValues text bytes with a Source logical identity, either no byte-order mark or a UTF-16 little-endian byte-order mark.
- Immutable binary KeyValues bytes plus the selected binary dialect identity and, where the dialect does not encode it, an explicit producer ABI.
- A required limits record containing `maxInputBytes`, `maxDecodedBytes`, `maxTokenBytes`, `maxDepth`, `maxNodes`, `maxDirectives`, `maxDependencies`, `maxIncludeDepth`, `maxAggregateDependencyBytes`, `maxOwnedBytes`, `maxOutputBytes`, and `maxDiagnostics`. The package defines no unbounded mode.
- A text profile selecting literal-backslash or escaped-string tokenization and preserve-or-evaluate conditional handling.
- A condition environment mapping exact ASCII-insensitive condition symbols to booleans. The syntax package never infers a browser, operating system, game, or rendering policy.
- An include/base callback that receives the current Source logical identity and exact directive token, then returns bytes and provenance or a `Missing` result. Provider selection and precedence remain outside this package.
- Valve Source SDK 2013 KeyValues contracts at commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`: `src/public/tier1/KeyValues.h`, `src/tier1/KeyValues.cpp`, `src/public/tier1/kvpacker.h`, `src/tier1/kvpacker.cpp`, `src/tier1/utlbuffer.cpp`, and `src/public/vstdlib/IKeyValuesSystem.h`.
- Named consumer contracts in `src/game/shared/econ/econ_item_system.cpp`, `src/game/shared/econ/econ_item_schema.cpp`, `src/game/client/econ/store/store_panel.cpp`, `src/game/shared/particle_parse.cpp`, `src/game/client/c_soundscape.cpp`, `src/vgui2/vgui_controls/BuildGroup.cpp`, and TF2 VGUI call sites.

## Outputs

- A `KeyValuesSyntaxDocument`: ordered top-level objects and directives; ordered repeated children; object or scalar node kind; exact key and scalar token bytes; quoted/unquoted form; comments and whitespace trivia; conditional annotations; inferred scalar type; source encoding; and half-open source spans.
- A `KeyValuesDocument`: the ordered typed tree after optional conditional evaluation and include/base composition, with one provenance record per contributing source and no presentation or game semantics.
- A `KeyValuesDependencyRequest` for each `#include` or `#base`, containing directive kind, parent logical identity, exact target token, directive span, and include chain.
- A `KeyValuesCompositionTrace` recording dependency order, returned provenance, conditional decisions, base merges, include appends, and every `Missing`, cycle, or limit result.
- Text or binary bytes from an explicitly selected serializer mode.
- Diagnostics containing coverage classification, stable error code, source identity, original-byte offset, one-based line and column for text, half-open span, key stack, and include chain.

## Invariants

- Every child list and top-level root list preserves source order and repeated keys. No map is the canonical representation.
- Key spelling and scalar bytes are preserved. ASCII-insensitive lookup returns the first matching sibling without changing stored spelling or removing later matches.
- A scalar and an object are distinct node kinds. Empty scalar strings, empty objects, missing values, and missing objects are never conflated.
- Text parsing is pure over supplied bytes. It performs no filesystem, provider, network, cache, game-policy, or rendering operation.
- Directive composition requests exact dependencies through the callback. It never searches, retries another provider, or broadens a failed logical identity.
- Conditional parsing always preserves the condition token and placement. Evaluation produces a derived document and never mutates the syntax document.
- No token, directive, include, base, conditional, unknown type tag, repeated key, comment, or malformed suffix is silently discarded.
- Text tokens contain at most 4,095 decoded bytes and text object depth contains at most 101 objects including the top-level object. The KVPacker name/string limit is 1,023 bytes. A lower caller limit takes precedence.
- Every count, offset, length, allocation, recursion step, dependency read, and emitted byte is checked against the required limits record before use.
- Text and binary serializers never infer a dialect. A node that the selected serializer cannot represent returns a typed handled error instead of being omitted or coerced.
- Source 2 KV3 syntax, types, containers, compatibility, and terminology never enter this package.

## Ownership Exclusions

- VMT shader roots, material parameters, proxies, patch-material composition, and material condition expressions belong to `packages/formats/vmt`; material meaning belongs to `packages/world/material`; shader execution belongs to `packages/presentation/rendering`.
- BSP entity-lump text belongs to `packages/formats/bsp`; entity classes, key meanings, outputs, and state transitions belong to `packages/world/entity` and `games/<game>`.
- PHY textual key data belongs to `packages/formats/phy`.
- DMX `keyvalues`, `keyvalues2`, and `keyvalues2_flat` encodings belong to the DMX format owner proposed by the Source format-universe roadmap; they are not KV1 dialects.
- Particle-manifest field meanings belong to `packages/presentation/particle`. Sound-event, soundscape, and sentence document schemas belong to the proposed `packages/formats/audio-script` owner, while playback belongs to `packages/presentation/audio`. Item, weapon, population, and game-configuration fields belong to `games/<game>`. KeyValues owns only their accepted KV1 syntax profile.
- VGUI control construction, layout, resolution selection, conditional override policy, fonts, colors, borders, commands, and interaction belong to `apps/web/<product>` for product UI.
- Condition-symbol values and effects belong to the consuming game, application, or tool. KeyValues owns condition token syntax and evaluation against a supplied environment.
- Logical-path canonicalization, mounted-provider precedence, exact byte retrieval, caching, and provenance acquisition belong to `packages/content`.
- Command orchestration and inventory generation belong to `tools/playsrc`.

## Behavior Families

| Target behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| KV1 text accepts no-BOM Source byte text and UTF-16LE-BOM text, preserves the selected encoding identity, and rejects embedded NUL, UTF-16BE, odd UTF-16 byte counts, and invalid surrogate sequences without prefix parsing. | No parser exists. | Fixed byte vectors compare decoded token bytes and failure boundaries with the SDK UTF-16LE conversion contract; malformed cases require exact classification and original-byte spans. | Not started |
| KV1 lexical analysis handles ASCII space, tab, carriage return, line feed, vertical tab, and form feed; `//` comments; quoted and unquoted tokens; braces; literal-backslash mode; escaped-string mode; and empty quoted values. Escaped mode accepts only `n`, `t`, `v`, `b`, `r`, `f`, `a`, `\\`, `?`, `'`, and `"`; another backslash sequence is Malformed. | No tokenizer exists. | Token-stream vectors cover every delimiter and escape, unknown escapes, comments at token boundaries, braces inside quotes, empty values, and 4,095/4,096-byte boundaries. | Not started |
| KV1 documents contain one or more top-level named objects, nested objects or scalars, empty objects, repeated keys, and ordered siblings; key lookup is ASCII-insensitive and first-match while stored spelling remains exact. | No typed syntax representation exists. | Parsed-tree comparisons use fixed multi-root, repeated-key, mixed-case, empty-object, and empty-string documents; order, spelling, node kind, and first-match lookup must match the declared representation exactly. | Not started |
| Text parsing infers a signed 32-bit integer only from a complete in-range decimal integer, an IEEE-754 binary32 only from a complete finite decimal float, and an unsigned 64-bit integer only from `0x` plus exactly 16 hexadecimal digits; every other scalar, including overflow and invalid/non-finite numeric lexemes, remains a byte string regardless of quoting. Binary/programmatic trees additionally represent wide strings, RGBA colors, and opaque pointer values without conflation. | No scalar representation exists. | Boundary vectors cover signed overflow, exponents, decimal ambiguity, non-finite floats, valid and invalid `0x` forms, quoted numerics, wide strings, colors, and pointer widths; compare type tags and stored values with the selected contract. | Not started |
| A condition token is an unquoted `[` followed by optional `!`, `$`, one or more ASCII letters/digits/underscores, and `]`. It may follow a top-level key before its object, follow a nested key before its value/object, or follow a nested scalar value; its exact token and placement survive parsing. | No conditional syntax representation exists. | Syntax-tree vectors enumerate all accepted placements and reject a standalone condition, a condition between key and missing value, a trailing object condition, malformed brackets/symbols, quoted lookalikes, compound expressions, and overlong tokens at the exact span. | Not started |
| Conditional evaluation applies optional leading negation and ASCII-insensitive symbol lookup against the supplied environment, retains nodes when evaluation is disabled, removes inactive nodes only from the derived document, and evaluates an unmapped symbol as false regardless of negation while recording that decision. | No evaluator exists. | Truth-table vectors cover true, false, negated, unmapped, and disabled evaluation at root, object, and scalar placements; compare derived order plus the immutable syntax document and trace. | Not started |
| Top-level case-insensitive `#include` and `#base` directives produce ordered dependency requests relative to the current logical identity; the parser rejects either directive inside an object or without one non-empty target token. | No directive parser or dependency interface exists. | Request-sequence vectors compare directive kind, parent identity, target bytes, source span, option propagation, and error result for top-level, nested, missing-target, and mixed-case forms. | Not started |
| `#include` parses dependencies with the same text profile, appends all included top-level roots after all local roots in directive order, preserves each included root's order and repeats, and fails with `Missing`, cycle, depth, count, or byte-budget diagnostics rather than returning a silent partial composition. | No include composition exists. | An in-memory exact-identity resolver drives include graphs covering multiple directives, multi-root dependencies, repeated inclusion, missing inputs, cycles, and every dependency limit; compare tree and trace order. | Not started |
| `#base` requires the local document and each base dependency to contain exactly one top-level object. For each base child in source order, it finds the first ASCII-insensitive local match: matching objects merge recursively, another matching node remains local, and an unmatched child is copied and appended. Bases run in directive order, root names are not compared, and include append semantics never apply to a base merge. | No base composition exists. | Fixed local/base trees cover nested conflicts, repeated names, case variants, scalar/object conflicts, different root names, multiple bases, and zero/multi-root inputs; compare the complete composed tree and merge trace. | Not started |
| Native binary KV1 reads and writes one-byte tags for object, byte string, little-endian signed 32-bit integer, little-endian binary32 float, little-endian 32-bit pointer payload, RGBA color, and little-endian unsigned 64-bit integer; NUL-terminated names/strings and tag `8` terminate peer lists; wide-string payloads are a handled non-serializable case. | No native binary reader or writer exists. | Byte-exact vectors generated through SDK `KeyValues::{Read,Write}AsBinary` cover every representable tag, peers, nested objects, empty strings/objects, invalid tags, truncation at each byte, depth 101/102, and trailing bytes. | Not started |
| KVPacker binary KV1 reads and writes its branch-neutral tag tree, including length-prefixed UTF-16 code units, only after one declared playsrc consumer and an exact pointer-width/endianness profile are accepted. | The SDK header and implementation define the API, but no accepted playsrc consumer or self-describing ABI profile establishes this dialect's required wire contract. | Compare byte vectors from the accepted producer ABI for every tag and malformed boundary, then exercise the named consumer through the current interface. | Blocked |
| Text serialization accepts a composed `KeyValuesDocument` and explicitly selects source order or stable ASCII-insensitive key order, empty-string preservation or omission, literal or escaped strings, and no-BOM Source byte text or UTF-16LE output. SDK-compatible numeric mode emits decimal integers, six-fraction-digit floats, and `0x` plus 16 uppercase hexadecimal digits; canonical numeric mode emits the shortest decimal that round-trips each binary32 value. Both modes quote keys/values, convert wide strings to UTF-8, and return handled errors for color/pointer nodes. | No text serializer exists. | Compare SDK-compatible modes against `SaveToFile`/`RecursiveSaveToFile`; separately compare canonical bytes for stable ordering, escapes, empty values, integer/float/uint64 formatting, wide-string conversion, and handled non-text-serializable types. | Not started |
| Unmodified syntax serialization is byte-identical; canonical text parse-write-parse preserves ordered tree structure, repeated keys, key/scalar bytes, and scalar bits; binary read-write is byte-identical for every accepted canonical encoding. | No round-trip contract exists. | Property vectors over bounded generated trees plus retained fixed edge vectors compare exact bytes for lossless and binary modes and exact typed-tree equality for canonical text mode. | Not started |
| Every malformed or unsupported input reports one stable code and exact source location without panic, silent truncation, partial success, or loss of prior diagnostics; dependency failures include the complete include chain. | No diagnostic model exists. | Mutation vectors truncate or corrupt every lexical delimiter, brace, directive, scalar, type tag, terminator, string, and dependency edge; assert classification, code, byte offset, line/column, span, key stack, and include chain. | Not started |
| Parsing, composition, and serialization enforce every caller limit before allocation or emission and remain deterministic at the limit and one unit above it. | No bounded implementation exists. | Boundary vectors run each operation at `limit-1`, `limit`, and `limit+1`; retained measurements assert node/dependency/output counts, peak owned bytes within the configured allocation budget, and no further resolver call after failure. | Not started |
| Every accepted dialect has one inventory identity, one named consumer, one syntax profile, and one explicit owner for semantics beyond syntax; every producer and consumer uses the sole KeyValues implementation. | The parent format inventory is Not accepted, the candidate dialect inventory has 4 rows and 0 accepted items, and no package implementation or integration exists. | Regenerate the inventory from the accepted format-universe identity and named SDK registries; audit all declared consumers for one dialect row, one semantic owner, and no embedded KV1 parser, fallback, or legacy reader. | Blocked |

## Generated Inventories

No generated dialect inventory is accepted. Accepted item count: 0.

The manually derived candidate at [`inventories/dialects.md`](inventories/dialects.md) records:

| Field | Current value |
|---|---|
| Authority identity | Source SDK commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; candidate Source format inventory; required TF2, CS:S, and legacy CS:GO content-build indexes are Missing. |
| Authority revision | SDK commit fixed; parent format inventory Not accepted; content-build revisions Missing. |
| Generator command | Missing. The future command must be checked in under `tools/playsrc` before inventory acceptance. |
| Output path | `packages/formats/keyvalues/inventories/dialects.md` |
| Item count | 4 candidate items; 0 accepted items. |

The future generator must read the accepted `packages/formats/inventories/formats.md` identity, the fixed SDK loader/serializer registries, and exact declared-game content indexes. It must sort by dialect identity and fail on an unassigned parser profile, encoding, binary producer, consumer, or semantic owner.

## Exit Criteria

KeyValues is Complete only when all of these predicates pass:

- All 16 behavior rows are Ready.
- The parent Source format-universe denominator is Accepted and assigns every required KV1 identity to this package.
- The dialect inventory is generated, current, Accepted, and contains no Blocked row.
- The KVPacker row either has an accepted consumer and wire profile and is Ready, or the parent format inventory excludes it and removes it from this denominator.
- Text, composition, native binary, accepted KVPacker, serialization, malformed-input, and limit evidence satisfies the roadmap contract.
- Every accepted producer and consumer uses the current KeyValues interface; no embedded KV1 tokenizer, parser, composer, serializer, fallback, compatibility layer, or legacy reader remains.
- Every encountered syntax value and record is classified `Handled`, `Intentionally inert`, `Unsupported`, `Malformed`, `Unknown`, or `Missing`, and no required item remains `Unsupported`, `Unknown`, `Missing`, `Partial`, or `Blocked`.

## Blockers

- **Parent denominator:** `packages/formats/ROADMAP.md` and `packages/formats/inventories/formats.md` state that the 62-row format candidate has 0 accepted items, no generator, five unresolved decisions, and 18 proposed leaf owners absent from the root Owner Registry. The KeyValues dialect set cannot be accepted before that prerequisite is accepted.
- **Dialect generator and content authority:** no checked-in command emits `packages/formats/keyvalues/inventories/dialects.md`, `playsrc.local.json` is Missing, and no exact TF2, CS:S, or legacy Source 1 CS:GO content-build index is available. Checked the current `tools/`, root manifests, format inventory, and KeyValues package tree.
- **KVPacker profile:** `src/public/tier1/kvpacker.h` describes branch-neutral interchange, while `src/tier1/kvpacker.cpp` writes pointer payloads through the producer ABI and the checked SDK tree contains no non-implementation call site. No accepted playsrc consumer, pointer-width profile, endianness profile, or declared-content identity resolves that contract.
