# TF2 Console And Client Diagnostic Evidence

## Target

- TF2 public content build `24207079`, patch/client/server version `10822003`.
- Composed SourceScheme revision `e9159a983557dea91b7030b382cce9ee7521c6f4de904107013bdcb47c4a732e`.
- Official behavioral authorities: Valve Source SDK 2013 commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`, `src/vgui2/vgui_controls/{consoledialog,Frame}.cpp`, and `src/game/client/vgui_fpspanel.cpp`.

## Browser Procedure

Run `bun --cwd packages/presentation/vgui run verify:browser`.

The command bundles one fixed direct-DOM fixture, starts one bounded loopback server, launches headed Chrome through `agent-browser`, and performs these schedules:

1. Capture 854×480, 1280×720, 1920×1080, 1280×720 at DPR 2, and a 1280×720 physical viewport represented by a 640×360 CSS viewport at 200% zoom.
2. Open completion for `cl_`, select the first row, and retain frame, title, client, history, entry, submit, close, popup, diagnostics, computed font, color, alpha, and side-border observations.
3. Drag the title to the workspace center, four edge centers, and four corners. Require pointer capture while dragging and exact bounded final rectangles with no capture after release.
4. Resize north, northeast, east, southeast, south, southwest, west, and northwest. Require exact final rectangles and no capture after release.
5. Hide during capture and require synchronous capture release. Recreate console and diagnostics 25 times and require one current pair of hosts with no old nodes or listeners.
6. Retain the full accessibility tree for the dialog, close button, log, text entry, submit button, diagnostic status, and diagnostic lines.

`browser-evidence.json` records the exact environment, resources, inputs, computed outputs, interaction states, accessibility tree, and result. All predicates passed in headed Chrome `149.0.0.0` on macOS `25.5.0` arm64 with Bun `1.4.0` and `agent-browser 0.32.1`.

## Windows Target Procedure

The glyph-raster acceptance input is one native TF2 public-build-`24207079` capture set on Windows with English UI, 100% OS display scale, and the default composed SourceScheme. Launch 854×480, 1280×720, and 1920×1080 separately; open `jump_beef`; open the console; wait 500 ms after focus settles; type `cl_show`; leave completion visible; and capture lossless full frames. Repeat 1280×720 at 200% application magnification. Retain focused, unfocused, submit armed, submit depressed, completion armed, title drag, and all eight resize-cursor states. Record panel rectangles, font pixel height/line height, side border colors/widths, frame/background alpha, all text colors, focus owner, OS, GPU/driver, display scale, game build, scheme/localization hashes, capture tool, and SHA-256 per image.

No Windows target capture is present. Browser captures are implementation evidence and do not substitute for this missing glyph-raster acceptance input.

## TF2 Integration Procedure

Run `bun --cwd packages/presentation/vgui run verify:tf2` with valid repository `playsrc.local.json`.

The command starts the checked `bun run dev jump_beef` workflow, launches headed WebGPU Chrome, waits for the fixed teamspawn to settle, and performs these schedules:

1. Press ControlLeft and require unchanged standing camera height.
2. Press ShiftLeft and ShiftRight independently and require a 23-unit crouched-eye reduction.
3. Dispatch browser blur while ShiftLeft is held and require standing height before the next accepted observation.
4. Query default `cl_showfps`, set mode 2, set `cl_showpos 2`, reject `cl_showfps 3`, and require catalog completion values `cl_showfps 2` and `cl_showpos 2`.
5. Require FPS/map output, authoritative player position/speed, explicit unavailable player absolute angles, console focus, fixed 300×48 diagnostic geometry at `(980, 0)`, and console geometry at `(616, 96, 640, 528)` with no overlap.
6. Set both modes to zero and require the panel hidden. Close the browser, interrupt the development owner once, and require child exit code zero.

`tf2-integration.json` records the exact environment and observations. The run passed in headed Chrome `149.0.0.0` using WebGPU `apple/metal-3` on macOS `25.5.0` arm64.

## Retained Objects

| Object | SHA-256 |
|---|---|
| `480p.png` | `4bd222cee2f0470d5cd12f3cd029a16b84f171fd32ff2ede3bacc5263197bc1e` |
| `720p.png` | `f92595a246ede6e9df7a1a1fc929fdaf3e088411f793c7b7a64a65d6971a1ce1` |
| `1080p.png` | `5b61506f3fb2da241f6f0877ae62daafebd31cef79dc7d0938947b94d035eeaf` |
| `720p-dpr2.png` | `601818cff53d79d90aa2e06a70f5f2c4361ad83696c856a1c297e512022a599c` |
| `720p-zoom2.png` | `8421066c519e83f593d9a7f38287a498b37cdd9bb60222f2ff4d54fc056c08a4` |
| `tf2-app-720p.png` | `7335cd0ed9ed3d36f215f8726d56591314b68355823e34e6e65cd112db6e93b7` |
| `browser-evidence.json` | `3dc54976692e2d135f96d241631ce3f2c89fa1acfad83df338604c136763908e` |
| `tf2-integration.json` | `b4c94fde8cc79e2234dd49007c8057df5daa74232c6530f7979006861d27dfaf` |

## Acceptance And Blocker

Computed geometry, colors, alpha, border widths/colors, focus, popup, diagnostics, capture ownership, cleanup, convars, and physical bindings pass. Browser computed styles retain the exact requested Tahoma and Lucida Console family names and pixel metrics, but configured content does not contain distributable Windows font bytes. A Windows build-24207079 target capture with those system fonts remains required before glyph-raster parity can be accepted.
