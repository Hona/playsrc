# VPK

## Sample

```ts
import { openVpk } from "@playsrc/vpk"

const archive = await openVpk(file)
const bytes = await archive.read("materials/example.vmt")
```

```rust
let archive = playsrc_vpk::Vpk::open("pak01_dir.vpk")?;
let bytes = archive.read("materials/example.vmt")?;
```

## Objective

Index and read Source 1 VPK archives by exact logical path.

## Responsibilities

- Parse archive directories and entry metadata with explicit bounds.
- Read exact entry bytes from the correct archive segment.
- Validate integrity data and report missing, malformed, and unsupported entries.

## Non-Responsibilities

- Choosing mount precedence across multiple content providers.
- Parsing the resources stored inside an archive.
- Discovering game installations or searching the filesystem.

## Relationships

Provides a VPK-backed content provider to `content` while remaining independently usable as an archive package.

## Completion

Complete when the declared VPK format family and access behavior are supported with bounded archive evidence.
