# Content Provider Contract Inventory

Status: Draft. This inventory is not accepted and does not enter the Content completion denominator.

## Metadata

| Field | Value |
|---|---|
| Authority identity | Valve Source SDK 2013 `src/public/filesystem.h`, `src/public/filesystem_init.h`, `src/public/filesystem_init.cpp`, `game/mod_tf/gameinfo.txt`; playsrc architecture and terminology |
| Authority revision | Source SDK commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`; playsrc working tree pending checkpoint |
| Generator command | Blocked: no checked-in generator exists |
| Output path | `packages/content/inventories/provider-contracts.md` |
| Item count | 6 |
| Owner | `packages/content` |

## Provider Contracts

| Identity | Input representation | Exact candidate location | Required result | Adjacent owner |
|---|---|---|---|---|
| `directory` | Configured non-symlink directory root, provider revision, path-ID set, request-only flag, and bounds | Root joined component-by-component with the normalized logical path; every component remains beneath the root | Exact bytes and directory provenance; Missing; ambiguous-case Malformed; or distinct containment, type, permission, changed-source, cancellation, and bound errors | Operating-system adapter performs bounded directory and file I/O; Content owns lookup semantics |
| `vpk` | Validated VPK index and exact ranged-entry reader, provider revision, path-ID set, request-only flag, and bounds | VPK provider identity plus normalized logical path and indexed preload, archive segment, offset, length, and integrity identity | Exact concatenated entry bytes and VPK provenance; Missing; or propagated malformed-index, integrity, range, short-read, changed-source, cancellation, and bound errors | `packages/formats/vpk` owns archive parsing, validation, and entry reads |
| `bsp-pak` | Active map identity plus validated embedded-PAK index and entry reader under the `GAME` path ID | BSP provenance plus PAK lump identity and normalized logical path | PAK-first bytes and BSP PAK provenance; Missing; or propagated malformed-index, integrity, read, cancellation, and bound errors | `packages/formats/bsp` owns BSP and embedded-lump parsing |
| `raw-source-cache` | Provider identity, provider revision, normalized logical path, source content hash, exact source provenance, and immutable cached bytes | Source-cache object selected only through the originating provider candidate | Hash-verified source bytes with unchanged origin provenance; invalid or corrupt entries fail and never become an alternate provider | `tools/playsrc` configures `sourceCacheDir`; Content owns cache identity and validity |

## Workflow Contracts

| Identity | Ordered exact locations checked | Success | Failure |
|---|---|---|---|
| `normal-map` | For normalized `maps/<map>.bsp`, visit each eligible configured directory or VPK provider in plan order. For each candidate, check a valid cache entry on behalf of that provider, then its exact directory path or VPK entry. Do not query the requested map's PAK. | Return first exact BSP bytes and provenance; parsing and active-PAK registration occur after resolution. | Stop on a declared but failed candidate; otherwise return Missing with every eligible provider candidate in order. |
| `normal-resource` | Visit the eligible active BSP PAK first. Then visit each eligible configured external provider in plan order. For each candidate, check a valid cache entry on behalf of that provider, then its exact PAK entry, VPK entry, or directory path. | Return first exact bytes and provenance and do not read shadows. | Stop on a declared but failed candidate; otherwise return Missing with the active PAK and every eligible external candidate in order. |

## Excluded Locations

Every workflow checks only locations represented by the immutable provider plan and the active map PAK. It never checks:

- Windows registry keys.
- Steam installation metadata or library folders.
- Parent or sibling directories inferred from a configured root.
- User profiles, drives, working directories, executable directories, or environment variables.
- Recursively discovered files outside a component-wise exact directory lookup.
- An archive not represented by a supplied validated index.
- A fallback root after a configured provider is missing, malformed, corrupt, unreadable, or stale.

## Acceptance Blockers

- Implement the checked-in generator command.
- Record an immutable playsrc checkpoint as the authority revision.
- Accept exact numeric bounds in the Content roadmap.
- Resolve ownership and generate content-build-specific TF2, CS:S, and legacy Source 1 CS:GO provider plans.
