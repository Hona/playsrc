# VGUI

## Sample

```ts
import { initializeClientDiagnostics, initializeDeveloperConsole } from "@playsrc/vgui"

const initialized = initializeDeveloperConsole({
  runtimeIdentity: "tf2-console",
  limits,
  resources: resolvedConsoleResources,
  catalog: immutableCommandAndConvarCatalog,
  viewport,
  reducedMotion,
  onRequest: applicationConsoleAdapter,
})
if (!initialized.ok) throw new Error(initialized.diagnostic.code)

initialized.console.apply({ kind: "mount", root: vguiMount })
initialized.console.apply({ kind: "activate" })
initialized.console.apply({ kind: "append-output", segments: outputBatch })

const diagnostics = initializeClientDiagnostics({
  runtimeIdentity: "tf2-client-diagnostics",
  resources: resolvedDiagnosticResources,
  viewport,
})
if (!diagnostics.ok) throw new Error(diagnostics.code)
diagnostics.diagnostics.apply({ kind: "mount", root: vguiMount })
diagnostics.diagnostics.apply({ kind: "present", frame: immutableClientDiagnosticFrame })
```

## Objective

Present generic Source 1 panel trees, controls, resources, schemes, localized text, HUD animations, the developer console, focus, and input as deterministic browser DOM and CSS.

## Responsibilities

- Own panel lifetime, hierarchy, z-order, bounds, clipping, visibility, enabled state, layout invalidation, paint order, popup state, and modal state.
- Own generic controls, control factories, resource-property semantics, condition application, dialog variables, scheme binding, localization binding, animation variables, animation sequences, and message dispatch.
- Own the generic developer-console frame, pointer-captured title movement and eight-direction resize, bounded output and command history, catalog-driven completion presentation, text-entry interaction, and typed submission/completion/system requests without executing commands or owning convar state.
- Own one bounded client diagnostic panel for immutable FPS and position inputs without owning `cl_showfps`, `cl_showpos`, map, camera, player, or Simulation state.
- Own keyboard, pointer, cursor, capture, focus, navigation, IME, clipboard-seam, accessibility, reduced-motion, browser-resize, and device-pixel-ratio behavior.
- Present VGUI through direct DOM and CSS without importing Preact.
- Consume immutable gameplay or replay presentation state and emit typed commands without mutating gameplay authority or replay authority.

## Browser Contract

- The application shell uses Preact, Vite, and Bun. Preact mounts and destroys one VGUI root but does not render, reconcile, animate, or own the VGUI subtree.
- One VGUI panel owns one DOM node. Source child order, z-order, clipping, visibility, proportional scaling, focus, and input routing determine observable DOM and CSS state.
- Source HUD animation sequences remain the sole owner of VGUI animation timing and commands. CSS transitions and application animation libraries cannot replace sequence semantics.
- World rendering remains a direct Three.js consumer outside VGUI and outside Preact.

## Non-Responsibilities

- TF2 health, ammo, weapons, objectives, death notices, scoreboards, class or team menus, buildings, conditions, game events, and game commands.
- Product routes, navigation policy, uploads, settings policy, application state, network access, external-origin policy, map acquisition, command execution, or convar effects.
- World, model, material, particle, or audio rendering; VGUI emits typed requests at those seams.
- Parsing KeyValues, localization files, font files, VTF images, or HUD-animation text into lossless format records.
- Advancing gameplay, replay, networking, simulation, or ruleset state.

## Relationships

KeyValues and format packages produce typed documents; Content resolves exact logical identities; VGUI applies generic UI semantics; game modules bind immutable game presentation state and typed commands; applications own browser lifecycle and product composition.

## Developer Console Interface

`initializeDeveloperConsole` validates one unique DOM-safe runtime identity, a required limits record, one complete resolved resource bundle, one immutable command/convar catalog snapshot, one viewport, reduced-motion state, and one request sink before creating state. A missing or malformed resource fails without a partial runtime or substitute style.

