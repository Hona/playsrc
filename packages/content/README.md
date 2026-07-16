# Content

## Sample

```ts
import { createContent, directory } from "@playsrc/content"

const content = createContent([directory(tf2Dir)])
const bytes = await content.read("materials/concrete/concretefloor001a.vmt")
```

```rust
let content = playsrc_content::Content::new([playsrc_content::directory(tf2_dir)]);
let bytes = content.read("materials/concrete/concretefloor001a.vmt")?;
```

## Objective

Resolve exact Source logical paths across explicitly configured content providers.

## Responsibilities

- Model mounted providers, search paths, precedence, and reusable raw-source cache entries.
- Apply declared search-path and archive precedence.
- Return exact source bytes with provenance, or report every exact location checked.
- Verify declared HTTPS download sources and retain encoded and decoded bytes as SHA-256-addressed raw-source cache objects.
- Read immutable remote BSP and VPK objects through exact HTTP ranges without extracting archive trees.

## Non-Responsibilities

- Discovering installations, scanning filesystems, or broadening failed lookups.
- Parsing, compiling, or publishing resource bytes.
- Deciding derived-cache identity or storing canonical/renderer output.

## Relationships

Defines the content-provider interface used by directory, HTTP object, VPK, and BSP PAK adapters; supplies exact raw bytes to native and WASM format/semantic compilers. `asset-store` may publish the same immutable source bytes and verified derived-cache objects without changing Content precedence.

## Completion

Complete when configured Source content resolves deterministically by logical path with exact precedence and failure reporting.
