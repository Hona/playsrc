# Generic VGUI Browser Evidence

`bun run verify:runtime-browser` bundles the framework-independent fixture, mounts every selected generic control family in direct DOM, replays pointer capture, checkbox, numeric text, slider, modal and cleanup schedules in headed Chrome, and writes `browser-evidence.json` plus `runtime.png` for review.

The retained JSON records the browser identity, complete runtime snapshot, panel/resource counts, DOM names, roles, computed geometry/styles, typed requests, modal accessibility tree and 25-cycle cleanup result. The fixture declares its unmounted font source unavailable, so glyph color is suppressed while the accessibility tree retains text; the screenshot tests non-glyph browser presentation without using a fallback raster. Configured TF2 resource and native-raster parity remain separate evidence obligations.
