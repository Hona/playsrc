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

`bun run dev jump_beef` verifies/acquires the declared BSP, packages exact raw dependencies through the configured build-`24207079` GAME provider plan, emits the complete provenance/absence ledger, builds the checked TF2 WASM target, installs the BSP, WASM, PSDB, and ledger objects in `assetDir`, starts the loopback Asset Service on port 4174 and TF2 Vite application on port 4173, waits for both readiness endpoints, prints `http://127.0.0.1:4173/`, and closes both owned listeners after one `SIGINT` or `SIGTERM`.

`bun run verify:browser jump_beef` additionally retains fixed world and active-PCF captures; byte-compares cold/warm world regions; probes decal alpha/PVS, 33 model matrices, Soldier/Demoman/player and six viewmodel timelines, separate viewmodel projection/depth, smooth crouch/view trajectories, complete Movement and TF2 weapon/activity/Entity facts, textured Particle sprites/trails, and Source Audio definition/channel/level lifecycle. It emits every support diagnostic and partitions it against the immutable dependency ledger: content blockers must be zero, the fixed content-closure partition must remain 15 behavior plus one platform blocker, and IVP, Tempus, sticky-random, rocket-Collision, and mover-result gaps remain separate authority blockers.

## Non-Responsibilities

- Implementing parsers, compilers, gameplay, presentation, or asset-store semantics.
- Accepting routine machine paths or stable build policy as command arguments.
- Discovering installations or searching the machine.

## Completion

Complete when every repeated supported operation has one deterministic command independent of the caller's working directory.
