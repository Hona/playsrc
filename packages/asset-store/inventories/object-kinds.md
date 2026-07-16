# Asset Store Object-Kind Inventory

Status: Draft. This inventory is not accepted and does not enter the Asset Store completion denominator.

## Metadata

| Field | Value |
|---|---|
| Authority identity | playsrc `TERMINOLOGY.md`, `ARCHITECTURE.md`, `packages/asset-store/README.md`, and `packages/world/map/README.md`; RFC 8785 JSON Canonicalization Scheme; OCI Image Specification 1.1.1 descriptor and image-layout contracts; Bazel Remote Execution CAS contract |
| Authority revision | playsrc commit `3661338b4672233425c4c3e7a4b52f5599cee534`; OCI Image Specification `v1.1.1`; Bazel Remote APIs commit `becdd8f9ff811df88a22d3eadd6341753d51d167` |
| Generator command | Blocked: no checked-in generator exists |
| Output path | `packages/asset-store/inventories/object-kinds.md` |
| Item count | 5 |
| Accepted item count | 0 |
| Owner | `packages/asset-store` |

## Object Kinds

Every item is one immutable byte sequence stored at `objects/sha256/<first-two-hash-hex>/<64-hash-hex>`. The kind classifies a descriptor's semantic role; it never changes the byte identity or creates another physical copy.

| Identity | Exact representation and identity | Permitted outgoing asset-graph edges | Semantic-content owner |
|---|---|---|---|
| `artifact` | Opaque playsrc-compiled or application-build bytes; descriptor media type is producer-owned RFC 6838, with canonical decimal byte length and SHA-256 | None interpreted by the Asset Store | The format, world, runtime, presentation, game, or application producer that emits the bytes |
| `map-root` | RFC 8785 canonical UTF-8 JSON `{kind:"map-root",identity:{game,contentBuild,map},references}` with media type `application/vnd.playsrc.asset-root+json`; the manifest object's SHA-256 is the root identity | `artifact` | `packages/world/map` owns the map dependency description; `packages/asset-store` owns the root envelope and storage contract |
| `game-root` | RFC 8785 canonical UTF-8 JSON `{kind:"game-root",identity:{game,contentBuild},references}` with media type `application/vnd.playsrc.asset-root+json`; the manifest object's SHA-256 is the root identity | `artifact`, `map-root` | `games/<game>` owns the selected contents; `packages/asset-store` owns the root envelope and storage contract |
| `catalog` | RFC 8785 canonical UTF-8 JSON `{kind:"catalog",identity:{application,catalog},entries,references}` with media type `application/vnd.playsrc.asset-catalog+json`; `entries` is preserved application-owned I-JSON | `map-root`, `game-root` | `apps/web/<product>` owns entries and selection; `packages/asset-store` owns the catalog envelope and storage contract |
| `application-root` | RFC 8785 canonical UTF-8 JSON `{kind:"application-root",identity:{application,applicationBuild},references}` with media type `application/vnd.playsrc.asset-root+json`; the manifest object's SHA-256 is the root identity | `artifact`, `map-root`, `game-root`, `catalog` | `apps/web/<product>` owns the selected contents; `packages/asset-store` owns the root envelope and storage contract |

## Excluded Resource Kinds

The following resources are not immutable object kinds:

- A channel is a mutable conditionally replaced record at `channels/<channel-name>.json`.
- A retained-root set is mutable local store state and is not published as graph content.
- Validation, synchronization-plan, publication, interruption, and rollback reports are operation records and never become reachable graph nodes unless an owning producer deliberately submits their exact bytes as `artifact`.
- Raw Source bytes belong only to the Source cache and cannot be submitted as asset-store objects.
- Work-directory intermediates, remote multipart-upload parts, CDN cache entries, and browser HTTP cache entries are not object identities.

## Acceptance Blockers

- Implement the checked-in generator command from machine-readable current-contract declarations.
- Record the checkpoint containing those declarations as the authority revision.
- Verify that generation emits exactly 5 stable identities and the edge matrix above.
- Accept exact manifest, catalog, descriptor, graph, and numeric-bound contracts in the Asset Store roadmap.
