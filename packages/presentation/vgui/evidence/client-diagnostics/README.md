# TF2 Console And Client Diagnostic Evidence

## Target

- TF2 public content build `24207079`, patch/client/server version `10822003`.
- Composed SourceScheme revision `e9159a983557dea91b7030b382cce9ee7521c6f4de904107013bdcb47c4a732e`.
- Official behavioral authorities: Valve Source SDK 2013 commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`, `src/public/vgui/{IScheme,ISurface}.h`, `src/vgui2/vgui_controls/{consoledialog,Frame}.cpp`, and `src/game/client/vgui_fpspanel.cpp`.
- Browser contracts: CSS Fonts 4 `src`/`local()` and CSS Font Loading 3 `FontFace.load()`/`FontFaceSet`.

## Platform-Font Contract

The declared target is Windows. Tahoma default-weight and weight-500 families cover `U+0000-FFFF`. Each Lucida Console family selects Lucida Console for `U+0000-00FF`, then Tahoma for `U+0100-FFFF`, at default weight or weight 500. The resulting six range faces retain Source order; every face has exactly one `local()` source. No URL source, copied system bytes, alternate local name, generic family, or synthesis is admitted.

The browser loads all six range faces before adding any to `document.fonts`. One rejection publishes none. Publication failure removes every previously added face. A non-Windows platform is rejected before probing installed fonts. Unsupported VGUI hosts are `inert`, accessibility-hidden, visibility-hidden, and expose `data-platform-font-capability="unsupported"`.

## Browser Procedure

Run `bun run verify:browser` in this package.

- On a supported Windows host, the command retains loaded-face records; 854×480, 1280×720, 1920×1080, DPR-2, and 200%-zoom lossless captures; computed role styles; Canvas metrics for `TF2 Console Hg 0123456789`; control states; move/resize schedules; accessibility; and cleanup.
- On an unsupported host, the command records the exact platform and capability result, removes prior browser raster captures, and exits successfully. It never captures browser fallback glyphs.

The retained run used headed Chrome `149.0.0.0` on macOS `25.5.0` arm64 with Bun `1.4.0` and agent-browser `0.32.1`. It returned `unsupported-platform` for the Windows target before local-face probing. `browser-evidence.json` contains no captures.

## TF2 Integration Procedure

Run `bun run verify:tf2` in this package with valid repository `playsrc.local.json`.

The retained macOS run reached the Ready application state, observed the unsupported developer-console host, required console and diagnostics paint/input suppression, removed the old fallback-font screenshot, closed the browser, interrupted the development process once, and required child exit zero. Supported Windows hosts continue into the complete console, diagnostics, convar, movement, and screenshot schedule.

## Native Raster Blocker

The missing acceptance input is one native TF2 build-`24207079` Windows capture set with English UI, 100% display scale, and the default composed SourceScheme. It must include the console and `DefaultFixedOutline` diagnostics at 854×480, 1280×720, and 1920×1080 plus 1280×720 at 200% magnification; focused/unfocused, submit armed/depressed, completion armed, and title-drag states; exact OS, GPU/driver, font versions, display scale, hashes, and capture tool.

Source requests non-antialiased GDI glyphs for these roles. Browser standards expose no GDI non-antialias raster control. A supported Windows browser capture proves local-face capability and records browser raster; it does not establish native pixel parity without the native target capture.

## Retained Objects

| Object | SHA-256 |
|---|---|
| `browser-evidence.json` | `1d34a764da8bb0fb8173165cb03422940fbb31ba8cac8f03ae53588bd1c668b7` |
| `tf2-integration.json` | `de4f4b051a0206f66f5075927ca3235c4580e9982d7c45702865f27c0fb3467d` |
