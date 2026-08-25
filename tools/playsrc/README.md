# playsrc Tool

## Objective

Provide one stable command interface for repeatable playsrc development, compilation, verification, and release operations.

## Responsibilities

- Validate local configuration before work begins.
- Resolve the repository-root `playsrc.local.json` independently of the caller's working directory and require exactly three accessible, absolute, distinct, non-nested roots.
- Resolve logical targets, call package interfaces, own child processes, and report exact results.
- Expose short commands for setup, direct-source compilation, development, verification, cache population, GLB export, publication, and release.

`bun run setup` verifies the checked Bun identity and installs the checked Rust toolchain, rustfmt, Clippy, and `wasm32-unknown-unknown` standard library under `sourceCacheDir`. It never changes a user-global Rust installation or shell path.

Map commands accept one checked logical target. `jump_beef` resolves to verified download-cache objects; `pl_upward` resolves exact configured build/provider bytes at `maps/pl_upward.bsp`. Aliases and path arguments are rejected.

Native compilation and browser WASM compilation call the same owning Rust crates and use the same derived-cache identity. The tool can prepopulate caches but cannot make native preprocessing a browser gameplay prerequisite.

`bun run dev jump_beef` verifies/acquires the declared BSP, concurrently builds checked TF2 WASM and native resource-graph targets, and reuses graph/chunk/ledger generation only when one cache report matches the exact generator executable and every declared artifact. It installs the independently hashed BSP, WASM, graph root, 144 chunks, and ledger in `assetDir`, starts the loopback Asset Service on port 4174 and TF2 Vite application on port 4173, waits for both readiness endpoints, prints `http://127.0.0.1:4173/`, and closes both listeners after one `SIGINT` or `SIGTERM`.

`bun run dev pl_upward` uses the same application and lifecycle with a target-qualified dynamic catalog, exact installed BSP, graph and ledger. Source-bundle generation resolves, parses, checksum/mesh/LOD joins and ledgers every profile-qualified static-prop VHV input, then packs the raw objects plus typed identities into one bounded gameplay aggregate. It retains Jump gameplay authority, admits existing noclip, and publishes no Payload gameplay state.

`bun run verify:displacement-visuals` selects configured sources 147, 381 and 138, derives above/front cameras from canonical bounds and parent orientation, and requires exact CPU winding/normals, PVS/frustum admission, batch submission, cull/depth/material/lightmap state and projected headed captures.

`bun run infra:publish` builds and locally verifies both declared BSPs, one shared WASM, one canonical two-target catalog, both resource roots, every reachable identity-deduplicated chunk, and both dependency ledgers; replaces the sole checked release descriptor; processes leaves before roots and catalog; conditionally creates missing R2 objects through S3 `If-None-Match: *`; verifies exact readback; and emits one bounded JSON report. A warm publication performs zero writes.

`bun run infra:deploy` never compiles Source content. It consumes the complete checked TF2 release descriptor without a map argument, applies the exact Terraform plan, requires both target closures at `assets.playsrc.online`, builds `/` plus `/tf2`, deploys the mirrored static tree through Wrangler, and requires production readiness. `bun run infra:verify` performs Terraform validation and a Wrangler dry run without changing Cloudflare.

`bun run test:rust` runs locked Rust `1.97.1` tests for the root workspace and every separately locked Rust workspace. Cargo's ignored configured-content evidence remains opt-in.

`bun run verify:browser jump_beef` captures exact startup first/middle/final frames, hidden-menu gesture admission, Escape skip, desktop/mobile loading VGUI, cold/warm world/HUD/Options/active-PCF behavior, and disconnect without startup replay. It byte-compares fixed world regions and probes the target-qualified Source closure, Simulation performance, 122 brush records, PSMP v3/PMPO v4 models, alpha/marks, random/audio, Collision/movers, Particle timelines, unsupported sticky atomicity, replacement, and shutdown. `verify:tf2-wasm` additionally fixes stock/Original launch, held cadence, configured brush solidity, PVS/frustum separation, and Water plans. The checked Rust 1.97.1 fixture fixes LDR payload SHA-256 `56153098a867c553651f9c773bd72c4659782bae8520277c80daaaa414bdf156`, LDR derived identity `23c6cd43e594a89e34514186dac3752c1ee31e497e48cebca412822c854228cd`, HDR payload SHA-256 `735995d68920adcb971fe4c5e773986f438c2a95c07c935882dc7fd081ce1e3a`, and HDR derived identity `5be4cb1aa68586dbf0d786fdd5a85928638baeec361fde60003f0b1f200e8758` through byte-identical native/WASM comparison.

