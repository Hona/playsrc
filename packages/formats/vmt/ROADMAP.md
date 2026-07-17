# VMT Roadmap

[`../../../docs/roadmap-contract.md`](../../../docs/roadmap-contract.md) defines the normative roadmap schema and denominator review gate. [`../../../TERMINOLOGY.md`](../../../TERMINOLOGY.md) defines delivery status, coverage classification, logical path, provenance, and Complete.

## Completion Denominator

The current completion denominator contains exactly the 18 Behavior Families rows below. [`inventories/document-constructs.md`](inventories/document-constructs.md) contains 20 candidate document constructs and 0 accepted items. Candidate inventory items do not enter the denominator until the Source format-universe denominator is Accepted, a checked-in generator emits the inventory from exact declared-game content indexes, and a denominator review accepts this roadmap.

The current denominator is Not accepted. It covers Source 1 VMT shader-root documents and case-insensitive `Patch` documents over the accepted KeyValues text dialect. Source 2 material resources and KV3 are excluded.

## Inputs

- One immutable VMT byte sequence, its canonical Source logical identity, and provenance. The identity is supplied by the caller; VMT never adds or removes `materials/`, `.vmt`, or another path component.
- The accepted `kv1-text-default` KeyValues parser and composition interface in literal-backslash mode. Bracket-condition evaluation is either disabled or receives an explicit caller environment.
- A dependency callback receiving the parent logical identity, exact decoded `Patch` `include` token, canonical identity validated by Content, dependency chain, and remaining budgets. It returns exact bytes plus provenance or `Missing`.
- A required limits record containing `maxDocumentBytes`, `maxAggregateDependencyBytes`, `maxKeyValuesDepth`, `maxNodes`, `maxPatchDepth`, `maxDependencies`, `maxCompositionSteps`, `maxOwnedBytes`, and `maxDiagnostics`. `maxPatchDepth` cannot exceed 10. The package defines no unbounded mode.
- Valve Source SDK 2013 contracts at commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`: `src/public/tier1/KeyValues.h`, `src/tier1/KeyValues.cpp`, `src/public/materialsystem/{imaterialsystem.h,imaterial.h,imaterialvar.h,IShader.h,imaterialproxy.h,imaterialproxyfactory.h}`, and `src/utils/vbsp/materialpatch.{h,cpp}`.
- Official TF2 document consumers and producers in `game/mod_tf/materials/logo/new_tf2_logo.vmt`, `src/game/client/tf/vgui/modelimagepanel.cpp`, and `src/game/client/tf/workshop/item_import.cpp`.

## Outputs

- A `VmtSyntaxDocument` containing the immutable KeyValues syntax document, exactly one top-level object, its exact root token, ordinary-or-`Patch` structural role, ordered children, repeated keys, node kinds, source spelling, scalar lexemes, condition annotations, trivia, source spans, and source provenance.
- Structural views over the syntax document for shader-root entries, flag-shaped entries, material-condition-prefixed entries, nested blocks, proxy declarations, and `Patch` members. A view references original nodes and never replaces the canonical ordered tree.
- A `VmtDependencyRequest` for each active `Patch` include, containing parent identity, exact target token, include span, canonical Content request identity, and complete dependency chain.
- A `VmtEffectiveDocument` containing one non-`Patch` root after KeyValues composition and VMT patch composition, with source order, repeated destination keys, exact surviving spelling and scalar lexemes, and one origin record for every effective node.
- A `VmtCompositionTrace` recording KeyValues composition, every patch dependency, shadowed or inert patch member, patch accumulation, insert or replace operation, node-kind decision, returned provenance, and every `Missing`, cycle, or limit result.
- Stable diagnostics containing coverage classification, error code, source identity, original-byte span, key stack, dependency chain, and no partial effective document.

## Invariants

- VMT parsing and composition are pure over supplied bytes, options, limits, and callback results. They perform no provider selection, filesystem access, archive scan, network request, cache lookup, shader lookup, or GPU operation.
- A VMT has exactly one top-level object. Its root token and all descendant token bytes are non-empty where required by KeyValues and retain their source spelling.
- An ordinary root token is an opaque shader identifier. Parameter names, flag-shaped names, proxy identifiers, proxy arguments, scalar value shapes, and nested block names remain syntax until `packages/world/material` classifies them.
- Every child list preserves order, repeated keys, empty objects, and case variants. ASCII-insensitive lookup returns the first matching child and never deletes, merges, or rewrites later matches unless a declared patch operation targets that first match.
- KeyValues bracket conditions, `#include`, and `#base` use the KeyValues interface and trace. VMT never reimplements their tokenization, condition evaluation, or merge behavior.
- Material-condition-prefixed keys and shader-selection override blocks are preserved without evaluation. Their condition vocabulary, selection environment, and effects belong to `packages/world/material`.
- The first case-insensitive `Proxies` object is the active structural proxy section. Every proxy child, repeated proxy name, empty declaration, scalar argument, nested argument block, order position, and later shadowed `Proxies` section remains present in the syntax document.
- A patch chain contains at most 10 include edges and at most 11 VMT documents. A lower caller limit takes precedence. Cycles are detected by canonical logical identity before the callback is invoked again.
- A patch include requests exactly the logical identity produced by Content validation of the decoded token. VMT never supplies a prefix, extension, relative-directory guess, alternate spelling, or second provider request.
- Patch documents accumulate from the requested document toward the first non-`Patch` document. Deeper patch documents override earlier values within the accumulated `insert` section and within the accumulated `replace` section. The complete accumulated `insert` section applies first; the complete accumulated `replace` section applies second.
- `insert` updates the first ASCII-insensitive matching destination node or appends a missing node. It recurses through object nodes, converts a matching scalar to an object when the inserted source is an object, and converts a matching object to a scalar when the inserted source is a scalar.
- `replace` changes only the first ASCII-insensitive matching destination node at each depth. Missing nodes remain missing. A source object descends only into a matching destination object and leaves a matching destination scalar unchanged; a source scalar replaces a matching destination object.
- Patch source repeats apply in source order to the first destination match. Destination repeats after the first match remain ordered and unchanged.
- Composition never mutates a source syntax document. Every effective node identifies the source document, source span, and patch operation that produced it.
- No malformed suffix, unknown ordinary member, unknown patch member, repeated reserved member, missing dependency, inactive condition, or limit failure is silently discarded. A target-defined no-effect member is retained and traced as `Intentionally inert`.
- Every byte count, node count, depth, dependency, composition step, allocation, callback, and diagnostic is checked against the limits record before use.
- Source 2 material containers, KV3 syntax, compiled material resources, compatibility behavior, and terminology never enter this package.

