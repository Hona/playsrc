# VGUI

## Sample

```ts
import { initializeClientDiagnostics, initializeDeveloperConsole, initializeVguiRuntime } from "@playsrc/vgui"

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

const generic = initializeVguiRuntime({
  runtimeIdentity: "tf2-vgui",
  root: vguiMount,
  rootControl: { control: "EditablePanel", name: "ClientRoot" },
  viewport,
  limits: vguiLimits,
  clock: presentationClock,
  random: deterministicPresentationRandom,
  scheme: suppliedScheme,
  localization: suppliedLocalization,
  animationScripts: suppliedHudAnimations,
  customControls: gameOwnedControlRegistrations,
  reducedMotion,
  onRequest: applicationVguiAdapter,
})
if (!generic.ok) throw new Error(generic.diagnostic.code)
generic.runtime.apply({
  kind: "replace-resource",
  parent: generic.runtime.snapshot().rootPanel,
  document: parsedResource,
  selection: { activeConditions, resolutionSuffixes },
})
```

## Objective

Present generic Source 1 panel trees, controls, resources, schemes, localized text, HUD animations, the developer console, focus, and input as deterministic browser DOM and CSS.

## Responsibilities

- Own panel lifetime, hierarchy, stable current-order signed-z insertion and mutation, bounds, clipping, visibility, enabled state, inherited proportional mode, integer viewport relayout, layout invalidation, paint order, separately ordered popups, cross-root activated/topmost/modal window stacking, and focus-safe DOM reordering.
- Own generic controls, control factories, resource-property semantics, condition application, dialog variables, scheme binding, localization binding, animation variables, animation sequences, and message dispatch.
- Preserve Label-descendant alignment/insets, authored `CExLabel` foreground-over-shadow sibling paint with visible opaque/translucent one-pixel offsets, exact normal/armed/depressed/selected/focused/disabled colors and borders, slider track/thumb state, PropertySheet tabs, SectionedList column flags, and one workspace-positioned ComboBox Menu popup with separate selected, armed and disabled rows.
- Own the generic developer-console frame, pointer-captured title movement and eight-direction resize, bounded output and command history, catalog-driven completion presentation, text-entry interaction, and typed submission/completion/system requests without executing commands or owning convar state.
- Own one bounded client diagnostic panel for immutable FPS and position inputs without owning `cl_showfps`, `cl_showpos`, map, camera, player, or Simulation state.
- Own keyboard, pointer, cursor, capture, focus, navigation, IME, clipboard-seam, accessibility, reduced-motion, browser-resize, and device-pixel-ratio behavior while leaving keyboard, text-input, and composition events from foreign DOM roots untouched.
- Recompute screen-relative and explicit `proportionalToParent` resource geometry from each admitted integer CSS-pixel viewport; device-pixel ratio alone never changes panel bounds.
- Compose lossless scheme documents, select desktop conditions and ordered font candidates, request exact content/external/bitmap/local sources, mount range faces atomically, consume supplied metrics/raster profiles, and suppress only unavailable glyph paint without disabling panel state or input.
- Present VGUI through direct DOM and CSS without importing Preact.
- Retain stable DOM parents and exact material rasters, skip geometry/DOM work for static frames, batch integration construction into one layout/publication commit, and publish only panels whose complete presentation signature changed.
- Consume immutable gameplay or replay presentation state and emit typed commands without mutating gameplay authority or replay authority.

## Browser Contract

- The application shell uses Preact, Vite, and Bun. Preact mounts and destroys one VGUI root but does not render, reconcile, animate, or own the VGUI subtree.
- One VGUI panel owns one DOM node. Source child order, z-order, clipping, visibility, proportional scaling, focus, and input routing determine observable DOM and CSS state.
- Source HUD animation sequences remain the sole owner of VGUI animation timing and commands. CSS transitions and application animation libraries cannot replace sequence semantics.
- World rendering remains a direct Three.js consumer outside VGUI and outside Preact.

## Generic Runtime Interface

`initializeVguiRuntime` validates the complete configuration before mounting one direct-DOM workspace. It requires one explicit limits record, monotonic clock, deterministic random stream, viewport, resolved scheme, localization table, ordered parsed animation script set, immutable game-owned custom-control registrations, reduced-motion selection and typed request sink. VGUI never parses localization, font, image, VTF or HUD-script bytes and never substitutes an unknown control, property, token, font, border, image, animation variable or command effect.

