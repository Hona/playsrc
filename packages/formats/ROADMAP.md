# Source Format Universe Roadmap

[`../../docs/roadmap-contract.md`](../../docs/roadmap-contract.md) defines the normative roadmap schema and denominator review gate. [`../../TERMINOLOGY.md`](../../TERMINOLOGY.md) defines delivery status, coverage classification, inventory, and completion terms.

## Completion Denominator

This aggregation roadmap contains exactly 4 behavior rows. The candidate format inventory contains exactly 62 decisions, but contributes 0 denominator items until a checked-in generator emits it and a denominator review accepts it. The current denominator is Not accepted.

The inventory covers Source 1 formats required by Team Fortress 2, Counter-Strike: Source, legacy Source 1 Counter-Strike: Global Offensive, Source map compilation, gameplay, replay, rendering, particles, audio, and declared tools. Source 2 and undeclared platforms, games, products, and workflows are excluded.

## Inputs

- Valve Source SDK 2013 public headers, game loaders, utility sources, and utility build manifests at commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`.
- Public format contracts named in [`inventories/formats.md`](inventories/formats.md), including Valve Developer Community BSP, DEM, DMX, MDL, NAV, PCF, VCD, VMT, VPK, VTF, VMF, and model-source pages. A public page with no immutable revision records its revision as `Unknown`.
- Exact archive indexes for one declared content build of TF2, CS:S, and legacy Source 1 CS:GO. These inputs are Missing because `playsrc.local.json` is absent and no checked content-build indexes exist.
- The current root Owner Registry and every accepted format-leaf roadmap.

## Outputs

- `packages/formats/inventories/formats.md`, containing one stable row for every discovered format identity and exactly one ownership outcome per row.
- One accepted owner path for every non-excluded row.
- One prerequisite inventory identity consumed by every format-leaf denominator review.
- Blocked records for every identity or owner that cannot be established from the named authorities.

The 18 proposed leaf paths below are ownership decisions, not existing packages.

| Proposed leaf | Caller-facing mental model |
|---|---|
| `packages/formats/encrypted-script` | VICE/ICE encrypted Source script envelopes and exact decrypted bytes. |
| `packages/formats/vmf` | Hammer VMF map-source documents, chunks, map objects, and instance references. |
| `packages/formats/map-compile` | PRT portal interchange, LIN leak paths, and RAD lighting-definition inputs. |
| `packages/formats/nav` | Navigation mesh headers, places, areas, ladders, connections, encounters, hiding spots, and game custom data. |
| `packages/formats/ain` | AI node-graph versions, map identity, nodes, links, hull connectivity, and lookup tables. |
| `packages/formats/dmx` | DMX headers, encodings, element graphs, typed attributes, references, and format identity. |
| `packages/formats/pcf` | Source 1 particle-system definitions expressed as DMX elements and operator attributes. |
| `packages/formats/vcd` | Choreography scene documents and compiled scene-image aggregates. |
| `packages/formats/audio-script` | Sound-event scripts, soundscapes, manifests, and sentence definitions. |
| `packages/formats/fgd` | Entity class declarations, inheritance, properties, inputs, outputs, helpers, and editor metadata. |
| `packages/formats/console-script` | Ordered CFG/RC console command streams, quoting, comments, and command boundaries. |
| `packages/formats/squirrel` | Source 1 Squirrel source syntax and parse output; game-owned bindings and execution remain outside this package. |
| `packages/formats/response-rules` | Criteria, rules, response groups, includes, and response alternatives in response-rule scripts. |
| `packages/formats/localization` | Valve localization token resources and compiled caption dictionaries. |
| `packages/formats/sound` | RIFF/WAV sound data, Valve metadata chunks, and MP3 sound assets. |
| `packages/formats/font` | Source bitmap-font records and declared TrueType/OpenType font inputs. |
| `packages/formats/model-source` | QC/QCI model build documents, SMD/VTA records, and model-schema DMX inputs. |
| `packages/formats/ui-script` | VGUI HUD animation sequences, commands, interpolation, timing, and conditions. |

## Invariants

- Every inventory row has one stable format identity and exactly one outcome: existing leaf owner, proposed leaf owner, internal format owned by an existing package, excluded with a target boundary, or blocked with a missing authority.
- An extension never establishes ownership by itself. Formats share a leaf only when callers use one representation, vocabulary, validation boundary, and deep interface.
- KeyValues owns KV1 token, tree, scalar, ordering, duplicate-key, directive, conditional, and binary encodings. A typed document family with a separate caller-facing contract owns its schema after KeyValues parsing.
- DMX owns container encodings and typed element graphs. PCF and model-source owners own their respective DMX schemas.
- BSP owns all 64 standard lump slots, the game-lump directory, accepted game-lump payloads, embedded PAK ZIP data, and BSP lump compression. World packages own map, collision, entity, and visibility semantics decoded from those records.
- Demo owns recorded DEM framing and records. Networking owns live transport and protocol semantics; replay owns recorded-state advancement.
- An internal format has no independent leaf roadmap. Its named existing package owns its parser behavior rows.
- A proposed leaf has no implementation, interface, or delivery status until its path enters the root Owner Registry and its roadmap is accepted.
- PoC behavior never changes playsrc delivery status or inventory acceptance.
- Source 2 KV3, VPK v3, compiled resource containers, behavior, terminology, and compatibility never enter this denominator.

## Ownership Exclusions

- Parser behavior rows belong to the existing or accepted proposed leaf named by the inventory; this aggregation roadmap owns only enumeration, assignment, and acceptance gates.
- Logical-path resolution and provider ordering belong to `packages/content`.
- Canonical map, material, entity, collision, and visibility semantics belong to `packages/world/*`.
- Live network state and wire semantics outside recorded DEM framing belong to `packages/runtime/networking`; replay timeline state belongs to `packages/runtime/replay`.
- Particle simulation and rendering belong to `packages/presentation/particle`; sound selection, mixing, spatialization, and playback belong to `packages/presentation/audio`.
- Rendering interpretation, shader behavior, and GPU work belong to `packages/presentation/rendering`.
- Game-specific KeyValues schemas, population behavior, item behavior, weapon behavior, Squirrel bindings, and script effects belong to `games/<game>`.
- Compiler and command orchestration belong to `tools/*`; immutable compiled outputs belong to `packages/asset-store`.
- Browser-native image, font, audio, and video presentation after format decoding belongs to the consuming web application or presentation package.

## Behavior Families

| Target behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| The declared Source 1 targets have one finite format universe with no silently omitted identity. | The candidate inventory has 62 rows; five authority decisions remain blocked and no declared-game content indexes have been checked. | Regenerate from the named SDK revision, public contracts, and three exact content-build indexes; compare stable identities and require every discovered identity to appear once. | Blocked |
| Every non-excluded format has exactly one package owner at the correct mental-model boundary. | Thirteen rows name existing leaves, nine name internal formats in existing packages, and 28 name 18 proposed leaves; the root Owner Registry contains only the nine current format leaves. | Audit the accepted inventory against the root Owner Registry and every format-leaf Ownership Exclusions section; require one owner and no duplicate parser contract per row. | Blocked |
| Every format-leaf denominator is derived from the same accepted inventory identity. | All nine current format-leaf roadmaps exist as Drafts and reference the same candidate inventory; none is accepted, and the 18 proposed leaf owners have no roadmap. | Repository audit requiring every non-excluded inventory row to map to exactly one leaf behavior denominator and every leaf review to record the same authority revisions and inventory hash. | Blocked |
| The inventory is reproducible, bounded, current, and fails rather than omitting malformed, unknown, missing, or unsupported discoveries. | No checked-in generator command exists. | Run the checked-in generator twice from clean work directories with fixed authority inputs; require byte-identical output, identical item counts, and explicit retained classifications for every non-handled discovery. | Not started |

## Generated Inventories

No generated inventory is accepted. Accepted item count: 0.

The manually derived candidate at [`inventories/formats.md`](inventories/formats.md) records:

| Field | Current value |
|---|---|
| Authority identity | Source SDK commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; named public format contracts; required TF2, CS:S, and legacy CS:GO archive indexes are Missing. |
| Authority revision | SDK commit fixed; public pages without immutable revisions are `Unknown`; content-build revisions are Missing. |
| Generator command | Missing. The future command must be checked in under the owning tool before inventory acceptance. |
| Output path | `packages/formats/inventories/formats.md` |
| Item count | 62 candidate items; 0 accepted items. |

## Exit Criteria

The Source format universe is Complete only when all of these predicates pass:

- All 4 behavior rows are Ready.
- Exact TF2, CS:S, and legacy Source 1 CS:GO content-build archive indexes have been included as generator inputs.
- The generator emits exactly the current inventory, including every excluded and blocked discovery, and a denominator review records `Accepted` with the required review metadata.
- All 62 current candidate decisions have resolved owners or exact exclusions; no row remains Blocked.
- The root Owner Registry contains every accepted leaf owner and no rejected proposed owner.
- Every non-excluded row maps to exactly one accepted leaf roadmap, or to one existing package roadmap when the row is internal.
- Every required producer and consumer names the current owner and no duplicate parser, fallback, compatibility layer, or legacy reader remains.

## Blockers

- **Inventory generator:** no checked-in command regenerates `packages/formats/inventories/formats.md`. Checked `tools/`, root package manifests, and the current formats tree.
- **Owner Registry conflict:** the inventory proposes 18 new format leaves, while the root Owner Registry lists only KeyValues, BSP, VPK, VTF, VMT, Studio model, VHV, PHY, and Demo. Accepting every proposal changes the format-leaf count from 9 to 27. The root registry and aggregate counts must change in the same accepted ownership checkpoint.
- **Legacy CS:GO model companions:** no checked authority establishes whether VVC or another vertex companion is required by the declared legacy content build. Checked Source SDK `src/public/studio.h`, `src/public/optimize.h`, the public MDL/VVD/VTX documentation, and the absent configured content indexes.
- **Legacy CS:GO recorded protocol:** no checked public authority provides the complete DEM command-to-protobuf/network-record mapping for the declared legacy content build. Checked Source SDK `src/public/demofile/demoformat.h`, public DEM documentation, official protobuf declarations, and the absent configured content indexes.
- **Legacy CS:GO VFONT:** the exact VFONT envelope, payload, and declared-content use are not established. Checked Source SDK `src/public/BitmapFontFile.h`, VGUI font interfaces, public font-tool documentation, and the absent configured content indexes.
- **Legacy CS:GO Panorama resources:** the exact Source 1 Panorama source/compiled resource identities consumed by the declared product are not established. Checked Source SDK Panorama client code, public Source SDK directory documentation, and the absent configured content indexes.
- **Game video assets:** the exact Bink or WebM inputs required by the three declared browser products are not established. Checked Source SDK `src/public/video/ivideoservices.h`, public game-file documentation, and the absent configured content indexes.
