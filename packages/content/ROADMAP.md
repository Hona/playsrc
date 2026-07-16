# Content Roadmap

[`../../TERMINOLOGY.md`](../../TERMINOLOGY.md) defines delivery statuses, coverage classifications, logical paths, provenance, content builds, source cache, and Complete. [`../../docs/roadmap-contract.md`](../../docs/roadmap-contract.md) defines denominator and evidence requirements.

## Completion Denominator

The completion denominator contains exactly the 28 Behavior Families rows below. [`inventories/provider-contracts.md`](inventories/provider-contracts.md) defines 8 draft provider and workflow contracts; those items do not enter the denominator until a checked-in generator exists and the inventory passes the denominator review gate.

Content is Complete only for configured providers. Installation discovery, registry lookup, Steam-library scanning, recursive machine search, and fallback roots are excluded rather than deferred.

## Inputs

- One immutable provider plan containing an ordered list of provider identities, provider kinds, provider revisions, path-ID memberships, request-only flags, exact configured source locations, and numeric bounds.
- One UTF-8 logical path and either one explicit case-insensitive path ID or no path ID.
- Directory roots, local or remote VPK indexes and ranged-entry readers, BSP PAK indexes and entry readers, raw-source cache entries, immutable HTTP object descriptors, map-supplement indexes, and HTTPS download declarations with exact encoded/decoded identities supplied through provider adapters.
- Game, content-build identity, cancellation signal, and operation-wide read and concurrency budgets.

## Outputs

- On success: exact source bytes and provenance containing game, content build, normalized logical path, provider identity, provider kind, provider revision, exact location inside that provider, byte length, and lowercase SHA-256 content hash.
- On absence: `Missing` plus the ordered list of every eligible exact location checked.
- On invalid request or provider state: `Malformed` with the rejected identity and invariant.
- On unavailable implemented behavior: `Unsupported` with the owning provider operation.
- On read failure, cancellation, or exhausted bound: a distinct deterministic error containing the normalized request, provider identity when selected, operation, and no substitute bytes.

## Invariants

- Provider order is immutable for the lifetime of one content instance. The first eligible provider containing the normalized identity is authoritative.
- A selected entry that is malformed, unreadable, corrupt, changed, or over budget fails the lookup. Resolution never continues to a lower-priority provider.
- Logical identity uses `/`, ignores ASCII case, preserves non-ASCII code points exactly, and contains no empty, `.` or `..` component.
- Absolute paths, drive prefixes, URI schemes, NUL, ASCII control characters, query strings, fragments, and paths exceeding the accepted byte bound are Malformed.
- Path IDs compare without ASCII case. An explicit path ID restricts lookup to providers carrying that ID. An omitted path ID excludes every provider marked request-only.
- An active map PAK eligible for the request precedes every external provider. Mounting a new map replaces the prior active map PAK atomically.
- Cache hits preserve the originating provider's provenance and precedence. The source cache is never an independent fallback root.
- Concurrent lookups against one content instance observe the same provider snapshot and return the same result as serial lookup order.
- Every failure report lists candidates in lookup order and contains no operating-system path outside the configured roots.

## Ownership Exclusions

- `packages/formats/keyvalues` owns parsing `gameinfo.txt`; Content consumes typed search-path entries.
- `packages/formats/vpk` owns VPK versions, directory trees, entry metadata, integrity validation, and ranged entry reads; Content owns where a validated VPK provider appears in lookup order.
- `packages/formats/bsp` owns BSP structure and the embedded PAK lump; Content owns active-map PAK registration and precedence over external providers.
- Format and semantic packages own VMT, VTF, model, sound, script, map, and demo decoding after bytes resolve.
- `packages/asset-store` owns playsrc-compiled objects, roots, catalogs, channels, and publication. The source cache stores only raw Source bytes.
- `tools/playsrc` owns configuration loading and repeated commands. Content receives validated `tf2Dir`, `sourceCacheDir`, and future game-root identities; it never discovers them.
- `games/tf2`, `games/css`, and `games/csgo` own their content-build-specific provider plans. The generic Content package owns plan validation and execution, not game-specific mount lists.

## Behavior Families