The selected generic control set is `Panel`, `EditablePanel`, `Label`, `ImagePanel`, `Button`, `TextEntry`, `RichText`, `Frame`, `ScrollBar`, `ScrollBar_Vertical`, `ScrollBar_Horizontal`, `Slider`, `ComboBox`, `Menu`, `MenuItem`, `PropertySheet`, `PropertyPage`, `CheckButton`, `RadioButton`, `ProgressBar`, `ListPanel`, `MessageBox`, `QueryBox` and `URLLabel`. Custom game controls expose only their element, role, focusability, accepted resource properties and inherited animation-variable definitions; game behavior remains outside VGUI.

The returned deep module exposes only `apply(operation)` and `snapshot()`. Operations own panel creation, deferred deletion, reparenting, geometry, state, z-order, popup order, resource replacement, registry replacement, dialog variables, control values, focus/default buttons, pointer capture, application and subtree modals, pointer/keyboard/composition input, clipboard results, messages, animation sequences, viewport changes, frames and terminal destruction. Every panel identity is monotonic and never reused. Resource replacement stages and validates the complete selected tree before deleting any resource-owned control. Code-created controls are matched ASCII-insensitively by block name and reused; unknown factories never become `Panel`.

One frame projects nondecreasing scheduler and browser-event timestamps onto one monotonic effective clock, rolls input edges, commits focus loss before focus gain, dispatches due messages, records the focus tick, solves geometry, runs due HUD commands before active interpolation, commits deferred deletion, publishes DOM, then emits typed requests. First callbacks predating runtime construction and callbacks following an event-driven frame never regress effective time; reversed caller schedules, negative values, NaN, and infinities remain errors. CSS transitions and animations never own VGUI sequence timing. Reduced motion publishes each interpolation endpoint at its start while preserving delayed command times and completion lifetime.

Normal children retain model and DOM parent ownership. Parenting and signed-z changes cross only strictly different-z siblings; existing equal-z neighbors retain their current order, so an authored foreground followed by its equal-z shadow paints the shadow first after default-z insertion. Popups retain model parent ownership but publish under the workspace, escape parent clipping, and use their own normal/topmost/modal order instead of authored child z. Frames start hidden. A document-local window workspace promotes the active popup's owning browser stacking context, restores its prior inline z-index on deactivation, and makes foreign VGUI hosts inert during application modality. Hit testing uses half-open solved clips in reverse paint order. Destroy clears focus, capture, modal state, queued work, animation work, clipboard requests, stacking overrides, auxiliary item nodes, listeners, and panel nodes.

Scheme input contains resolved colors, base settings, fonts, line/image/nine-slice borders and browser-presentable image descriptors. Line borders retain ordered side records and offsets. A non-white image tint requires a supplied exact frame/tint browser variant; an absent variant suppresses the image and remains diagnostic instead of applying a CSS color approximation. Fonts with unavailable exact browser presentation suppress their glyph color without disabling control state, input or accessibility.

