# VPK

## Sample

```rust
let archive = playsrc_vpk::parse(
    &directory_bytes,
    "pak01_dir.vpk",
    playsrc_vpk::Layout::Split,
    playsrc_vpk::Limits::default(),
)?;
let result = archive.read_entry("materials/example.vmt", &segment_reader)?;
```

## Objective

Index and read Source 1 VPK archives by exact logical path.

## Responsibilities

- Parse archive directories and entry metadata with explicit bounds.
- Retain version 1/2 section ranges, tree order, stored components, canonical lookup identities, preload ranges, data descriptors, archive-MD5 records, self-MD5 values, and signature material.
- Read full entries or entry-relative ranges across preload, directory-contained data, and the one exact numeric segment requested through a positioned reader.
- Verify full-entry CRC-32, individual archive-range MD5, all three directory-file MD5 values, and RSA PKCS#1 v1.5 SHA-256 signatures, including an optional expected-key check.
- Report missing, malformed, unsupported, changed, short, corrupt, and over-limit operations without extraction or fallback.

## Non-Responsibilities

- Choosing mount precedence across multiple content providers.
- Parsing the resources stored inside an archive.
- Discovering game installations or searching the filesystem.

## Relationships

Provides a VPK-backed content provider to `content` while remaining independently usable as an archive package.

## Completion

Complete when the declared VPK format family and access behavior are supported with bounded archive evidence.