## Ownership Exclusions

- `packages/formats/keyvalues` owns byte decoding, tokenization, comments, quoting, scalar inference, ordered trees, bracket conditions, `#include`, `#base`, and KeyValues serialization.
- `packages/world/material` owns every shader identifier, parameter, flag, material-condition expression, shader-selection override block, proxy identifier, proxy argument, material reference, texture reference, animation rule, proxy execution rule, default, and material-state effect after VMT structure and patch composition.
- `packages/content` owns logical-path validation, provider eligibility and precedence, exact byte retrieval, active-map PAK precedence, caching, and provenance acquisition. VMT owns only the exact patch dependency request and graph composition.
- `packages/formats/vtf` owns VTF headers, resources, image formats, mip levels, frames, faces, slices, and decoded pixels.
- `packages/presentation/rendering` owns shader execution, GPU materials, draw state, texture binding, lighting presentation, and pixels.
- `games/tf2`, `games/css`, and `games/csgo` own game-specific effect selection and mappings from game state or events to material behavior.
- `packages/formats/bsp` owns BSP embedded PAK structure; `packages/formats/vpk` owns VPK structure. VMT never parses either container or chooses between them.
- `tools/playsrc` owns command orchestration and inventory generation.

## Behavior Families

### `jump_beef` world-surface parity checkpoint

