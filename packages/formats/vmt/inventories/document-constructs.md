# VMT Document-Construct Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

Authority identity: Valve Source SDK 2013 commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`, the candidate Source format inventory at [`../../inventories/formats.md`](../../inventories/formats.md), and exact TF2, CS:S, and legacy Source 1 CS:GO content-build archive indexes and VMT bytes. The parent inventory is Not accepted and the content-build inputs are Missing.

Authority revision: SDK commit fixed; parent format inventory Not accepted; content-build revisions Missing.

Generator command: Missing.

Output path: `packages/formats/vmt/inventories/document-constructs.md`.

Candidate item count: 20. Accepted item count: 0.

Each row defines a document-syntax construct. Shader, parameter, flag, material-condition, proxy, texture-reference, and material-reference meanings remain owned by [`../../../world/material`](../../../world/material/README.md). Successful VMT parsing never classifies those meanings as `Handled`.

| Construct identity | Accepted syntax form | VMT-owned result | Semantics beyond syntax owner | Authority | Acceptance |
|---|---|---|---|---|---|
| `vmt-root-shader` | Exactly one non-empty KeyValues object whose root token is not case-insensitive `Patch` | Preserve the exact opaque shader token, root span, ordered body, and provenance | `packages/world/material` | SDK `imaterialsystem.h`, `IShader.h`, TF2 `new_tf2_logo.vmt`, and procedural-material call sites | Blocked by the Not accepted parent inventory and Missing generator/content indexes |
| `vmt-parameter-scalar` | A root or nested key with one scalar KeyValues value, including `$` parameter, material-flag, and `%` tool-key spellings | Preserve key/value token bytes, quote forms, inferred KeyValues type, order, span, and provenance | `packages/world/material` | SDK `imaterial.h`, `imaterialvar.h`, `IShader.h`, and TF2 material producers | Blocked by the Not accepted parent inventory and Missing generator/content indexes |
| `vmt-parameter-block` | A root or nested key whose value is a KeyValues object | Preserve node kind and complete ordered subtree | `packages/world/material` | SDK VMT-shaped KeyValues interfaces and material consumers | Blocked by the Not accepted parent inventory and Missing generator/content indexes |
| `vmt-nested-block` | An object below another non-root object at any accepted KeyValues depth | Preserve every ancestor, child, scalar, repeat, and span | `packages/world/material` | SDK KeyValues tree contract and material proxy interface | Blocked by the Not accepted parent inventory and Missing generator/content indexes |
| `vmt-empty-block` | An object with no child node | Preserve the empty-object kind distinctly from an empty scalar or missing key | `packages/world/material` | TF2 workshop VMT validation requires empty proxy declarations; SDK KeyValues distinguishes objects from scalars | Blocked by the Not accepted parent inventory and Missing generator/content indexes |
| `vmt-repeated-key` | Two or more sibling keys with the same ASCII-insensitive identity | Preserve every occurrence in source order and expose first-match lookup without deleting later nodes | `packages/world/material` decides semantic duplicate handling outside declared patch operations | SDK KeyValues ordered peer and first-lookup contract | Blocked by the Not accepted parent inventory and Missing generator/content indexes |
| `vmt-case-variant` | Root or descendant names differing only by ASCII case | Preserve each spelling; reserved VMT lookup remains ASCII-insensitive | `packages/world/material` | SDK case-insensitive material lookup and KeyValues symbol contract | Blocked by the Not accepted parent inventory and Missing generator/content indexes |
| `vmt-kv-condition` | A KeyValues bracket condition attached at an accepted KeyValues position | Retain the annotation and consume only a KeyValues-produced derived selection | Condition environment belongs to `packages/world/material`, a game, application, or tool | SDK KeyValues condition contract | Blocked by the Not accepted parent inventory and Missing generator/content indexes |
| `vmt-material-condition-key` | A parameter key containing a material-condition prefix, separator, and parameter suffix | Preserve the complete key and expose its syntactic components without evaluation | `packages/world/material` | playsrc VMT target contract; exact target-build occurrences require the generated inventory | Blocked by the Not accepted parent inventory and Missing generator/content indexes |
| `vmt-shader-override-block` | A root child object whose name participates in shader or render-configuration selection | Preserve it as an ordered nested block without selecting or merging it | `packages/world/material` | SDK `IShader.h` fallback contract; exact target-build block names require the generated inventory | Blocked by the Not accepted parent inventory and Missing generator/content indexes |
| `vmt-proxy-container` | The first case-insensitive root `Proxies` object, plus any later shadowed matching object | Identify the active structural section and preserve every section in source order | `packages/world/material` | SDK `imaterialproxy.h`, `imaterialproxyfactory.h`, and TF2 workshop VMT consumers | Blocked by the Not accepted parent inventory and Missing generator/content indexes |
| `vmt-proxy-declaration` | An ordered child of the active `Proxies` object whose key is an opaque proxy identifier and whose value is an object | Preserve identifier, repeats, order, complete subtree, and span | `packages/world/material` | SDK material-proxy initialization contract | Blocked by the Not accepted parent inventory and Missing generator/content indexes |
| `vmt-proxy-scalar-argument` | A scalar child below one proxy declaration | Preserve exact key/value syntax and order | `packages/world/material` | SDK material-proxy initialization receives the declaration KeyValues object | Blocked by the Not accepted parent inventory and Missing generator/content indexes |
| `vmt-proxy-block-argument` | An object child below one proxy declaration, including an empty or nested object | Preserve the complete subtree without flattening | `packages/world/material` | SDK material-proxy initialization receives the declaration KeyValues object | Blocked by the Not accepted parent inventory and Missing generator/content indexes |
| `vmt-patch-root` | Exactly one KeyValues object whose root token equals `Patch` without ASCII case | Enter VMT patch validation and composition while preserving the source object | VMT owns document composition; material semantics remain in `packages/world/material` | SDK `materialpatch.{h,cpp}` | Blocked by the Not accepted parent inventory and Missing generator/content indexes |
| `vmt-patch-include` | The first case-insensitive `include` member with one non-empty scalar logical-path token | Emit one exact dependency request and retain later matching members as shadowed syntax | `packages/content` resolves the request | SDK `materialpatch.{h,cpp}` and Content logical-path contract | Blocked by the Not accepted parent inventory and Missing generator/content indexes |
| `vmt-patch-insert` | The first case-insensitive object-valued `insert` member | Accumulate and recursively apply append-or-update operations with provenance | VMT owns composition; inserted names and values remain semantically owned by `packages/world/material` | SDK `materialpatch.{h,cpp}` plus the playsrc recursive patch target contract | Blocked by the Not accepted parent inventory and Missing generator/content indexes |
| `vmt-patch-replace` | The first case-insensitive object-valued `replace` member | Accumulate and recursively apply existing-key-only operations with provenance | VMT owns composition; replaced names and values remain semantically owned by `packages/world/material` | SDK `materialpatch.{h,cpp}` plus the playsrc recursive patch target contract | Blocked by the Not accepted parent inventory and Missing generator/content indexes |
| `vmt-unknown-ordinary-member` | A structurally valid scalar or object member not assigned a document-level VMT role | Preserve it without parser rejection or semantic success | `packages/world/material` assigns `Handled`, `Intentionally inert`, `Unsupported`, or `Unknown` | SDK shader and material-variable enumeration interfaces require separation of syntax from semantic registries | Blocked by the Not accepted parent inventory and Missing generator/content indexes |
| `vmt-unknown-patch-member` | A structurally valid `Patch` root member other than the active `include`, `insert`, or `replace` members | Preserve it and record its target-defined no-effect result as `Intentionally inert` | VMT owns the no-effect composition result | SDK `materialpatch.{h,cpp}` recognizes only the three active member identities | Blocked by the Not accepted parent inventory and Missing generator/content indexes |

## Generation Contract

The future checked-in generator must:

1. Consume the Accepted identity and content hash of `packages/formats/inventories/formats.md`.
2. Consume exact archive indexes and content-build identities for TF2, CS:S, and legacy Source 1 CS:GO.
3. Resolve only indexed `.vmt` logical identities through the declared Content provider plans and retain each file's provenance and SHA-256 hash.
4. Parse in KeyValues syntax-preservation mode, classify every root and descendant into the stable identities above, and record per-game document counts, construct occurrence counts, distinct opaque-token counts, and representative logical identities with hashes.
5. Record shader identifiers, parameter names and values, flag-shaped names, material-condition tokens, proxy identifiers, and proxy arguments as opaque syntax occurrence sets. It never assigns their semantics.
6. Fail on an unclassified root, node kind, reserved-member shape, condition form, path result, malformed document, missing indexed file, or exhausted bound instead of omitting it.
7. Sort by construct identity and emit this file byte-identically from fixed inputs.

Acceptance requires all three declared content builds, an Accepted parent inventory, one checked-in generator command, two byte-identical clean-work-directory runs, exact item and occurrence counts, no unclassified structural form, and denominator review metadata satisfying `docs/roadmap-contract.md`.
