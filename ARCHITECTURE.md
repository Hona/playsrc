# Architecture

## Shape

`playsrc` is one domain-first monorepo. Rust, WASM, TypeScript, browser code, and shaders live inside the modules whose behavior they implement.

```text
format primitives
    -> semantic representations
    -> compiler and runtime modules
    -> game adapters
    -> rulesets
    -> applications
```

Dependencies point downward only. Parsers do not depend on renderers or games. Generic Source packages do not contain TF2-specific behavior. Applications assemble modules and do not reimplement engine behavior.

## Responsibilities

- Canonical binary parsing and deterministic gameplay belong in native cores that can target native and WASM environments.
- Browser fetching, GPU resources, presentation, input, and UI belong near browser APIs.
- Rendering presents gameplay or replay state and never owns either authority.
- Game differences belong in game adapters.
- Mode differences belong in rulesets.

## Canonical Representations

Raw Source formats compile into playsrc-owned representations before runtime-specific output.

```text
BSP -> map representation -> map package
VMT/VTF -> material representation -> GPU material
MDL/VVD/VTX -> model representation -> GPU model
entity lump -> entity graph -> gameplay state
```

Transport formats such as GLB are derived artifacts, not semantic authority.

## Storage

Local configuration identifies three roots:

```json
{
  "tf2Dir": "...",
  "sourceCacheDir": "...",
  "assetDir": "..."
}
```

- The Source cache stores reusable raw inputs and is never deployed.
- Per-job work directories are temporary and owned by one process tree.
- The asset store contains durable content-addressed outputs consumed locally and mirrored to the CDN.
- Map packages are manifests referencing shared immutable objects; they do not embed duplicate global assets.

## Development

The intended workflow is:

```bash
bun run dev jump_beef
```

Development resolves exact inputs, rebuilds only stale outputs, validates the map package, updates the local catalog, and starts the owned asset and application servers. Conversion is a dependency of development, not a user-selected mode.

## Deployment

Compilation and deployment are separate. Deployment uploads missing immutable objects and roots, deploys applications, and atomically changes a small channel pointer. Raw sources and temporary work are never deployed.

## Applications

Planned applications are served from subdomains of `playsrc.online`:

```text
tf2.playsrc.online
css.playsrc.online
csgo.playsrc.online
tempus.playsrc.online
assets.playsrc.online
api.playsrc.online
servers.playsrc.online
```
