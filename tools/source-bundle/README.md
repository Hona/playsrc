# Source Dependency Bundle

`playsrc-source-bundle <map>` reads only repository `playsrc.local.json`, the selected TF2 map declaration, the exact source-cache BSP object, configured TF2 build `24207079`, and the `gameinfo.txt`-declared TF2/HL2 VPK families. It resolves active BSP PAK, optional supplement, then TF2/HL2 misc, texture, and sound VPKs; composes every world/model/particle-material VMT dependency; resolves selected textures; closes MDL/VVD/DX90.VTX/ANI/PHY dependencies for map dynamic props, Soldier/Demoman, stock projectile, launcher, and viewmodel models; includes the five target PCFs and eight stock projectile WAVE resources; and writes one deterministic `PSDB` bundle of lowercase logical paths plus unchanged raw Source bytes under `sourceCacheDir/browser-bundles`.

For `jump_beef`, the bundle contains both profile-qualified sky families, all six LDR/HDR cubemaps, resolved `infodecal` inputs, complete selected model textures, and the projectile PCF material closure. The exact closure is 294 entries, 112,112,616 bytes, SHA-256 `34cbd09a63f1ba8407c7a775de20467773f87a41db78e34447734799fa2dba78`.

`bun run verify:tf2-wasm jump_beef` internally invokes the bounded `--verify-hdr` evidence mode. That mode writes `jump_beef.native-hdr.psmp` beside the bundle and reports its payload and derived hashes for byte comparison with WASM; it is not a development, publication, or browser prerequisite.

The browser transfers the bundle to the same Rust WASM compiler that parses VMT/VTF and StudioModel companions and emits the runtime map/model payload. The bundle performs content resolution and raw-byte packaging only; it contains no decoded texture/model, renderer resource, GLB, native map conversion, or gameplay state.