| Source/TF2 behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| Every world, decal, water, glass, and sky VMT encountered by the declared `jump_beef` build composes through the sole VMT parser; `Patch` includes retain exact identity and nested empty `replace` objects apply target no-effect semantics rather than deleting inherited proxy children. | The exact target closure retains every ordinary/patch source identity. An empty nested replacement descends without deleting inherited children. | [`../../world/map/inventories/jump-beef-surfaces.md`](../../world/map/inventories/jump-beef-surfaces.md) plus the fixed two-proxy empty-replacement vector. | Ready |
| Proxy declarations remain ordered/repeated structured VMT data, including empty proxy objects and scalar arguments, for Material's typed source-order program. | The complete first active `Proxies` object retains declaration/argument spelling and source order; Material classifies the six target water identities without VMT re-parsing. | Exact water source hashes plus ordered, repeated, empty, malformed, and unsupported proxy vectors. | Ready |

The active migration checkpoint implements the literal-backslash KeyValues VMT structure, ordinary and case-insensitive `Patch` roots, ordered/repeated syntax, first proxy and reserved-member views, batched patch dependency requests/responses, bounded cycle detection, insert/replace composition, immutable source documents, and effective-node origin traces required by the configured `jump_beef` dependency graph. Shader and parameter meaning remains with Material.

