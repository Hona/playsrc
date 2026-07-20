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

`bun run dev jump_beef` verifies/acquires the declared BSP, concurrently builds the checked TF2 WASM and native source-bundle targets, and reuses PSDB/PUIB/ledger generation only when one cache report matches the exact generator executable and declared artifact sizes. It concurrently installs the independently hashed BSP, WASM, PSDB, PUIB, and ledger objects in `assetDir`, starts the loopback Asset Service on port 4174 and TF2 Vite application on port 4173, waits for both readiness endpoints, reports command-to-ready milliseconds on standard error, prints `http://127.0.0.1:4173/`, and closes both owned listeners after one `SIGINT` or `SIGTERM`. Cache misses package exact raw dependencies through the configured build-`24207079` GAME provider plan and emit the complete provenance/absence ledger.

`bun run verify:browser jump_beef` starts at the configured Main Menu, loads gameplay through Console, retains world, HUD, Options, mobile-VGUI, and active-PCF captures, byte-compares cold/warm ceiling, forward-wall, and floor regions while allowing authored viewmodel/Particle time to advance, and probes the 867-entry Source closure, Simulation performance, 122 brush records, PSMP v3/PMPO v4 models, alpha/marks, random/audio, Collision/movers, Particle timelines, first-gesture audio, unsupported sticky atomicity, replacement, and shutdown. `verify:tf2-wasm` additionally fixes stock/Original launch, held cadence, configured brush solidity, PVS/frustum separation, and above/below/intersection Water plans. The checked Rust 1.97.1 fixture fixes LDR payload SHA-256 `56153098a867c553651f9c773bd72c4659782bae8520277c80daaaa414bdf156`, LDR derived identity `0c46669280c6ad49d32416d5246caca940d450fe57b6ae6f932c98723bb5aff8`, HDR payload SHA-256 `d68ce8b744648715d3224f5bc71beb2bea8b592bdc722098c432a2131abc75ce`, and HDR derived identity `f772a27b1c97b2659712f5fa0873fe9c2f3aed2a8999c814674e2cdb2da02b86` through byte-identical native/WASM comparison.

`bun run profile:gameui` starts a fresh checked `jump_beef` development owner and captures Main Menu startup, 15 requested seconds of MainMenu steady state, cold standard Options opening, and five requested seconds of visible Options steady state. It replaces `sourceCacheDir/profiles/gameui/jump_beef` with `report.json`, one combined Playwright trace, phase-separated CDP timelines and V8 CPU profiles, and MainMenu/Options screenshots. The report separates RAF intervals/callback costs, long tasks, event-loop lag, DOM cardinality/mutation rates and targets, heap, CDP task/script/style/layout/paint metrics, module/function/caller/stack CPU costs, renderer-main-thread trace functions, and per-runtime frame-work reasons. It exits nonzero after writing artifacts when any frame callback reaches the five-millisecond budget.

`bun run profile:hud` starts the same checked development owner, loads `jump_beef` through Console, and replaces `sourceCacheDir/profiles/hud/jump_beef` with one geometry report plus screenshots at 1,280×720, 1,024×768, 1,600×900, and restored 1,280×720. It requires the configured class, health, ammo, weapon-selection, and crosshair rectangles to follow exact TF2 proportional viewport math through every resize.

`bun run profile:gameplay` keeps Chromium visible without acquiring the host pointer, loads `jump_beef` through Console, and measures key-down/up/fire response, continuous look, repeated jump, held movement, held fire, Simulation and presentation rates, worker transactions, main-thread gaps, and per-frame model/visibility/Particle/audio/dynamic/world/viewmodel/render phases. `bun run profile:map-load` runs the same visible cursor-safe path through one cold and one same-session repeated `map jump_beef`, separating WASM input copy, BSP/canonical/material/entity/presentation/model/Particle/runtime/collision/game compilation, scene activation, and cache reuse.

`bun run profile:startup` starts the real `jump_beef` development owner, records map verification, concurrent build/cache, parallel immutable publication, inline Vite creation, listener readiness, total milliseconds, and sampled parent-process memory, writes `sourceCacheDir/profiles/startup/jump_beef/report.json`, then closes both ports. Equivalent isolated worktrees on the same Windows host measured 189,402 ms baseline versus 50,531 ms candidate from empty Cargo targets, and 104,948 ms baseline versus 1,021 ms candidate with warm Cargo outputs.

## Non-Responsibilities

- Implementing parsers, compilers, gameplay, presentation, or asset-store semantics.
- Accepting routine machine paths or stable build policy as command arguments.
- Discovering installations or searching the machine.

## Completion

Complete when every repeated supported operation has one deterministic command independent of the caller's working directory.
