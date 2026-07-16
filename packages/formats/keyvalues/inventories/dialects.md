# KeyValues Dialect Inventory

Owner: [`../ROADMAP.md`](../ROADMAP.md)

State: Candidate; Not accepted.

Authority identity: Valve Source SDK 2013 commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`, the candidate Source format inventory at [`../../inventories/formats.md`](../../inventories/formats.md), and exact TF2, CS:S, and legacy Source 1 CS:GO content-build indexes. The parent inventory is Not accepted and the content-build indexes are Missing.

Generator command: Missing.

Candidate item count: 4. Accepted item count: 0.

Each row assigns KeyValues syntax only. The named semantic owner interprets fields after parsing.

| Dialect identity | Encoding and syntax profile | Named consumer | Semantics beyond syntax owner | Authority | Acceptance |
|---|---|---|---|---|---|
| `kv1-text-default` | No-BOM Source byte text or UTF-16LE-BOM text; ordered named object roots; quoted or unquoted tokens; literal backslashes; `//` comments; scalar inference; bracket conditionals; top-level `#include` and `#base` | TF2 `scripts/items/items_game.txt` loader | `games/tf2` | Source SDK `src/public/tier1/KeyValues.h`, `src/tier1/KeyValues.cpp`, and `src/game/shared/econ/econ_item_system.cpp` | Blocked by the Not accepted parent format inventory and Missing generator/content indexes |
| `kv1-text-vgui-escaped` | `kv1-text-default` tree plus escaped quoted strings; VGUI resolution and override keys remain ordinary syntax nodes | TF2 `Resource/GameMenu.res` loader | `apps/web/tf2` | Source SDK `src/tier1/utlbuffer.cpp`, `src/vgui2/vgui_controls/BuildGroup.cpp`, and `src/game/client/tf/tf_hud_mainmenuoverride.cpp` | Blocked by the Not accepted parent format inventory and Missing generator/content indexes |
| `kv1-binary-native` | One-byte type tags and peer terminators; NUL-terminated names and byte strings; little-endian signed 32-bit integer, binary32 float, 32-bit pointer payload, RGBA, and unsigned 64-bit payloads; wide strings have no valid payload in this dialect | TF2 item-schema and store-pricesheet loaders | `games/tf2` | Source SDK `src/public/tier1/KeyValues.h`, `src/tier1/KeyValues.cpp`, `src/game/shared/econ/econ_item_schema.cpp`, and `src/game/client/econ/store/store_panel.cpp` | Blocked by the Not accepted parent format inventory and Missing generator/content indexes |
| `kv1-binary-kvpacker` | KVPacker tags and peer terminator; NUL-terminated names/byte strings; length-prefixed UTF-16 code units; numeric, color, and pointer payloads whose producer ABI is not self-described | Candidate TF2 GC request/response payload interchange | `games/tf2` | Source SDK `src/public/tier1/kvpacker.h` and `src/tier1/kvpacker.cpp` | Blocked: no accepted playsrc consumer or exact producer pointer-width/endianness profile; parent inventory and generator are also unavailable |

## Boundary Audit

These KV-shaped inputs are not additional KeyValues dialect items:

| Input | Syntax owner | Reason |
|---|---|---|
| VMT shader documents and Patch `include`/`insert`/`replace` | `packages/formats/vmt` | VMT owns its root shader, material condition expressions, patch dependency rules, parameter/proxy schema, and effective document. |
| BSP entity-lump text | `packages/formats/bsp` | The BSP-contained grammar is a sequence of quoted key/value pairs inside anonymous entity blocks, not named KV1 object roots. |
| PHY textual key data | `packages/formats/phy` | PHY owns the embedded key-data byte range and typed physics metadata contract. |
| DMX `keyvalues`, `keyvalues2`, and `keyvalues2_flat` | Proposed `packages/formats/dmx` | These are DMX element-graph encodings with DMX headers, references, and typed attributes, not KV1. |
| PCF particle bytes | Proposed `packages/formats/pcf` over the proposed DMX owner | PCF consumes a DMX graph. Particle operator semantics do not create a KV1 dialect. |
| Sound-event scripts, soundscape definitions, and sentence definitions | Proposed `packages/formats/audio-script` | Their typed document schemas remain separate from the reusable KV1 token/tree contract. |
| Particle manifest field names | `packages/presentation/particle` | A particle-manifest schema over `kv1-text-default` does not create another parser dialect. |
| Item, weapon, population, and game-configuration field names | `games/<game>` | A game-owned schema over `kv1-text-default` does not create another parser dialect. |
| Content mount and search-path field names | `packages/content` | Content-provider policy over `kv1-text-default` does not create another parser dialect. |
| VGUI resource and scheme field names | `apps/web/<product>` | Product UI semantics over `kv1-text-vgui-escaped` do not create another parser dialect. |