The application supplies one dedicated positioned mount element whose content box matches the explicit viewport. Each VGUI service appends one absolute inset-zero host and never changes the mount element's style or adopts adjacent application DOM.

The returned deep module exposes only `apply(operation)` and `snapshot()`:

- Lifecycle operations are `mount`, `replace-root`, `activate`, `foreground`, `focus-entry`, `hide`, `cancel`, and terminal idempotent `destroy`.
- Presentation operations are `append-output`, `clear-output`, `replace-resources`, `replace-catalog`, `apply-completion`, `set-viewport`, and `set-reduced-motion`.
- `snapshot()` returns immutable model state and owned node/listener/observer/timer counts. Mutable DOM is never state authority.
- Requests are `submission`, `completion`, `completion-cancelled`, and Backquote-owned `visibility`. The request sink receives each request only after model and DOM publication.
- Frame state retains one workspace-bounded rectangle, active move/resize direction, and captured pointer identity. Release, cancellation, capture loss, browser blur, visibility loss, hide, root replacement, and destroy release capture synchronously.

The required caller limits may lower but never raise 255 UTF-8 input bytes, 100 history items, 64 owner suggestions, 63 UTF-8 bytes per owner suggestion, or 10 popup rows. The limits also bound catalogs, output batches, retained output, diagnostics, DOM nodes, and listeners. Output retention removes complete oldest segments; malformed and limit-plus-one operations publish nothing.

Catalog snapshots contain command/convar kind, exact name, hidden/development disposition, command-specific completion capability, and owner-supplied convar display values. VGUI uses the first ASCII space only to route completion. It never tokenizes for execution, owns a command/convar registry, executes a command, changes a convar, acquires content, replaces a map, or mutates Simulation.

The application adapter may interpret an exact submission as `map <catalog-map-name>` or as the playsrc extension `map https://<allowed-origin>/<path>/<map-name>.bsp`. The first selects one declared catalog identity. The second performs the shared bounded HTTPS acquisition contract and is not a TF2 parity capability. Both operation forms remain application-owned; console output is their only VGUI presentation seam.

`initializeClientDiagnostics` accepts one exact resource record and viewport. `present` admits only finite timestamps, mode values `0|1|2`, one bounded lower-ASCII map identity, finite view/player vectors, and nullable player absolute angles. It publishes at most four lines. FPS mode 1 truncates the instantaneous rate; mode 2 applies a 0.1 new-sample weight and retains low/high instantaneous integers. Position mode 1 displays view inputs; mode 2 displays player inputs and prints an explicit unavailable line when absolute angles are absent.

The selected official behavior bounds and presentation semantics are grounded in Valve Source SDK 2013 `src/common/GameUI/IGameConsole.h`, `src/public/vgui_controls/consoledialog.h`, `src/vgui2/vgui_controls/{consoledialog,Frame}.cpp`, `src/game/client/vgui_fpspanel.cpp`, `src/public/tier1/convar.h`, and `src/public/tier1/CommandBuffer.h`. Configured SourceScheme, base-scheme, localization, border, geometry, and control-state inputs resolve for public TF2 build `24207079`. Windows Tahoma/Lucida Console font bytes and a declared Windows target glyph-raster capture remain unavailable; browser evidence therefore proves exact computed role/family/size/line-height values but not target glyph-raster parity.

Run deterministic package evidence with `bun test packages/presentation/vgui/tests`, retained headed captures with `bun --cwd packages/presentation/vgui run verify:browser`, and the configured TF2 integration schedule with `bun --cwd packages/presentation/vgui run verify:tf2`.

## Roadmap

[`ROADMAP.md`](ROADMAP.md) defines 42 behavior rows. The five candidate inventories contain 300 items and 0 generated or accepted items.

## Completion

Complete only when the accepted inventories, all behavior rows, browser conformance evidence, TF2 integration, and ownership registry agree on one current VGUI interface with no duplicate UI authority.
