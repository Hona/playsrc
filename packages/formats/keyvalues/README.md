# KeyValues

## Sample

```ts
import { parseKeyValues } from "@playsrc/keyvalues"

const document = parseKeyValues(text)
```

```rust
let document = playsrc_keyvalues::parse_text(
    bytes,
    playsrc_keyvalues::EscapeMode::LiteralBackslash,
    playsrc_keyvalues::Limits::default(),
)?;
```

## Objective

Provide a bounded representation and parser for the Source 1 KeyValues format family.

## Responsibilities

- Parse keys, values, nested objects, repeated keys, and format-level directives.
- Preserve ordering and distinctions required by consuming Source formats.
- Expose explicit malformed and unsupported input states.
- Retain original byte spans, ordered repeated keys, scalar inference, conditions, directives, and byte-exact unmodified text.

## Non-Responsibilities

- VMT shader semantics, entity behavior, or game configuration policy.
- Resolving logical paths through mounted content.

## Relationships

VMT, material, particle, audio, and game modules may build domain semantics over KeyValues data.

## Completion

Complete when the declared KeyValues behavior family is bounded, represented without silent loss, and verified by fixed inputs, expected observable outputs, and declared comparison methods.

The current Rust crate implements bounded text syntax and evaluation. Include/base composition, native binary KV1, canonical text serialization, and KVPacker remain unimplemented and explicit in the roadmap.
