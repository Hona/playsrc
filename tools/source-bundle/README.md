# Source Dependency Bundle

`playsrc-source-bundle <map>` reads only repository `playsrc.local.json`, the selected TF2 map declaration, the exact source-cache BSP object, configured TF2 build `24207079`, and the `gameinfo.txt`-declared TF2/HL2 VPK families. It resolves the active BSP PAK before `tf2_misc`, `tf2_textures`, `hl2_misc`, and `hl2_textures`; composes every world VMT dependency; resolves every source texture request; and writes one deterministic `PSDB` bundle of lowercase logical paths plus unchanged raw VMT/VTF bytes under `sourceCacheDir/browser-bundles`.

The browser transfers the bundle to the same Rust WASM compiler that parses VMT/VTF and emits the runtime map payload. The bundle performs content resolution and raw-byte packaging only; it contains no decoded texture, renderer resource, GLB, native map conversion, or gameplay state.
