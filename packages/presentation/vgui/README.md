# VGUI

## Sample

```ts
import { createVgui } from "@playsrc/vgui"

const vgui = createVgui({ content, mount })
await vgui.load("resource/UI/HudPlayerHealth.res")
vgui.update(presentationState)
```

## Objective

Present generic Source 1 panel trees, controls, resources, schemes, localized text, HUD animations, the developer console, focus, and input as deterministic browser DOM and CSS.

## Responsibilities

- Own panel lifetime, hierarchy, z-order, bounds, clipping, visibility, enabled state, layout invalidation, paint order, popup state, and modal state.
- Own generic controls, control factories, resource-property semantics, condition application, dialog variables, scheme binding, localization binding, animation variables, animation sequences, and message dispatch.
- Own the generic developer-console frame, bounded output and command history, catalog-driven completion presentation, text-entry interaction, and typed submission/completion/system requests without executing commands or owning convar state.
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

## Roadmap

[`ROADMAP.md`](ROADMAP.md) defines 41 behavior rows. The five candidate inventories contain 300 items and 0 generated or accepted items.

## Completion

Complete only when the accepted inventories, all behavior rows, browser conformance evidence, TF2 integration, and ownership registry agree on one current VGUI interface with no duplicate UI authority.