The behavior is grounded in Valve Source SDK 2013 commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`: `src/public/vgui/{IVGui,IPanel,IClientPanel,IInput,IInputInternal,ISurface,IScheme,ILocalize,IImage,IBorder,ISystem}.h`, `src/public/vgui_controls/{Panel,EditablePanel,BuildGroup,MessageMap,PanelAnimationVar,AnimationController,FocusNavGroup}.h`, and the selected generic-control implementations under `src/vgui2/vgui_controls`.

## Non-Responsibilities

- TF2 health, ammo, weapons, objectives, death notices, scoreboards, class or team menus, buildings, conditions, game events, and game commands.
- Product routes, navigation policy, uploads, settings policy, application state, network access, external-origin policy, map acquisition, command execution, or convar effects.
- World, model, material, particle, or audio rendering; VGUI emits typed requests at those seams.
- Parsing KeyValues, localization files, font files, VTF images, or HUD-animation text into lossless format records.
- Advancing gameplay, replay, networking, simulation, or ruleset state.

## License

`src/runtime.ts` ports behavior from Valve Source SDK 2013 and is subject to the [Source 1 SDK License](LICENSE.source-sdk-2013). The repository includes Valve's exact `thirdpartylegalnotices.txt` at [`../particle/thirdpartylegalnotices.txt`](../particle/thirdpartylegalnotices.txt), SHA-256 `21319cf7b185d8676801680bc394655a028bc84e257ed844023c8bbed66d3a9e`. Original playsrc material remains subject to the repository MIT license.

## Relationships

KeyValues and format packages produce typed documents; Content resolves exact logical identities; VGUI applies generic UI semantics; game modules bind immutable game presentation state and typed commands; applications own browser lifecycle and product composition.

## Developer Console Interface

`initializeDeveloperConsole` validates one unique DOM-safe runtime identity, a required limits record, one complete resolved resource bundle, one immutable command/convar catalog snapshot, one viewport, reduced-motion state, and one request sink before creating state. A missing or malformed resource fails without a partial runtime or substitute style.

The application supplies one dedicated positioned mount element whose content box matches the explicit viewport. Each VGUI service appends one absolute inset-zero host without adopting adjacent application DOM. When independently mounted windows overlap, their shared document-local workspace temporarily projects exact activation/topmost/modal precedence onto the smallest owning stacking-context branches and restores each prior inline z-index when the override is no longer required.

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

The selected official behavior bounds and presentation semantics are grounded in Valve Source SDK 2013 `src/common/GameUI/IGameConsole.h`, `src/public/vgui/{IScheme,ISurface}.h`, `src/public/vgui_controls/consoledialog.h`, `src/vgui2/vgui_controls/{consoledialog,Frame}.cpp`, `src/game/client/vgui_fpspanel.cpp`, `src/public/tier0/platform.h`, `src/tier1/KeyValues.cpp`, `src/public/tier1/convar.h`, and `src/public/tier1/CommandBuffer.h`. Configured SourceScheme, base-scheme, localization, border, geometry, control-state, and eight TF2 font-file identities resolve for public TF2 build `24245096`.

## Scheme And Font Interface

`resolveVguiSchemeFonts` accepts exact document identities, hashes, relative base edges, lossless ordered nodes, file identities, desktop context, and named normal/proportional lookups. It filters `$WIN32`, `$WINDOWS`, `$OSX`, `$LINUX`, `$POSIX`, `$DECK`, `$X360`, and negation; recursively merges bases with derived values winning; applies desktop `_minmode`; selects the first candidate admitted by `yres`; applies 480-height proportional scaling and language minimums; and emits immutable source/range/effect requests. Candidate-local `range` is inert; a selected-language custom-file range owns the primary family interval.

`mountVguiFontSet` verifies every supplied content or external byte length and SHA-256, tries only each face's declared sources, publishes all loaded faces or none, and exposes one idempotent destroy operation. Exact source admission does not imply exact Source metrics or pixels. The mounted capability reports supplied exact metric/raster profiles or typed requirements for browser shaping, metrics, local version, bitmap adaptation, non-antialiased mode, and native raster comparison. `measureVguiText`, `getVguiCharacterMetrics`, and `applyVguiGlyphEffects` consume exact supplied profiles and fixed RGBA glyph inputs.

The existing console and diagnostic resource records expose unavailable legacy browser families as `source-required`. Their text is transparent instead of browser-fallback rendered, while frame/border paint, focus, editing, submission, completion, diagnostics state, accessibility, and cleanup remain active. Configured content supplies no Tahoma or Lucida Console bytes, and no native non-antialiased Windows target capture exists, so those raster requirements remain explicit.

Run `bun test packages/presentation/vgui/tests` from the repository root. From this package directory, run visibly headed configured-font foreground/shadow RGBA, offset, Options/console overlap, and hit-target evidence with `bun run verify:ordering-browser`; generic control/resource/input/animation DOM and accessibility evidence with `bun run verify:runtime-browser`; retained console/diagnostic capability evidence with `bun run verify:browser`; and configured TF2 integration with `bun run verify:tf2`.

## Roadmap

[`ROADMAP.md`](ROADMAP.md) defines 47 behavior rows. The five candidate inventories contain 301 items and 0 generated or accepted items. [`inventories/tf2-font-files.md`](inventories/tf2-font-files.md) retains the eight configured game-font identities separately from the denominator inventories.

## Completion

Complete only when the accepted inventories, all behavior rows, browser conformance evidence, TF2 integration, and ownership registry agree on one current VGUI interface with no duplicate UI authority.
