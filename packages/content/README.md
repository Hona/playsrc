# Content

## Objective

Resolve exact Source logical paths across explicitly configured content providers.

## Responsibilities

- Model mounted providers, search paths, precedence, and reusable raw-source cache entries.
- Apply declared search-path and archive precedence.
- Return exact source bytes with provenance, or report every exact location checked.

## Non-Responsibilities

- Discovering installations, scanning filesystems, or broadening failed lookups.
- Parsing, compiling, or publishing resource bytes.
- Storing compiled playsrc artifacts.

## Relationships

Defines the content-provider interface used by directory, VPK, and BSP PAK adapters; supplies raw bytes to format and semantic packages. `asset-store` owns compiled output at the opposite end of the build.

## Completion

Complete when configured Source content resolves deterministically by logical path with exact precedence and failure reporting.
