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

`bun run verify:browser jump_beef` starts at the configured Main Menu, loads gameplay through Console, retains world, HUD, Options, mobile-VGUI, and active-PCF captures, byte-compares cold/warm ceiling, forward-wall, and floor regions while allowing authored viewmodel/Particle time to advance, and probes the 867-entry Source closure, Simulation performance, 122 brush records, PSMP v3/PMPO v4 models, alpha/marks, random/audio, Collision/movers, Particle timelines, first-gesture audio, unsupported sticky atomicity, replacement, and shutdown. `verify:tf2-wasm` additionally fixes stock/Original launch, held cadence, configured brush solidity, and above/below/intersection Water plans.

`bun run profile:gameui` starts a fresh checked `jump_beef` development owner and captures Main Menu startup, 15 requested seconds of MainMenu steady state, cold standard Options opening, and five requested seconds of visible Options steady state. It replaces `sourceCacheDir/profiles/gameui/jump_beef` with `report.json`, one combined Playwright trace, phase-separated CDP timelines and V8 CPU profiles, and MainMenu/Options screenshots. The report separates RAF intervals/callback costs, long tasks, event-loop lag, DOM cardinality/mutation rates and targets, heap, CDP task/script/style/layout/paint metrics, module/function/caller/stack CPU costs, renderer-main-thread trace functions, and per-runtime frame-work reasons. It exits nonzero after writing artifacts when any frame callback reaches the five-millisecond budget.

## Non-Responsibilities

- Implementing parsers, compilers, gameplay, presentation, or asset-store semantics.
- Accepting routine machine paths or stable build policy as command arguments.
- Discovering installations or searching the machine.

## Completion

Complete when every repeated supported operation has one deterministic command independent of the caller's working directory.
