# Content

## Sample

```rust
let content = playsrc_content::Content::open(game, content_build, providers, limits)?;
let map = content.resolve_map("maps/jump_beef.bsp")?;
let content = content.with_active_pak(pak_id, bsp_sha256, map_path, &pak)?;
let resource = content.resolve_resource("materials/concrete/concretefloor001a.vmt")?;
```

## Objective

Resolve exact Source logical paths across explicitly configured content providers.

## Responsibilities

- Model mounted providers, search paths, precedence, and reusable raw-source cache entries.
- Apply declared search-path and archive precedence.
- Return exact source bytes with provenance, or report every exact location checked.
- Verify declared HTTPS download sources and retain encoded and decoded bytes as SHA-256-addressed raw-source cache objects.
- Read immutable remote BSP and VPK objects through exact HTTP ranges without extracting archive trees.
- Resolve map BSPs only through declared external providers, then resolve resources through the active BSP PAK, optional map-supplement atlas, and game providers in that order.
- Accept verified VPK directory bytes and a thread-safe immutable segment-range adapter so native files and browser HTTP ranges use one Rust resolver and VPK parser.
- Package exact map-selected raw VMT/VTF dependencies from the active BSP PAK and the configured `gameinfo.txt` TF2/HL2 VPK order for coarse browser-WASM transfer without decoding or extracting archive trees.

## Non-Responsibilities

- Discovering installations, scanning filesystems, or broadening failed lookups.
- Parsing, compiling, or publishing resource bytes.
- Deciding derived-cache identity or storing canonical/renderer output.

## Relationships

Defines the content-provider interface used by directory, HTTP object, VPK, and BSP PAK adapters; supplies exact raw bytes to native and WASM format/semantic compilers. `asset-store` may publish the same immutable source bytes and verified derived-cache objects without changing Content precedence.

## Completion

Complete when configured Source content resolves deterministically by logical path with exact precedence and failure reporting.
