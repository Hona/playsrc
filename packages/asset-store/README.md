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

Store and publish exact immutable Source objects and reproducible playsrc-derived objects without duplicating shared bytes.

## Responsibilities

- Address exact object bytes by content hash and store each immutable object once.
- Preserve representation kind and provenance for raw BSP, VPK directory, VPK segment, and additional Source objects.
- Represent immutable source, map-runtime, game, and application descriptors plus catalogs and mutable channels.
- Validate reachability and synchronize missing objects to remote storage.

## Non-Responsibilities

- Resolving Source logical paths or interpreting Source formats.
- Compiling maps, materials, models, or gameplay data.
- Provisioning CDN infrastructure or defining product catalogs.

## Relationships

Receives validated raw and derived objects, serves applications through the asset application, and uses infrastructure adapters for remote publication. A derived object is a cache hit only when its complete source/dependency/compiler/configuration/role identity verifies.

## Completion

Complete when object identity, roots, catalogs, channels, reachability, publication, and rollback invariants are implemented and verified.