| Target behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| Parse VMT bytes through the accepted literal-backslash KeyValues profile, complete KeyValues composition under explicit options, and require exactly one top-level object with no trailing root. | VMT uses the Rust KeyValues literal-backslash parser, requires one object root, and retains exact syntax bytes. Generic `#include`/`#base` composition is explicitly unavailable rather than reimplemented. | Ordinary, patch, directive, root-kind/count, condition, and parser-limit vectors pass; UTF-16 and complete root mutations remain at KeyValues evidence only. | Partial |
| Represent every non-`Patch` root as an exact opaque shader identifier, preserving quote form, spelling, case, source span, and condition annotation without shader lookup. | Every ordinary root remains its KeyValues token and receives no shader classification. | Synthetic mixed-case syntax and the 14 `jump_beef` world-material documents retain exact shader tokens, including `LightmappedGeneric` and `Water`. | Ready |
| Preserve scalar parameter and flag-shaped entries as exact key/value syntax without treating integer, float, boolean, vector, color, matrix, transform, texture, or material meanings as VMT parser success. | Parameter nodes expose unchanged KeyValues tokens, scalar lexemes/kinds, spans, conditions, order, and repeats. | Numeric, `$` key, repeat, condition, and arbitrary-scalar vectors pass; semantic interpretation remains absent. | Partial |
| Preserve arbitrary nested and empty blocks at every accepted KeyValues depth, including parameter blocks and shader-selection override blocks, without flattening or dropping scalar/object distinctions. | Object/scalar identity and empty/nested child lists remain unchanged in syntax and effective output. | Proxy and patch matrices cover empty, nested, and scalar/object conversion boundaries. | Ready |
| Preserve source order, repeated keys, case variants, and first ASCII-insensitive lookup independently at every tree depth. | Syntax preserves all nodes; structural and patch lookups select only the first ASCII-insensitive match. | Repeated ordinary, proxy, reserved patch, and destination-key vectors pass. | Ready |
| Carry KeyValues bracket conditions and generic KeyValues composition provenance without changing condition placement or inferring a platform environment. | Syntax preserves conditions. Composition requires an explicit environment, removes only explicitly inactive nodes, and fails on an unmapped symbol. Generic directive composition remains unavailable. | Mapped-active and unmapped condition vectors pass; generic composition provenance remains absent. | Partial |
| Recognize the syntax shape of material-condition-prefixed keys and shader-selection override blocks while preserving the complete token and assigning selection to Material. | No material-condition syntax view exists. | Fixed documents compare prefix, optional negation, parameter suffix, nested override block, spelling, source order, and no-evaluation trace; Material integration supplies the only selection result. | Not started |
| Expose the first case-insensitive `Proxies` object as ordered proxy declarations whose complete scalar, object, repeated, and empty argument subtrees remain intact; later matching sections remain shadowed syntax. | `active_proxies` exposes only the first object while every later section and repeated declaration remains in syntax. | Repeated/empty proxy vectors pass. Complete TF2 proxy-shape evidence remains absent. | Partial |
| Recognize a case-insensitive `Patch` root, require one non-empty scalar `include` and object-valued first `insert` and `replace` members when present, and retain later duplicates and unknown members with explicit no-effect trace entries. | The first valid reserved members drive composition; duplicates and unknown members emit `InertMember` trace entries. | Valid, missing/wrong-kind, mixed-case, duplicate, and unknown-member vectors pass. | Ready |
| Emit one exact patch dependency request from the decoded include token without adding, removing, relativizing, or retrying a logical-path component. | Composition emits one bounded request batch containing the exact token, parent, span, and chain; caller responses supply the canonical Content identity. | A literal-backslash target remains byte-exact through two dependency rounds; four configured map patches resolve through Content with one request each. | Ready |
| Resolve patch graphs deterministically, fail on `Missing`, malformed dependencies, canonical-identity cycles, depth 11, dependency count, or byte budget, and return no partial effective document. | Repeated coarse calls consume accumulated response batches, detect canonical cycles before parsing, and return only `Needs` or a complete output. | Missing, direct cycle, invalid limit, and two-edge success vectors pass. Indirect cycles and complete limit-minus/equal/plus evidence remain absent. | Partial |
| Accumulate patch chains from outer document to base so deeper documents override earlier values within the same section, then apply the accumulated insert section before the accumulated replace section. | Outer-to-inner accumulation and insert-before-replace order are fixed in one composer. | A three-document conflict vector proves deeper insert/replace values win and both sections apply in order. | Ready |
| Apply `insert` recursively with first-match ASCII-insensitive updates, append-on-missing behavior, source-repeat order, and declared scalar/object conversion. | Recursive insert implements every listed operation and preserves destination repeats after the first. | Nested update/append, scalar-to-object, repeat, case, and ordering vectors pass. | Ready |
| Apply `replace` recursively only through existing first matches, preserve every missing key as absent, leave scalar destinations unchanged for source-object collisions, and replace object destinations for source scalars. | Recursive replace implements every listed collision and missing-key disposition. | Dedicated object/scalar/missing and nested replacement vectors pass. | Ready |
| Preserve every source document immutably and map every effective node and inert member to source identity, span, dependency edge, and composition operation. | Every source retains exact syntax bytes; effective nodes carry source identity/span/operation and trace steps retain dependency and inert-member origins. | Exact root bytes and effective origin fields pass. Full trace replay evidence remains absent. | Partial |
| Report malformed VMT structure and unknown document-level constructs with stable classifications and exact spans while preserving ordinary unknown shader, parameter, flag, proxy, and nested names for Material. | No VMT diagnostic model exists. | Mutation vectors corrupt every root, reserved Patch member, node kind, dependency edge, and boundary; compare classification, code, source span, key stack, dependency chain, retained unknown syntax, and zero panic or silent prefix success. | Not started |
| Enforce all document, aggregate-byte, depth, node, dependency, composition-step, owned-byte, and diagnostic limits before allocation or callback and remain deterministic at the limit and one unit above it. | All declared limit categories exist; patch depth/dependency never exceed 10 and no provider callback exists. Some owned-byte accounting occurs after effective construction. | Invalid hard ceiling and parser limits pass; complete boundary and pre-allocation instrumentation remains absent. | Partial |
| Generate the complete document-construct inventory for exact TF2, CS:S, and legacy Source 1 CS:GO content builds and integrate one VMT implementation with KeyValues, Content, Material, and every declared producer. | The parent format inventory is Not accepted; the 20-item candidate inventory has no generator or target-build occurrence data; no implementation or integration exists. | Regenerate from the accepted parent identity and exact archive indexes, require every VMT to classify into the declared syntax constructs, and audit all producers and consumers for one parser/composer with no embedded reader, alternate patch resolver, inferred path, or fallback. | Blocked |