| Target behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| Accept a non-empty UTF-8 logical path composed of non-empty slash-separated components. | No implementation exists. | Table vectors covering shortest valid path, multicomponent path, non-ASCII exact code points, empty input, empty component, trailing slash, and invalid UTF-8 at native boundaries. | Not started |
| Normalize `\` to `/`, collapse repeated separators, remove `.` components, fold ASCII letters to lowercase, and preserve non-ASCII code points exactly. | No implementation exists. | Cross-platform vectors proving one normalized identity and identical provider selection for every accepted spelling. | Not started |
| Reject absolute paths, drive prefixes, URI schemes, `..`, NUL, ASCII controls, query strings, and fragments before provider access. | No implementation exists. | Synthetic traversal and injection vectors instrumenting providers to prove zero provider calls for every rejected request. | Not started |
| Register each provider with one unique stable identity, kind, revision, exact configured source location, path-ID set, request-only flag, and bounds. | No implementation exists. | Provider-plan validation vectors comparing accepted state and exact malformed-field diagnostics. | Not started |
| Reject duplicate provider identities and duplicate registrations of the same exact source identity. | No implementation exists. | Plan vectors with ASCII-case identity collisions, repeated source identities, and one source intentionally registered under distinct path-ID memberships. | Not started |
| Preserve declared provider order and allow explicit head or tail insertion only while constructing a new immutable plan. | No implementation exists. | Shadowed-resource vectors comparing serial results for every provider permutation and proving mutation is unavailable after construction. | Not started |
| Associate one provider with one or more case-insensitive path IDs without duplicating its underlying index or bytes. | No implementation exists. | Multi-ID vectors proving one provider identity, one source read, and identical provenance through each eligible ID. | Not started |
| Restrict explicit path-ID requests and exclude request-only providers from omitted-ID requests. | No implementation exists. | Vectors covering matching, nonmatching, omitted, request-only, and ASCII-case path-ID requests with ordered checked-location reports. | Not started |
| Resolve a loose-directory candidate by bounded component-wise lookup beneath its configured root without recursive search or symlink escape. | No implementation exists. | Temporary-directory vectors covering exact case, case repair, missing components, symlinks, non-files, permission errors, root replacement, and containment. | Not started |
| Reject a loose-directory component when multiple entries have the same ASCII-folded identity. | No implementation exists. | Directory vectors containing case-only sibling collisions at root, intermediate, and leaf components. | Not started |
| Consume a validated VPK index and exact ranged-entry reader without parsing, extracting, or scanning the archive. | No implementation exists. | Adapter contract vectors for preload-only, embedded, segmented, missing, malformed-index, integrity-failure, short-read, and changed-entry outcomes supplied by `packages/formats/vpk`. | Not started |
| Register one validated BSP PAK for the active map under `GAME`, place it before external providers, and remove the prior map PAK atomically. | No implementation exists. | Two-map transition vectors proving PAK-first shadowing, empty PAK behavior, exact replacement, explicit non-`GAME` exclusion, and no stale-map result under concurrent reads. | Not started |
| Read an immutable remote BSP, VPK directory, VPK segment, or supplemental object by whole-object or exact byte range and verify every returned byte against its declared object identity. | No range-capable remote provider exists. | HTTP vectors covering whole and ranged `200`/`206`, `Content-Range`, ignored range, short/long body, changed ETag, hash mismatch, cancellation, cache hit, and retry prohibition; compare exact requested ranges and verified bytes. | Not started |
| Mount one optional map-supplement index after the active BSP PAK and before selected game-content providers without making the supplement mandatory or changing game VPK order. | No map-supplement provider exists. | Three-provider shadow vectors compare PAK, supplement, and game VPK results; absent supplement is Intentionally inert, while a declared but missing/corrupt supplement entry fails without lower-provider substitution. | Not started |
| Reuse raw bytes only through a cache entry keyed by provider identity, provider revision, normalized logical path, and source content hash. | The TypeScript Content package verifies a declared HTTPS response's status, redirect absence, byte length, and SHA-256, performs exact-size bzip2 decoding, verifies the decoded identity, atomically installs both immutable objects by hash, rejects corrupt cached bytes, and returns root-relative provenance. Directory/VPK/PAK provider cache keys and cancellation remain unimplemented. | Cold/warm fixed bzip2 vectors prove one request, encoded/decoded byte identity, warm reuse, HTTP/length/hash failure, corrupt-cache rejection, malformed-source rejection before fetch, and no fallback. Live `jump_beef` cold/warm runs produced the two declared objects and independent `shasum`/length matches. | Partial |
| Return the first eligible exact entry and stop checking lower-priority providers after a successful read. | No implementation exists. | Instrumented mixed-provider vectors proving candidate call order, one returned identity, and zero calls below the winner. | Not started |
| Stop on malformed, corrupt, changed, unreadable, or over-budget data at the first provider that declares the requested identity. | No implementation exists. | Fault-injected shadow vectors proving the lower-priority valid duplicate is never returned and the selected provider identifies the failure. | Not started |
| Emit complete provenance for every successful directory, VPK, BSP PAK, and cache-backed read. | No implementation exists. | Provenance snapshots validated against fixed source bytes, provider revisions, exact archive locations, and independently computed SHA-256 hashes. | Not started |
| Preserve cross-provider duplicate logical identities as ordered shadows and report the winner plus every shadow without reading shadow bytes. | No implementation exists. | Three-provider duplicate vectors comparing winner, shadow identities, and read counters. | Not started |
| Return `Missing` only after checking every eligible exact location in deterministic order. | No implementation exists. | All-missing vectors comparing the complete provider identity, kind, exact candidate location, and outcome sequence byte-for-byte across repeated runs. | Not started |
| Reject malformed provider plans, indexes, ambiguous identities, escaped locations, and invalid revisions before they can produce bytes. | No implementation exists. | Provider-specific malformed vectors proving classification, stable diagnostic code, and zero partial registration. | Not started |
| Distinguish missing candidates from provider I/O, permission, integrity, short-read, changed-source, and unavailable-adapter failures. | No implementation exists. | Fault-injection matrix comparing exact error kind, selected provider, operation, and absence of fallback bytes. | Not started |
| Cancel queued or active lookup work, release owned resources, and prevent cancelled bytes from entering the source cache. | No implementation exists. | Controlled cancellation before dispatch, between candidates, during read, after read before hash, and after completion with resource-leak and cache assertions. | Not started |
| Execute concurrent reads with an immutable provider snapshot, bounded shared resources, and serial-equivalent results. | No implementation exists. | Deterministic schedules covering same-path coalescing, distinct paths, cancellation of one waiter, provider failure, map transition, and repeated result/provenance equality. | Not started |
| Enforce numeric maxima for logical-path bytes, providers, path IDs, candidates, queued reads, concurrent reads, bytes per read, total in-flight bytes, cache-entry bytes, cache bytes, and failure-report entries with backpressure. | Exact playsrc limits have not been accepted; the SDK establishes 128 search paths but does not establish the complete browser/native bound set. | Boundary vectors at each accepted maximum and maximum-plus-one, plus queue saturation and memory measurements under fixed configuration. | Blocked |
| Resolve a normal map request only as `maps/<map>.bsp` through eligible configured external providers; the requested map's PAK is unavailable until that BSP succeeds. | No implementation exists. | Directory/VPK shadow vectors comparing every exact checked location, selected BSP provenance, and proof that no map PAK was queried for its containing BSP. | Not started |
| Resolve a normal resource request through the active map PAK first, optional map supplement second, then selected game-content providers in exact plan order, using cache entries only on behalf of each candidate. | No implementation exists. | Material, model, sound, script, particle, and map-adjacent resource vectors comparing the full candidate sequence and successful provenance. | Not started |
| Reproduce the content-build-specific TF2, CS:S, and legacy Source 1 CS:GO provider order from exact configured manifests and archive indexes without hardcoded game mounts. | `playsrc.local.json` is missing; exact configured manifests, content-build identities, and archive indexes were unavailable. Game-specific inventory ownership is unresolved. | Generated provider-plan inventories compared entry-for-entry with each configured `gameinfo.txt`, wildcard expansion, VPK directory index, and loose root at its recorded content build. | Blocked |

## Generated Inventories

[`inventories/provider-contracts.md`](inventories/provider-contracts.md) contains 8 draft items and 0 accepted items. Its generator command is Blocked because no production code may be implemented during roadmap research.

The TF2, CS:S, and legacy Source 1 CS:GO provider-plan inventories are also Blocked. Their authoritative inputs must be the exact configured `gameinfo.txt`, content-build identity, wildcard directory entries, VPK directory indexes, and loose roots. The ownership paths require project review because game-specific lists cannot live in this generic package.

## Exit Criteria

- All 28 Behavior Families rows are Ready.
- The provider-contract inventory is generated by its checked-in command, records all mandatory metadata, contains exactly the accepted items, and passes denominator review.
- The TF2, CS:S, and legacy Source 1 CS:GO provider-plan inventories have accepted owners and reproduce their configured content builds exactly.
- Fixed vectors cover directory, VPK, BSP PAK, and raw-source cache reads through synchronous and asynchronous consumers.
- Every producer and consumer uses the same current provider plan, logical-path identity, provenance, error, cancellation, and bounds contracts.
- Normal map and resource workflows report every exact candidate checked and perform no discovery, fallback-root selection, archive extraction, or recursive filesystem search.
- No required value or behavior remains Unsupported, Unknown, Missing, Partial, or Blocked.

## Blockers

- **Configured retail authority:** `playsrc/playsrc.local.json` does not exist. The exact TF2 `gameinfo.txt`, TF2 content-build identity, and archive indexes could not be read. The current local contract has no `cssDir` or `csgoDir`, so exact CS:S and legacy CS:GO manifests cannot be configured. Checked `playsrc.local.example.json`, the official SDK `game/mod_tf/gameinfo.txt`, public TF2 tracking, public CS:S documentation, and a public legacy CS:GO manifest; secondary sources do not establish the configured target builds.
- **Game-plan ownership:** generic `packages/content` cannot own hardcoded TF2, CS:S, or legacy CS:GO mount lists. Proposed ownership is `games/tf2/inventories/content-providers.md`, `games/css/inventories/content-providers.md`, and `games/csgo/inventories/content-providers.md`, generated through a Content-owned plan contract. No assigned public path permits those files in this task.
- **Numeric bounds:** exact accepted maxima for browser and native provider execution are unresolved. Checked official `IFileSystem` limits and asynchronous statuses plus both frozen PoCs. These sources do not establish one complete playsrc bound set.
- **Inventory generator:** no checked-in command generates `inventories/provider-contracts.md`; production implementation is prohibited in this research task.
