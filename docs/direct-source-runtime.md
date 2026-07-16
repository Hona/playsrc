# Direct Source Runtime

This document defines how playsrc loads Source 1 maps and game content without requiring a prebuilt playsrc map package.

## Runtime Input

A playable map begins with one `MapSource`:

```text
game
content build
map logical identity
BSP byte identity
BSP byte length
BSP acquisition descriptor
additional declared content providers
```

The BSP acquisition descriptor is exactly one of:

- An immutable asset-store object descriptor.
- A declared HTTPS URL with expected byte length and SHA-256.
- A configured local Content provider and exact logical path.
- A browser-selected file with computed SHA-256 and explicit game/content-build selection.

A catalog may list any number of `MapSource` records. A map does not require prior native conversion, a checked generated package, or a pre-existing derived root.

## Content Providers

One immutable Content plan resolves Source logical paths in this order:

1. The active BSP PAK.
2. The optional map-supplement atlas declared by the selected map source.
3. The selected game content-build providers in declared search-path order.

Accepted provider kinds are:

- Configured local directory.
- Local or remote VPK directory index plus immutable random-access segment readers.
- Active BSP PAK index plus immutable random-access BSP reader.
- Raw-source cache object tied to its originating provider identity.
- Immutable HTTP object with byte-range support and verified SHA-256.

The VPK parser reads the directory index once. Each entry resolves to preload bytes plus an exact archive segment, offset, length, and CRC. Remote segment reads use HTTP byte ranges; the browser never downloads an entire VPK unless requested ranges cover the entire object.

The BSP parser reads the complete local/file input or exact HTTP ranges selected by the compiler. The embedded PAK remains inside the BSP object and is indexed directly. It is never extracted into a map directory.

## On-Demand Dependency Graph

The worker grows one transitive graph from the BSP instead of downloading a precomputed map archive:

```text
BSP lumps and entities
    -> map materials
        -> VMT includes, textures, proxies, and dependent materials
            -> selected VTF subresources
    -> static, dynamic, and entity models
        -> MDL/VVD/VTX/ANI companions
        -> model materials and skins
        -> PHY collision assets
    -> particle systems
        -> child systems, materials, models, and sprite sheets
    -> sound events and soundscapes
        -> selected sound assets
    -> scripts and game-owned resources
```

Each graph edge records the requesting object, logical path, provider, byte identity, and semantic role. The loader requests only objects reachable from the selected map and current game/ruleset/application configuration.

Official TF2, CS:S, HL2, or another Source 1 content build can be published as one source root containing unchanged VPK directory and segment objects. Publishing a content build does not claim gameplay support for that game; it makes its exact logical content available to format and tooling consumers.

Custom external content that is not packed into the BSP may be published as an optional map-supplement atlas. The atlas maps exact logical paths to immutable source objects. It never overrides the BSP PAK and never changes the selected game's VPK order. When no atlas is declared, the provider is Intentionally inert. When an atlas declares a logical path whose object is missing or corrupt, resolution fails at that provider instead of substituting a lower-priority game asset.

## Compilation

Source-compiled bytes remain authoritative inputs. Runtime consumers do not operate on unsafe raw offsets. The shared Rust compiler transforms verified Source bytes into canonical playsrc representations:

```text
BSP bytes
VPK/PAK-resolved dependencies
compiler behavior identity
build configuration identity
    -> CanonicalMap
    -> CollisionWorld
    -> VisibilityWorld
    -> EntityGraph
    -> semantic materials and models
    -> direct renderer upload data
```

The same Rust crates execute:

- Natively in tools and future servers.
- As coarse WASM worker operations in browsers.

Native and WASM output must be byte-identical for invariant serialized representations. GPU handles, DOM nodes, browser audio nodes, and process-local handles are excluded from this equality because they are presentation resources, not canonical data.

[`native-wasm-contract.md`](native-wasm-contract.md) defines the batched dependency-request protocol, worker boundary, packed data representations, handle ownership, cancellation, and performance evidence.

## Derived Cache Identity

Every reproducible derived object uses this identity:

```text
source object hashes
transitive dependency hashes
compiler behavior identity
build configuration identity
output role
```

The complete tuple is hashed with SHA-256. A cache hit is accepted only when the descriptor and exact bytes verify against that identity. Corrupt, incomplete, mismatched, or stale entries fail validation and are recomputed by the same compiler implementation.

Cache lookup followed by deterministic computation on a miss is one implementation path. It is not a fallback implementation.

## Storage

The local Source cache stores acquired raw Source bytes and download intermediates for developer workflows. It is disposable.

The global asset store may contain both:

- Exact immutable Source objects intentionally published for runtime access: BSP files, VPK directory files, VPK segment files, and declared additional content objects.
- Immutable derived objects produced by the current compiler.

Every object descriptor records representation kind, media type, byte length, SHA-256, and provenance. Raw and derived representations are different objects even when they represent related content.

The browser stores verified raw HTTP responses in the HTTP cache and stores verified derived objects in IndexedDB. Neither cache is authority. Eviction causes exact refetch or deterministic recomputation.

## Map Runtime Descriptor

After compilation, the worker emits one `MapRuntimeDescriptor` containing:

- `MapSource` identity.
- Every consumed raw object hash.
- Compiler and configuration identities.
- Canonical map object identities.
- Collision, visibility, entity, material, model, particle, audio, and renderer-input identities.
- Coverage classifications and diagnostics.

The descriptor may be published as an immutable derived cache object. It is generated automatically and never required before the first load.

## Server-Side Population

Native tools may precompute and publish the exact derived objects that a browser would compute. The browser verifies the same cache identities before use. Server population improves first-load latency; it does not create another compiler, another contract, or a required manual map-processing gate.

## Map Availability

A map is loadable when:

- Its BSP bytes resolve and match their declared identity.
- Every required external logical path resolves through its declared providers.
- Every required format and semantic behavior is Handled or Intentionally inert.
- Compilation remains within accepted bounds.

A map is Missing when a required external dependency is absent from its BSP PAK, selected game content build, and additional declared providers. playsrc reports the exact missing logical paths. It never scans for substitutes or selects fallback assets.

## GLB Export

GLB remains a useful independently consumable export format. [`bsp-to-glb`](https://github.com/Hona/bsp-to-glb) remains a dedicated tool for workflows that require GLB.

The playsrc runtime does not serialize a map to GLB before play. Three.js receives direct renderer data from the canonical compiler output. GLB cannot own BSP topology, PVS, collision, entities, lightmaps, Source materials, model bodygroups, game state, or runtime authority.
