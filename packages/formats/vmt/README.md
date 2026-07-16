# VMT

## Sample

```ts
import { parseVmt } from "@playsrc/vmt"

const document = parseVmt(text)
```

```rust
let document = playsrc_vmt::parse(text)?;
```

## Objective

Parse Source 1 VMT material documents without conflating document syntax with runtime material behavior.

## Responsibilities

- Represent shader names, parameters, proxy declarations, and document-level composition.
- Resolve referenced VMT documents through an explicit content interface.
- Preserve unknown parameters and structures for semantic classification.

## Non-Responsibilities

- Implementing shader or material-proxy behavior.
- Decoding VTF images.
- Creating GPU materials.

## Relationships

Builds on `keyvalues`, resolves documents through `content`, and supplies parsed material documents to `material`.

## Completion

Complete when the declared VMT document behavior is represented without silently treating unknown material semantics as handled.
