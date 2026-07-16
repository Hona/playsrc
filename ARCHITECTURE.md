# Architecture

## Mental Models

playsrc is organized so one Source behavior family can be understood, implemented, and verified without loading the whole engine into working memory.

```text
packages/     reusable Source mental models
games/        behavior belonging to one game
apps/         deployed product and network applications
tools/        programs run by developers and operators
infra/        resources on which applications run
docs/         cross-cutting knowledge with no single module owner
```

Every leaf module defines one objective and one interface. Its README states what it owns and, equally importantly, what it does not own.

## Package Organization

```text
packages/
|-- formats/
|   |-- keyvalues/
|   |-- bsp/
|   |-- vpk/
|   |-- vtf/
|   |-- vmt/
|   |-- studio-model/
|   |-- phy/
|   `-- demo/
|-- world/
|   |-- map/
|   |-- material/
|   |-- entity/
|   |-- collision/
|   `-- visibility/
|-- runtime/
|   |-- movement/
|   |-- physics/
|   |-- simulation/
|   |-- networking/
|   `-- replay/
|-- presentation/
|   |-- rendering/
|   |-- particle/
|   |-- audio/
|   `-- vgui/
|-- content/
`-- asset-store/
```

Group directories are navigational only. They contain no implementation and expose no interface. Rust, WASM, TypeScript, browser integration, and shaders live inside the modules whose behavior they implement.

## Composition

The mental models compose in one direction:

```text
format data
    -> world semantics
    -> runtime behavior
    -> game behavior
    -> game-owned ruleset
    -> application

world, gameplay, or replay state
    -> presentation
    -> application
```

Lower modules do not know which game, ruleset, or product consumes them. Applications may assemble several branches, but they do not become behavior owners.

For example:

```text
Tempus application
    -> TF2 jump ruleset
        -> TF2 game behavior
            -> generic movement and simulation
                -> collision and map state
```

Someone working on rocket behavior can remain in TF2 weapons, projectiles, damage, and their immediate runtime seams. They do not need Tempus records, VPK indexing, deployment, or UI knowledge.

## Games And Rulesets

Each game owns behavior that varies by title:

```text
games/
|-- tf2/
|   `-- rulesets/
|-- css/
|   `-- rulesets/
`-- csgo/
    `-- rulesets/
```

Games own classes, items, weapons, movement differences, entities, objectives, replicated state, game rules, and game-specific presentation mappings.

Rulesets belong to a game. A CS:S surf ruleset and legacy CS:GO surf ruleset are separate modules even when they share a mode name. Reusable mechanics move into generic packages only when they are genuinely game-independent.

## Applications

All deployed programs live under `apps`:

```text
apps/
|-- web/
|   |-- tf2/
|   |-- css/
|   |-- csgo/
|   `-- tempus/
`-- services/
    |-- api/
    |-- assets/
    |-- matchmaking/
    `-- game-servers/
```

Web applications own browser and product integration. Service applications own network process and transport integration. Both consume public module interfaces and remain thin behavior assemblers.

## Authority

- Simulation is the single authority that advances gameplay state.
- The same deterministic game behavior is shared by browser prediction and future native server authority.
- Replay state is extracted from recorded data without becoming another gameplay simulation.
- Rendering, particles, and audio present gameplay or replay state and cannot modify authority.

Native and WASM interfaces stay coarse:

```text
compiler.beginMap(mapSource)
compiler.supplyObjects(objectBatch)
compiler.advance(workBudget)
compiler.readResult()

simulation.submitCommands(commandBatch)
simulation.advance(tickCount)
simulation.readSnapshot()
```

Call count cannot scale with faces, traces, entities, particles, archive entries, or another unbounded content cardinality. Fine-grained domain calls remain inside Rust. [`docs/native-wasm-contract.md`](docs/native-wasm-contract.md) defines worker topology, data representation, memory ownership, cancellation, and evidence.

## Canonical Representations

Source formats are parsed before semantic and runtime interpretation:

```text
BSP              -> map and world representations
VMT + VTF        -> material representation
MDL/VVD/VTX/ANI  -> model representation
entity data      -> entity graph and game entities
demo records     -> replay state
```

Transport formats such as GLB are optional exports. The runtime compiles direct renderer, collision, visibility, entity, and simulation inputs without serializing through GLB.

## Direct Source Runtime

Declared Source bytes are runtime inputs:

```text
MapSource
    -> BSP bytes
    -> active BSP PAK
    -> selected game VPK directory and segment bytes
    -> additional declared providers
    -> Rust compiler in a native process or browser WASM worker
    -> canonical map, collision, visibility, entity, material, and model data
    -> direct simulation and Three.js inputs
```

A map catalog records a BSP acquisition descriptor and exact content identities. It never requires a pre-existing GLB or derived map root. The first browser load compiles missing derived objects; later loads reuse only objects whose complete derived cache keys verify.

[`docs/direct-source-runtime.md`](docs/direct-source-runtime.md) defines source providers, compiler identity, derived cache identity, browser storage, server-side cache population, and map availability.

## Content And Assets

Content resolves exact logical paths against active BSP PAK, declared map providers, and selected game content-build providers. Local and remote VPK providers parse the official directory file and read exact entry ranges from unchanged VPK segment objects.

Local configuration identifies exactly three roots:

```json
{
  "tf2Dir": "...",
  "sourceCacheDir": "...",
  "assetDir": "..."
}
```

- The Source cache contains reusable local acquisition inputs and download intermediates. It is disposable and never referenced by published descriptors.
- Per-job work directories are temporary and owned by one process tree.
- The asset store contains immutable raw Source objects intentionally published for runtime access and immutable derived objects produced by the current compiler.
- Raw BSP, VPK directory, and VPK segment objects retain their original bytes and provenance.
- Derived objects record source hashes, transitive dependency hashes, compiler behavior, build configuration, and output role.
- Map packages are optional generated descriptors referencing shared immutable raw and derived objects. They are caches and publication units, not first-load prerequisites.
- Browser HTTP cache stores verified source responses; IndexedDB stores verified derived objects. Eviction causes exact refetch or deterministic recompilation.

Content resolution uses configured roots, declared URLs, exact logical paths, and known archive indexes. It never discovers installations or scans the machine.

## Tools And Infrastructure

```text
tools/
|-- playsrc/
`-- inspector/

infra/
```

The playsrc tool exposes repeatable setup, direct-source compilation, export, development, verification, cache population, publication, and release commands over module interfaces. The inspector provides interactive diagnostics without becoming an alternate authority.

Infrastructure defines hosting resources and environments. Application-specific deployment configuration remains with its application. Compilation and deployment remain separate operations.

## Development

The intended common workflow remains one configuration-driven command such as:

```bash
bun run dev jump_beef
```

The command validates local configuration, resolves the exact map source and game content build, compiles missing derived objects through the same Rust implementation used by browser WASM, validates them, populates the local cache, starts owned processes, and prints the exact application URL. One interrupt stops the owned process tree.

Browser products can also load a declared BSP source directly. The worker verifies source bytes, resolves PAK/VPK dependencies, compiles missing runtime data, stores verified derived objects in IndexedDB, and starts the experience without a prior native conversion operation.

## Deployment

Deployment may publish exact raw BSP/VPK objects and precomputed derived objects to reduce first-load latency. It validates and uploads missing immutable objects and descriptors, deploys applications, and atomically changes a channel pointer. Rollback changes the pointer rather than rebuilding or mutating published objects.

Temporary work and undeclared local Source inputs are never deployed. Every published raw Source object is an explicit descriptor member with exact provenance and SHA-256.

## Domains

Planned applications and shared services use subdomains of `playsrc.online`:

```text
tf2.playsrc.online
css.playsrc.online
csgo.playsrc.online
tempus.playsrc.online
assets.playsrc.online
api.playsrc.online
servers.playsrc.online
```
