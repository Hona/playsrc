# Asset Store

## Sample

```ts
import { openAssetStore } from "@playsrc/asset-store"

const store = await openAssetStore(assetDir)
const hash = await store.put(bytes)
const stored = await store.get(hash)
```

```rust
let store = playsrc_asset_store::open(asset_dir)?;
let hash = store.put(&bytes)?;
let stored = store.get(hash)?;
```

## Objective

Store and publish immutable compiled playsrc output without duplicating shared objects.

## Responsibilities

- Address exact object bytes by content hash and store each immutable object once.
- Represent immutable map, game, and application roots plus catalogs and mutable channels.
- Validate reachability and synchronize missing objects to remote storage.

## Non-Responsibilities

- Resolving Source logical paths or interpreting Source formats.
- Compiling maps, materials, models, or gameplay data.
- Provisioning CDN infrastructure or defining product catalogs.

## Relationships

Receives validated compiler output, serves applications through the asset application, and uses infrastructure adapters for remote publication.

## Completion

Complete when object identity, roots, catalogs, channels, reachability, publication, and rollback invariants are implemented and verified.
