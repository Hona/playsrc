# TF2 Application

## Objective

Deliver faithful TF2 gameplay and presentation at `tf2.playsrc.online`.

## Responsibilities

- Assemble the TF2 game, selected TF2 rulesets, simulation, networking, presentation, assets, and product UI.
- Own browser lifecycle, input, settings, navigation, and application state.
- Adapt typed VGUI console submissions to owner-defined command and convar operations. `map <catalog-map-name>` selects one declared map; `map https://<allowed-origin>/<path>/<map-name>.bsp` is an explicit bounded playsrc acquisition operation and is not a TF2 parity capability.
- Own typed `cl_showfps 0|1|2` and `cl_showpos 0|1|2` values, immutable diagnostic inputs, exact catalog revisions/current-value output, and explicit unavailable distinctions; VGUI owns the bounded diagnostic panel.
- Run one direct-BSP TF2 worker, Three.js/WebGPU canvas, selected-teamspawn camera initialization, Source-signed pointer look, physical left/right Shift crouch with no Control crouch binding, application-owned fixed-tick pacing/input neutralization, separate VGUI mount, exact support-blocker ledger, and generation-safe catalog or ephemeral HTTPS map replacement.

Run the checked end-to-end browser procedure with `bun run verify:browser jump_beef`. It starts `bun run dev jump_beef`, drives cold and warm headed-browser runs, retains a fixed 1,280×720 canvas capture, rejects missing spawn-room surfaces or reversed pointer direction, and verifies one-interrupt process shutdown.

Run console visual/interaction captures with `bun --cwd packages/presentation/vgui run verify:browser` and configured TF2 convar/diagnostic/Shift integration with `bun --cwd packages/presentation/vgui run verify:tf2`.

## Non-Responsibilities

- Reimplementing TF2 mechanics, rulesets, generic Source behavior, or asset-store semantics.
- Owning future server authority inside presentation code.

## Completion

Complete when the declared TF2 application experiences integrate completed modules without duplicate behavior or authority.