`bun run verify:browser-switch` starts from the configured default and executes `jump_beef → pl_upward → jump_beef` through one Worker/WASM owner. Every generation must admit noclip movement for at least three fixed ticks and publish exact release identities plus 25 bounded final-Ready canvas-color/geometric-depth/source-face/world-batch/material samples joined to leaf, area, PVS, and draw-surface facts. Unknown aliases must add no resource request or state change; shutdown must release both listeners.

`bun run profile:gameui` starts a fresh checked `jump_beef` development owner and captures Main Menu startup, 15 requested seconds of MainMenu steady state, cold standard Options opening, and five requested seconds of visible Options steady state. It replaces `sourceCacheDir/profiles/gameui/jump_beef` with `report.json`, one combined Playwright trace, phase-separated CDP timelines and V8 CPU profiles, and MainMenu/Options screenshots. The report separates RAF intervals/callback costs, long tasks, event-loop lag, DOM cardinality/mutation rates and targets, heap, CDP task/script/style/layout/paint metrics, module/function/caller/stack CPU costs, renderer-main-thread trace functions, and per-runtime frame-work reasons. It exits nonzero after writing artifacts when any frame callback reaches the five-millisecond budget.

`bun run profile:hud` starts the same checked development owner, loads `jump_beef` through Console, and replaces `sourceCacheDir/profiles/hud/jump_beef` with one geometry report plus screenshots at 1,280×720, 1,024×768, 1,600×900, and restored 1,280×720. It requires the configured class, health, ammo, weapon-selection, and crosshair rectangles to follow exact TF2 proportional viewport math through every resize.

`bun run profile:gameplay` keeps Chromium visible without acquiring the host pointer, loads `jump_beef` through Console, and measures key-down/up/fire response, continuous look, repeated jump, held movement, held fire, Simulation and presentation rates, worker transactions, main-thread gaps, and per-frame model/visibility/Particle/audio/dynamic/world/viewmodel/render phases. `bun run profile:map-load` runs the same visible cursor-safe path through cold, same-session replacement, and page-reload `map jump_beef`, separating WASM input copy, BSP/canonical/material/entity/presentation-model/texture/Particle/environment/serialization/runtime/collision/game compilation, Blob-backed IndexedDB reads/writes, parsing, scene activation, process-tree CPU/private/working memory, active GPU identity/engine utilization/memory, and cache reuse. `bun run profile:cold-map` prefetches only the exact BSP before its measured interval, keeps derived caches absent, uses the production Chromium worker/WASM path, and retains Console-to-Ready plus every worker phase under `sourceCacheDir/profiles/cold-map/jump_beef`.

`bun run profile:map-coverage` launches installed Edge, derives goals from authored non-solid leaf-ambient samples, validates BSP leaf and Collision admission, enters Noclip, and dynamically traverses goals through real pointer-lock mouse, `W`, intermittent `A+Shift`, and Soldier primary-fire input. It fails immediately on terminal phase or one-second Simulation/display stalls, bounds stalled goals and total runtime, releases every held input, and retains complete frame, route, exclusion, long-task, GPU, and raw Windows process-counter evidence.

`bun run profile:map-sanity` skips startup playback and directly visits authored spawn, outdoor-terrain, floor, water, bridge, and intelligence viewpoints across `jump_beef`, `pl_upward`, and `ctf_2fort`. It uses authoritative noclip/setpos and mouse input, verifies real headed canvas pixels against visible world depth, preserves full-quality rendering and fixed Simulation cadence, and reports bounded five-to-ten-second frame p50/p95/p99 plus per-map screenshots under `sourceCacheDir/profiles/map-sanity`.

`bun run profile:startup` starts the real `jump_beef` development owner, records map verification, concurrent build/cache, parallel immutable publication, inline Vite creation, listener readiness, total milliseconds, and sampled parent-process memory, writes `sourceCacheDir/profiles/startup/jump_beef/report.json`, then closes both ports. Equivalent isolated worktrees on the same Windows host measured 189,402 ms baseline versus 50,531 ms candidate from empty Cargo targets, and 104,948 ms baseline versus 1,021 ms candidate with warm Cargo outputs.

`bun run profile:startup-browser` launches installed Edge without pointer capture, admits audible startup playback when a gesture is required, retains frames after 4.9 and 9.75 seconds, requires startup completion and Main Menu reveal, submits `map jump_beef` through Console, and requires in-game `Ready`.

## Non-Responsibilities

- Implementing parsers, compilers, gameplay, presentation, or asset-store semantics.
- Accepting routine machine paths or stable build policy as command arguments.
- Discovering installations or searching the machine.

## Completion

Complete when every repeated supported operation has one deterministic command independent of the caller's working directory.
