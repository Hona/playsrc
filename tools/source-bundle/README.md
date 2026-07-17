# Source Dependency Bundle

`playsrc-source-bundle <map>` verifies the configured TF2 app/build/depot identities, derives GAME precedence from the exact `gameinfo.txt`, and resolves the BSP PAK plus configured wildcard, VPK, loose, and download providers for world, model, particle, and audio presentation. It includes every requested MDL/VVD/DX90.VTX/ANI/PHY companion or exact optional absence, selected VMT/VTF inputs, five target PCFs, eight stock WAVE resources, `game_sounds_weapons.txt`, and `soundmixers.txt`, then writes one deterministic lowercase-path `PSDB` of unchanged Source bytes.

For `jump_beef`, the exact closure is 296 entries, 112,303,242 bytes, SHA-256 `494c282a45b2c1ae1882e66aabe234cda3f92d950e1d2a37c2616db845164884`.

The same command writes `jump_beef.dependencies.json`: 345 bounded requests comprising 296 resolved entries and 49 optional authoritative absences, 305,858 bytes, SHA-256 `8c721c901a7b17d8bde76374690b61bbeb3ca2d172c7964ebf4b75b585294b8b`. Each resolved row records exact app/build/depot identity, bytes, provider revision/location, and consumers. Each absence records all 13 exact checked locations. The ledger and PSDB descriptors are verified before local Asset Store publication.

`bun run verify:tf2-wasm jump_beef` internally invokes the bounded `--verify-hdr` evidence mode. That mode writes `jump_beef.native-hdr.psmp` beside the bundle and reports its payload and derived hashes for byte comparison with WASM; it is not a development, publication, or browser prerequisite.

The browser transfers the bundle to the same Rust WASM compiler that parses VMT/VTF and StudioModel companions and emits the runtime map/model payload. The bundle performs content resolution and raw-byte packaging only; it contains no decoded texture/model, renderer resource, GLB, native map conversion, or gameplay state.
