# TF2 Console And Client Diagnostic Evidence

## Target

- TF2 public content build `24207079`, patch/client/server version `10822003`.
- Composed SourceScheme revision `e9159a983557dea91b7030b382cce9ee7521c6f4de904107013bdcb47c4a732e`.
- Official behavioral authorities: Valve Source SDK 2013 commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`, `src/public/vgui/{IScheme,ISurface}.h`, `src/vgui2/vgui_controls/{consoledialog,Frame}.cpp`, and `src/game/client/vgui_fpspanel.cpp`.
- Browser contracts: CSS Fonts 4 `src`/`local()` and CSS Font Loading 3 `FontFace.load()`/`FontFaceSet`.

## Font Capability Contract

VGUI resolves Windows, macOS, and Linux scheme conditions before selecting the first admitted `yres` candidate. It admits only the selected exact content bytes, caller-supplied exact external bytes, bitmap input, platform-local face, and established surface fallback ranges. No arbitrary URL source, copied system bytes, TF2 display-font substitution, browser generic, or synthesis is admitted.

The browser verifies supplied byte length and SHA-256, loads all range faces before adding any to `document.fonts`, publishes all or none, and releases every face once. Source admission, exact metrics, browser raster, and native target pixels have separate dispositions. An unavailable source suppresses only glyph paint: panel paint, focus, editing, submission, completion, diagnostics state, accessibility, and teardown remain active.

## Browser Procedure

Run `bun run verify:browser` in this package.

- On a supported Windows host, the command retains loaded-face records; 854×480, 1280×720, 1920×1080, DPR-2, and 200%-zoom lossless captures; computed role styles; Canvas metrics for `TF2 Console Hg 0123456789`; control states; move/resize schedules; accessibility; and cleanup.
- On an unsupported host, the command removes prior browser raster captures, then executes focus, armed/depressed controls, movement, resize, cancellation, accessibility, and 25 mount/destroy cycles without painting browser fallback glyphs.

The retained run used headed Chrome `149.0.0.0` on macOS `25.5.0` arm64 with Bun `1.4.0` and agent-browser `0.32.1`. The application-owned legacy Windows target returned `unsupported-platform`; `browser-evidence.json` contains no raster captures, records active focus and accessibility, and proves zero inert state.

## TF2 Integration Procedure

Run `bun run verify:tf2` in this package with valid repository `playsrc.local.json`.

The procedure always executes the complete console, diagnostics, convar, movement, focus, and shutdown schedule. It retains a screenshot only when every selected source face is admitted. No current TF2 integration object is retained; `verify:tf2` generates it only with a valid repository-root `playsrc.local.json` and fails before launch when that input is absent.

## Native Raster Blocker

The missing acceptance input is one native TF2 build-`24207079` Windows capture set with English UI, 100% display scale, and the default composed SourceScheme. It must include the console and `DefaultFixedOutline` diagnostics at 854×480, 1280×720, and 1920×1080 plus 1280×720 at 200% magnification; focused/unfocused, submit armed/depressed, completion armed, and title-drag states; exact OS, GPU/driver, font versions, display scale, hashes, and capture tool.

Source requests non-antialiased GDI glyphs for these roles. Browser standards expose no GDI non-antialias raster control. A supported Windows browser capture proves local-face capability and records browser raster; it does not establish native pixel parity without the native target capture.

## Retained Objects

| Object | SHA-256 |
|---|---|
| `browser-evidence.json` | `61278e0af7f8bec73121aa52f88df2a4272a75cb3a2154831038f0f79ee6955c` |
