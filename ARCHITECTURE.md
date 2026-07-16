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
submit commands
advance ticks
read snapshot
```

Fine-grained domain calls remain inside the runtime implementation.

## Canonical Representations

Source formats are parsed before semantic and runtime interpretation:

```text
BSP              -> map and world representations
VMT + VTF        -> material representation
MDL/VVD/VTX/ANI  -> model representation
entity data      -> entity graph and game entities
demo records     -> replay state
```

Transport formats such as GLB may be derived outputs but never become semantic authority.

## Content And Assets

Raw Source content and compiled playsrc assets are separate mental models:

```text
configured Source roots
    -> content logical-path resolution
    -> format parsing and compilation
    -> immutable asset-store objects and roots
    -> asset application and CDN
```

Local configuration identifies exactly three roots:

```json
{
  "tf2Dir": "...",
  "sourceCacheDir": "...",
  "assetDir": "..."
}
```

- The Source cache contains reusable raw inputs and is never deployed.
- Per-job work directories are temporary and owned by one process tree.
- The asset store contains durable content-addressed output consumed locally and mirrored remotely.
- Map packages are manifests referencing shared immutable objects rather than archives duplicating global assets.

Content resolution uses configured roots, exact logical paths, and known archive indexes. It never discovers installations or scans the machine.

## Tools And Infrastructure

```text
tools/
|-- playsrc/
`-- inspector/

infra/
```

The playsrc tool exposes repeatable setup, compilation, development, verification, publication, and release commands over module interfaces. The inspector provides interactive diagnostics without becoming an alternate authority.

Infrastructure defines hosting resources and environments. Application-specific deployment configuration remains with its application. Compilation and deployment remain separate operations.

## Development

The intended common workflow remains one configuration-driven command such as:

```bash
bun run dev jump_beef
```

The command validates local configuration, resolves exact inputs, builds stale outputs, validates them, publishes immutable local assets, starts owned processes, and prints the exact application URL. One interrupt stops the owned process tree.

## Deployment

Deployment validates the prepared local asset store, uploads missing immutable objects and roots, deploys applications, and atomically changes a channel pointer. Rollback changes the pointer rather than rebuilding or mutating published objects.

Raw Source inputs and temporary work are never deployed.

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
