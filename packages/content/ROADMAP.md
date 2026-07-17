# Content Roadmap

[`../../TERMINOLOGY.md`](../../TERMINOLOGY.md) defines delivery statuses, coverage classifications, logical paths, provenance, content builds, source cache, and Complete. [`../../docs/roadmap-contract.md`](../../docs/roadmap-contract.md) defines denominator and evidence requirements.

## Completion Denominator

The completion denominator contains exactly the 29 Behavior Families rows below. [`inventories/provider-contracts.md`](inventories/provider-contracts.md) defines 8 draft provider and workflow contracts; those items do not enter the denominator until a checked-in generator exists and the inventory passes the denominator review gate.

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
| Accept a non-empty UTF-8 logical path composed of non-empty slash-separated components. | The Rust Content plan accepts bounded UTF-8 requests and emits one canonical non-empty identity before provider access. Native invalid-UTF-8 boundaries remain absent. | Fixed empty, one-component, multicomponent, and normalized-component vectors pass. | Partial |
| Normalize `\` to `/`, collapse repeated separators, remove `.` components, fold ASCII letters to lowercase, and preserve non-ASCII code points exactly. | One Rust normalizer performs every listed transformation without locale-sensitive folding. | A mixed case, backslash, repeated-separator, and dot-component vector produces one exact identity used by all provider kinds. | Ready |
| Reject absolute paths, drive prefixes, URI schemes, `..`, NUL, ASCII controls, query strings, and fragments before provider access. | Every listed request form fails before candidate construction. | Fixed traversal/injection vectors return `InvalidPath`; complete provider-call instrumentation remains absent. | Partial |
| Register each provider with one unique stable identity, kind, revision, exact configured source location, path-ID set, request-only flag, and bounds. | No implementation exists. | Provider-plan validation vectors comparing accepted state and exact malformed-field diagnostics. | Not started |
| Reject duplicate provider identities and duplicate registrations of the same exact source identity. | No implementation exists. | Plan vectors with ASCII-case identity collisions, repeated source identities, and one source intentionally registered under distinct path-ID memberships. | Not started |
| Preserve declared provider order and allow explicit head or tail insertion only while constructing a new immutable plan. | No implementation exists. | Shadowed-resource vectors comparing serial results for every provider permutation and proving mutation is unavailable after construction. | Not started |
| Associate one provider with one or more case-insensitive path IDs without duplicating its underlying index or bytes. | No implementation exists. | Multi-ID vectors proving one provider identity, one source read, and identical provenance through each eligible ID. | Not started |
| Restrict explicit path-ID requests and exclude request-only providers from omitted-ID requests. | No implementation exists. | Vectors covering matching, nonmatching, omitted, request-only, and ASCII-case path-ID requests with ordered checked-location reports. | Not started |
| Resolve a loose-directory candidate by bounded component-wise lookup beneath its configured root without recursive search or symlink escape. | The native adapter enumerates only each requested component, repairs ASCII case, rejects symlinks, and accepts only the final regular file. | Temporary-directory vectors cover case repair, exact read, and missing components. Symlink, permission, and root-replacement vectors remain absent. | Partial |
| Reject a loose-directory component when multiple entries have the same ASCII-folded identity. | No implementation exists. | Directory vectors containing case-only sibling collisions at root, intermediate, and leaf components. | Not started |
| Consume a validated VPK index and exact ranged-entry reader without parsing, extracting, or scanning the archive. | Content delegates index parsing and full-entry integrity to VPK and accepts either native files or supplied immutable directory bytes plus a thread-safe segment-range adapter. | Native and supplied-adapter vectors read the same segmented entry; selected CRC corruption and missing segments fail without consulting a lower provider. | Partial |
| Register one validated BSP PAK for the active map under `GAME`, place it before external providers, and remove the prior map PAK atomically. | `with_active_pak` derives a new immutable plan whose one PAK precedes supplement and game providers; the prior plan remains unchanged. | PAK shadowing, unsupported selected entry, and prior-map exclusion from `resolve_map` pass. Concurrent transition evidence remains absent. | Partial |
| Read an immutable remote BSP, VPK directory, VPK segment, or supplemental object by whole-object or exact byte range and verify every returned byte against its declared object identity. | No range-capable remote provider exists. | HTTP vectors covering whole and ranged `200`/`206`, `Content-Range`, ignored range, short/long body, changed ETag, hash mismatch, cancellation, cache hit, and retry prohibition; compare exact requested ranges and verified bytes. | Not started |
| Mount one optional map-supplement index after the active BSP PAK and before selected game-content providers without making the supplement mandatory or changing game VPK order. | An absent supplement is inert. `with_map_supplement` installs one immutable exact-path/object index between PAK and game providers; selected bytes must match declared length and SHA-256. | PAK/supplement/directory shadowing and selected missing supplement vectors pass. A selected corrupt object follows the same failure branch; broader atlas evidence remains absent. | Partial |
| Reuse raw bytes only through a cache entry keyed by provider identity, provider revision, normalized logical path, and source content hash. | The TypeScript Content package verifies a declared HTTPS response's status, redirect absence, byte length, and SHA-256, performs exact-size bzip2 decoding, verifies the decoded identity, commits both immutable objects only after the final cancellation boundary, rejects corrupt cached bytes, and returns root-relative provenance. Directory/VPK/PAK provider cache keys remain unimplemented. | Cold/warm fixed bzip2 vectors prove one request, encoded/decoded byte identity, warm reuse, HTTP/length/hash failure, corrupt-cache rejection, cancellation before fetch and during a cold body with zero installed objects, malformed-source rejection before fetch, and no fallback. Live `jump_beef` cold/warm runs produced the two declared objects and independent `shasum`/length matches. | Partial |
| Return the first eligible exact entry and stop checking lower-priority providers after a successful read. | Native resolution evaluates PAK, supplement, then external providers and returns immediately at the first exact success. | Directory-directory, PAK-supplement-directory, and VPK-directory shadow vectors return only the first bytes. | Ready |
| Stop on malformed, corrupt, changed, unreadable, or over-budget data at the first provider that declares the requested identity. | Selected PAK, supplement, VPK, and directory failures return typed errors and never resume candidate iteration. | Unsupported PAK, missing supplement, corrupt VPK, and over-bound result paths prove no valid lower directory is returned. Complete adapter fault injection remains absent. | Partial |
| Emit complete provenance for every successful directory, VPK, BSP PAK, and cache-backed read. | Native directory, VPK, PAK, and supplement results contain game, content build, canonical path, provider identity/kind/revision, provider-relative location, length, and SHA-256. Existing HTTPS acquisition retains its cache provenance contract separately. | Fixed directory SHA-256/provenance, provider-kind, VPK location, supplement, and exact-map PAK outputs pass; complete cache-backed integration remains absent. | Partial |
| Preserve cross-provider duplicate logical identities as ordered shadows and report the winner plus every shadow without reading shadow bytes. | No implementation exists. | Three-provider duplicate vectors comparing winner, shadow identities, and read counters. | Not started |
| Return `Missing` only after checking every eligible exact location in deterministic order. | `Missing` contains every eligible provider identity, kind, and provider-relative location in plan order. | A two-directory all-missing vector compares exact order; mixed-provider and path-ID matrices remain absent. | Partial |
| Reject malformed provider plans, indexes, ambiguous identities, escaped locations, and invalid revisions before they can produce bytes. | No implementation exists. | Provider-specific malformed vectors proving classification, stable diagnostic code, and zero partial registration. | Not started |
| Distinguish missing candidates from provider I/O, permission, integrity, short-read, changed-source, and unavailable-adapter failures. | No implementation exists. | Fault-injection matrix comparing exact error kind, selected provider, operation, and absence of fallback bytes. | Not started |
| Cancel queued or active lookup work, release owned resources, and prevent cancelled bytes from entering the source cache. | No implementation exists. | Controlled cancellation before dispatch, between candidates, during read, after read before hash, and after completion with resource-leak and cache assertions. | Not started |
| Execute concurrent reads with an immutable provider snapshot, bounded shared resources, and serial-equivalent results. | No implementation exists. | Deterministic schedules covering same-path coalescing, distinct paths, cancellation of one waiter, provider failure, map transition, and repeated result/provenance equality. | Not started |
| Enforce numeric maxima for logical-path bytes, providers, path IDs, candidates, queued reads, concurrent reads, bytes per read, total in-flight bytes, cache-entry bytes, cache bytes, and failure-report entries with backpressure. | Exact playsrc limits have not been accepted; the SDK establishes 128 search paths but does not establish the complete browser/native bound set. | Boundary vectors at each accepted maximum and maximum-plus-one, plus queue saturation and memory measurements under fixed configuration. | Blocked |
| Resolve a normal map request only as `maps/<map>.bsp` through eligible configured external providers; the requested map's PAK is unavailable until that BSP succeeds. | `resolve_map` admits only `maps/*.bsp` and bypasses active PAK and supplement state, including state derived from a prior map. | A prior PAK declaring the requested map cannot shadow the exact external directory BSP. VPK-map and all-missing vectors remain absent. | Partial |
| Resolve a normal resource request through the active map PAK first, optional map supplement second, then selected game-content providers in exact plan order, using cache entries only on behalf of each candidate. | `resolve_resource` implements exactly PAK → supplement → game providers with immutable plan snapshots. | Fixed three-level shadowing, selected failures, native/supplied VPK reads, and an exact `jump_beef` PAK/VPK run pass. Cache integration and broader resource families remain absent. | Partial |
| Close one declared map's exact browser dependency graph over every current VMT patch/base, selected VTF, MDL companion/include, PCF root/material, WAVE, script, sky face, cubemap, decal, and water request while preserving optional exact absences and non-content blockers. | `jump_beef.dependencies.json` records 345 exact requests: 296 resolved PSDB entries and 49 optional authoritative absences. Every resolved row carries byte length, SHA-256, provider kind/revision/location and consumers; every absence carries all 13 checked locations. Browser evidence partitions all 16 support diagnostics into 0 content, 15 behavior, and 1 platform blocker without changing those owners. | Repeated generation produces PSDB SHA-256 `494c282a…4884`, ledger SHA-256 `8c721c90…4b8b`, exact extension counts, and byte-identical descriptors. Fresh headed cold/warm browser acceptance reports `contentBlockers: []`; unit vectors cover cold/warm/corrupt/cancel cache and publication paths. | Ready |
| Reproduce the content-build-specific TF2, CS:S, and legacy Source 1 CS:GO provider order from exact configured manifests and archive indexes without hardcoded game mounts. | The TF2 source-bundle operation verifies app `440`, build `24207079`, its three installed depot manifests and sizes, patch `10822003`, and `gameinfo.txt` SHA-256 `a85196fd…982da`; it parses GAME path IDs, inactive low-violence disposition, exact wildcard expansion, VPK-directory identities, loose roots, and request order. The target plan is BSP PAK followed by 12 configured GAME providers. CS:S, legacy CS:GO, and generic generated provider inventories remain absent. | Exact `jump_beef` resolution selects 10 PAK, 95 TF2 texture, 8 TF2 sound, 151 TF2 miscellaneous, 17 HL2 texture, and 15 HL2 miscellaneous entries. Every selected VPK entry passes CRC validation; each provider records its index SHA-256 or configured directory revision. | Partial |

## Generated Inventories

[`inventories/provider-contracts.md`](inventories/provider-contracts.md) contains 8 draft items and 0 accepted items. Its generator command is Blocked because no production code may be implemented during roadmap research.

The TF2, CS:S, and legacy Source 1 CS:GO provider-plan inventories are also Blocked. Their authoritative inputs must be the exact configured `gameinfo.txt`, content-build identity, wildcard directory entries, VPK directory indexes, and loose roots. The ownership paths require project review because game-specific lists cannot live in this generic package.

## Exit Criteria

- All 29 Behavior Families rows are Ready.
- The provider-contract inventory is generated by its checked-in command, records all mandatory metadata, contains exactly the accepted items, and passes denominator review.
- The TF2, CS:S, and legacy Source 1 CS:GO provider-plan inventories have accepted owners and reproduce their configured content builds exactly.
- Fixed vectors cover directory, VPK, BSP PAK, and raw-source cache reads through synchronous and asynchronous consumers.
- Every producer and consumer uses the same current provider plan, logical-path identity, provenance, error, cancellation, and bounds contracts.
- Normal map and resource workflows report every exact candidate checked and perform no discovery, fallback-root selection, archive extraction, or recursive filesystem search.
- No required value or behavior remains Unsupported, Unknown, Missing, Partial, or Blocked.

## Blockers

- **Other configured retail authorities:** TF2 build `24207079`, patch `10822003`, its app/depot manifest, `gameinfo.txt`, and selected archive indexes are available through the exact local configuration. The current three-field contract has no `cssDir` or `csgoDir`, so exact CS:S and legacy CS:GO manifests and archive indexes remain unavailable.
- **Game-plan ownership:** generic `packages/content` cannot own hardcoded TF2, CS:S, or legacy CS:GO mount lists. Proposed ownership is `games/tf2/inventories/content-providers.md`, `games/css/inventories/content-providers.md`, and `games/csgo/inventories/content-providers.md`, generated through a Content-owned plan contract. No assigned public path permits those files in this task.
- **Numeric bounds:** exact accepted maxima for browser and native provider execution are unresolved. Checked official `IFileSystem` limits and asynchronous statuses plus both frozen PoCs. These sources do not establish one complete playsrc bound set.
- **Inventory generator:** no checked-in command generates `inventories/provider-contracts.md`; production implementation is prohibited in this research task.
