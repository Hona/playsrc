# TF2 Application

## Objective

Deliver faithful TF2 gameplay and presentation at `tf2.playsrc.online`.

## Responsibilities

- Assemble the TF2 game, selected TF2 rulesets, simulation, networking, presentation, assets, and product UI.
- Own browser lifecycle, input, settings, navigation, and application state.
- Present the configured Main Menu before gameplay initialization; drive Console map loading, pause/resume/disconnect, TF2 HUD resources, and persistent Options through the game-owned GameUI/HUD adapters and generic VGUI runtime. HUD/GameUI/Options resources share the selected desktop condition, and HUD geometry reapplies exact proportional coordinates on integer viewport changes. Standard and Advanced Options construct independently on first request, and hidden Options perform no frame work.
- Adapt typed VGUI console submissions to owner-defined command and convar operations. `map <catalog-map-name>` selects one declared map; `map https://<allowed-origin>/<path>/<map-name>.bsp` is an explicit bounded playsrc acquisition operation and is not a TF2 parity capability.
- Own typed `cl_showfps 0|1|2` and `cl_showpos 0|1|2` values, immutable diagnostic inputs, exact catalog revisions/current-value output, and explicit unavailable distinctions; VGUI owns the bounded diagnostic panel.
- Run one direct-BSP TF2 worker with Rust-owned Source Simulation pacing, physical-button held/release identity across simultaneous inputs, 15 ms latest-value browser clock admission with ordered suspension transitions, display-cadence camera sampling, raw pointer movement when supported, post-command teleport-angle rebasing, bounded Blob-backed map/presentation caching, exact cached model-header reconstruction, Three.js/WebGPU bundled static world plus inline brush models, selected-teamspawn camera initialization, Source-signed pointer look, physical left/right Shift crouch, separate VGUI mount, exact blocker ledger, and generation-safe map replacement.
- Select `mat_hdr_level 0|1|2` through profile-distinct map generations; cache bounded StudioModel artifacts; execute projectile PCF phases in the worker; and apply Rust-produced PVS face candidates plus typed environment counts without gameplay feedback. Do not configure `jump_beef` course state until an accepted Tempus core and exact zone-contact definition are supplied.
- Consume PSSN v8 random/audio/Collision/pusher/Entity/locker facts, 123 brush descriptors, collision-selected moving marks, PMRQ v5/PMPO v4 hand-plus-item/ammunition frames, PSMP v3 facing, Particle v3 materials/sheets/trails, typed alpha discard, complete Water view plans, Source Audio, and configured macOS SourceScheme fonts. IVP remains explicit: sticky fire is rejected before spawn. Native glyph-raster and aligned target captures remain blockers.
- Keep the package-owned all-class TF2 core separate from the compact Soldier/Demoman direct-map session until one complete adapter can replace the current authority. Keep Demo, Networking, and Replay unmounted until TF2 recorded-state decoding/presentation and the declared upload/control experience exist; never route recorded commands through Simulation.

Run the checked end-to-end browser procedure with `bun run verify:browser jump_beef`. It starts the same checked development owner as `bun run dev jump_beef`, verifies Main Menu before gameplay, loads through Console, drives cold and warm headed-browser runs, retains fixed 1,280×720 world evidence plus a 390×844 VGUI capture, and verifies one-interrupt process shutdown.

Run console capability evidence with `bun --cwd packages/presentation/vgui run verify:browser` and configured TF2 integration with `bun --cwd packages/presentation/vgui run verify:tf2`. Unsupported hosts suppress VGUI paint/input and retain no fallback-font raster.

## Non-Responsibilities

- Reimplementing TF2 mechanics, rulesets, generic Source behavior, or asset-store semantics.
- Owning future server authority inside presentation code.

## Completion

Complete when the declared TF2 application experiences integrate completed modules without duplicate behavior or authority.
