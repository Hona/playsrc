# playsrc Tool

## Objective

Provide one stable command interface for repeatable playsrc development, compilation, verification, and release operations.

## Responsibilities

- Validate local configuration before work begins.
- Resolve the repository-root `playsrc.local.json` independently of the caller's working directory and require exactly three accessible, absolute, distinct, non-nested roots.
- Resolve logical targets, call package interfaces, own child processes, and report exact results.
- Expose short commands for setup, direct-source compilation, development, verification, cache population, GLB export, publication, and release.

`bun run setup` verifies the checked Bun identity and installs the checked Rust toolchain, rustfmt, Clippy, and `wasm32-unknown-unknown` standard library under `sourceCacheDir`. It never changes a user-global Rust installation or shell path.

Map commands accept one checked logical target. `jump_beef` resolves to its exact TF2 map registry entry and verified raw-source cache objects; aliases and path arguments are rejected.

Native compilation and browser WASM compilation call the same owning Rust crates and use the same derived-cache identity. The tool can prepopulate caches but cannot make native preprocessing a browser gameplay prerequisite.

`bun run dev jump_beef` verifies/acquires the declared BSP, packages exact raw VMT/VTF dependencies from configured TF2/HL2 VPK providers, builds the checked TF2 WASM target, installs all three exact objects in `assetDir`, starts the loopback Asset Service on port 4174 and TF2 Vite application on port 4173, waits for both readiness endpoints, prints `http://127.0.0.1:4173/`, and closes both owned listeners after one `SIGINT` or `SIGTERM`.

`bun run verify:browser jump_beef` starts the exact `bun run dev jump_beef` command plus one fresh headed WebGPU browser session and checks cold compilation, warm reuse, IndexedDB identity, the exact selected teamspawn and settled camera, retained 1,280×720 floor/ceiling/wall canvas regions, signed horizontal/vertical pointer direction, exact Audio decode/resume, pointer lock, movement, jump, Soldier/Demoman combat, VGUI history/completion/focus/repeated visibility, `map jump_beef` generation replacement, browser shutdown, one `SIGINT`, child exit code zero, and listener release. It emits one JSON result.

## Non-Responsibilities

- Implementing parsers, compilers, gameplay, presentation, or asset-store semantics.
- Accepting routine machine paths or stable build policy as command arguments.
- Discovering installations or searching the machine.

## Completion

Complete when every repeated supported operation has one deterministic command independent of the caller's working directory.
