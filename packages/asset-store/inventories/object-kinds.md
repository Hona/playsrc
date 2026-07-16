# Asset Store Object-Kind Inventory

Status: Draft. This inventory is not accepted and does not enter the Asset Store completion denominator.

## Metadata

| Field | Value |
|---|---|
| Authority identity | playsrc `TERMINOLOGY.md`, `ARCHITECTURE.md`, `packages/asset-store/README.md`, and `packages/world/map/README.md`; RFC 8785 JSON Canonicalization Scheme; OCI Image Specification 1.1.1 descriptor and image-layout contracts; Bazel Remote Execution CAS contract |
| Authority revision | playsrc commit `3661338b4672233425c4c3e7a4b52f5599cee534`; OCI Image Specification `v1.1.1`; Bazel Remote APIs commit `becdd8f9ff811df88a22d3eadd6341753d51d167` |
| Generator command | Blocked: no checked-in generator exists |
| Output path | `packages/asset-store/inventories/object-kinds.md` |
| Item count | 7 |
| Accepted item count | 0 |
| Owner | `packages/asset-store` |

## Object Kinds

Every item is one immutable byte sequence stored at `objects/sha256/<first-two-hash-hex>/<64-hash-hex>`. The kind classifies a descriptor's semantic role; it never changes the byte identity or creates another physical copy.

| Identity | Exact representation and identity | Permitted outgoing asset-graph edges | Semantic-content owner |
|---|---|---|---|
| `source-object` | Exact immutable Source bytes with representation identity `bsp`, `vpk-directory`, `vpk-segment`, or owner-declared additional content; descriptor records media type, canonical decimal byte length, SHA-256, and provenance | None interpreted by the Asset Store | `packages/content` and format owners define acquisition and interpretation; Asset Store owns bytes and descriptors |
| `derived-object` | Reproducible bytes whose descriptor records source hashes, transitive dependency hashes, compiler behavior identity, build configuration identity, output role, media type, byte length, and SHA-256 | `source-object`, `derived-object` | The format, world, runtime, presentation, game, or application producer owns semantics; Asset Store owns bytes and descriptors |
| `source-root` | RFC 8785 canonical UTF-8 JSON `{kind:"source-root",identity:{game,contentBuild},entries,references}` mapping exact game-content logical identities to immutable Source objects | `source-object` | The game owns selected content-build entries; Content owns resolution semantics; Asset Store owns the envelope |
| `map-root` | Optional RFC 8785 canonical UTF-8 JSON `{kind:"map-root",identity:{game,contentBuild,map,bspSha256,compilerBehavior,buildConfiguration},references}` describing one automatically generated map runtime cache | `source-object`, `derived-object`, `source-root` | `packages/world/map` owns the runtime dependency description; Asset Store owns the envelope and storage contract |
| `game-root` | RFC 8785 canonical UTF-8 JSON `{kind:"game-root",identity:{game,contentBuild},references}` with media type `application/vnd.playsrc.asset-root+json`; the manifest object's SHA-256 is the root identity | `source-root`, `derived-object`, `map-root` | `games/<game>` owns the selected contents; `packages/asset-store` owns the root envelope and storage contract |
| `catalog` | RFC 8785 canonical UTF-8 JSON `{kind:"catalog",identity:{application,catalog},entries,references}` with media type `application/vnd.playsrc.asset-catalog+json`; `entries` is preserved application-owned I-JSON and may contain exact `MapSource` records | `source-object`, `source-root`, `map-root`, `game-root` | `apps/web/<product>` owns entries and selection; `packages/asset-store` owns the catalog envelope and storage contract |
| `application-root` | RFC 8785 canonical UTF-8 JSON `{kind:"application-root",identity:{application,applicationBuild},references}` with media type `application/vnd.playsrc.asset-root+json`; the manifest object's SHA-256 is the root identity | `derived-object`, `source-root`, `map-root`, `game-root`, `catalog` | `apps/web/<product>` owns the selected contents; `packages/asset-store` owns the root envelope and storage contract |

## Excluded Resource Kinds

The following resources are not immutable object kinds:

- A channel is a mutable conditionally replaced record at `channels/<channel-name>.json`.
- A retained-root set is mutable local store state and is not published as graph content.
- Validation, synchronization-plan, publication, interruption, and rollback reports are operation records and never become reachable graph nodes unless an owning producer deliberately submits their exact bytes as `artifact`.
- Work-directory intermediates, remote multipart-upload parts, CDN cache entries, and browser HTTP cache entries are not object identities.

## Acceptance Blockers

- Implement the checked-in generator command from machine-readable current-contract declarations.
- Record the checkpoint containing those declarations as the authority revision.
- Verify that generation emits exactly 7 stable identities and the edge matrix above.
- Accept exact manifest, catalog, descriptor, graph, and numeric-bound contracts in the Asset Store roadmap.