## Generated Inventories

No generated document-construct inventory is accepted. Accepted item count: 0.

The manually derived candidate at [`inventories/document-constructs.md`](inventories/document-constructs.md) records:

| Field | Current value |
|---|---|
| Authority identity | Source SDK commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; candidate Source format inventory; exact TF2, CS:S, and legacy Source 1 CS:GO content-build archive indexes and VMT bytes are Missing. |
| Authority revision | SDK commit fixed; parent format inventory Not accepted; content-build revisions Missing. |
| Generator command | Missing. The future command must be checked in under `tools/playsrc` before inventory acceptance. |
| Output path | `packages/formats/vmt/inventories/document-constructs.md` |
| Item count | 20 candidate construct identities; 0 accepted items. |

The future generator must enumerate exact `.vmt` logical identities from accepted archive indexes, read only those indexed bytes, classify every root and descendant into one construct identity, retain exact source hashes and per-game occurrence counts, and fail on an unclassified structural form. Shader identifiers, parameter keys and values, flags, and proxy names are recorded only as opaque syntax occurrences; their semantic inventories belong to `packages/world/material`.

## Exit Criteria

The VMT package is Complete only when all of these predicates pass:

- All 18 Behavior Families rows are Ready.
- The parent Source format-universe denominator and this leaf denominator have Accepted review records for the same VMT identity.
- The document-construct inventory is generated, current, Accepted, and covers exact TF2, CS:S, and legacy Source 1 CS:GO content builds with no unclassified structural form.
- Fixed vectors cover ordinary and `Patch` roots; scalar, object, nested, empty, repeated, mixed-case, conditional, proxy, unknown, malformed, missing, cyclic, and over-limit documents.
- Patch vectors cover 0 through 10 include edges, every insert/replace ordering, repeated reserved members, nested composition, and every scalar/object collision.
- Every effective node has exact source provenance and every source syntax document remains immutable and byte-preservable through KeyValues.
- KeyValues, Content, Material, map compilation, games, and tools consume the sole VMT parser and patch composer; no embedded tokenizer, alternate resolver, inferred path, placeholder parse result, compatibility layer, or legacy reader remains.
- Every encountered document-level value and construct is classified `Handled`, `Intentionally inert`, `Unsupported`, `Malformed`, `Unknown`, or `Missing`, and no required item remains `Unsupported`, `Unknown`, `Missing`, `Partial`, or `Blocked`.

## Blockers

- **Parent denominator:** `packages/formats/ROADMAP.md` and `packages/formats/inventories/formats.md` state that the 62-row format candidate has 0 accepted items, no generator, five unresolved decisions, and 18 proposed leaf owners absent from the root Owner Registry. VMT inventory acceptance cannot precede that prerequisite.
- **Target content and generator:** `playsrc.local.json` is Missing, the local configuration contract has no CS:S or legacy CS:GO root, no exact declared-game archive indexes or VMT byte sets are checked in, and no command generates `inventories/document-constructs.md`. Checked `playsrc.local.example.json`, the current format inventory, `tools/`, and the official TF2 SDK material sample and call sites named under Inputs.
- **Semantic consumer denominator:** `packages/world/material/README.md` assigns shader, parameter, flag, proxy, animation, and material-state semantics to Material, but `packages/world/material/ROADMAP.md` does not exist. The VMT integration row cannot become Ready until that roadmap accepts the semantic inventories and consumes opaque VMT syntax without transferring semantic ownership back to this package.
