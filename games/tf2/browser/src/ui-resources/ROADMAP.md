# TF2 Browser UI Resources Roadmap

This game-owned leaf describes the immutable configured-content inputs consumed by generic VGUI, TF2 HUD/GameUI models, and the TF2 browser application. It does not parse Source formats, create controls, execute commands, or own browser capabilities.

## Inputs

- The exact `playsrc.local.json` TF2 root, its `gameinfo.txt` provider order, and content build identity.
- KeyValues syntax trees, Content resolutions, VTF metadata, and font identities supplied by their owning producers.
- Valve Source SDK 2013 commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`, including `src/game/client/game_controls/baseviewport.cpp`, `src/game/client/tf/tf_hud_mainmenuoverride.cpp`, `src/game/client/tf/tf_hud_playerstatus.cpp`, `src/game/client/tf/tf_hud_ammostatus.cpp`, `src/game/client/tf/tf_hud_weaponselection.cpp`, `src/game/client/tf/tf_hud_demomanpipes.cpp`, and `src/game/client/tf/vgui/tf_matchmaking_dashboard.cpp`.

## Output

One deeply immutable descriptor retains selected main-menu, loading, pause, HUD, and Options roots and their complete selected dependency closure. Every record has a stable identity, owner, source SHA-256, provider identity, provider revision, provider kind, logical path, selected conditions, ordered occurrence, and bounded typed payload. Missing dependencies remain typed records and never select replacement resources.

## Behavior Families

| Source/TF2 behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| GameUI, TF2 viewport, HUD, and Options code select exact resource roots through configured search paths. | Generate one provider-bound root and dependency inventory without filesystem discovery or extracted content. | Two configured generations compare byte-for-byte; changed, missing, and malformed inputs fail atomically. | Complete |
| VGUI resource trees preserve panel order, repeated keys, conditions, resolution overrides, control identities, and property occurrences. | Emit ordered typed panel nodes and classify every selected control and resource property by generic VGUI, TF2, GameUI, settings, application, service, external, or unsupported ownership. | Generated occurrence totals and zero-unclassified checks cover every selected tree. | Complete |
| ClientScheme and SourceScheme supply ordered colors, fonts, custom font files, bitmap fonts, borders, aliases, and base composition. | Emit ordered scheme documents, base edges, font and border dependencies, platform/language/resolution conditions, and explicit missing base records. | Fixed scheme hashes, dependency identities, repeated-key order, and producer-validation failures are compared. | Complete |
| GameUI, VGUI, and TF2 localization tables supply selected labels and formatted text. | Emit ordered localization documents and token occurrences for English plus language-template identities without silently substituting an unresolved token. | Every selected `#token` occurrence resolves to one retained definition or one typed missing dependency. | Complete |
| Image controls, borders, HUD code, and menu backgrounds select exact image identities. | Emit image occurrences, exact Content outcomes, and VTF metadata for resolved VTF objects; dynamic and externally supplied images retain explicit capability owners. | VTF producer inspection and zero-unclassified image occurrence checks cover the selected closure. | Complete |
| The HUD animation manifest composes scripts in source order and scripts retain named sequences and ordered commands. | Emit manifest edges, script hashes, conditions, and source descriptors without parsing or scheduling scripts outside generic VGUI. | Manifest order, script identities, missing sources, malformed manifests, and source hashes are fixed evidence. | Complete |
| Configured menu and resource commands route to game, application, online services, or external hosts. | Classify each exact command and required capability without execution; unsupported records are inert. | Exhaustive command/capability tests prove one classification and owner per configured occurrence. | Complete |
| Resource descriptors have finite browser-transfer and validation bounds. | Reject count, depth, string, retained-byte, and duplicate-stable-identity violations before publication; publish no partial descriptor. | Boundary tests cover every declared limit and immutability check. | Complete |

## Ownership Exclusions

- KeyValues owns syntax and condition-token preservation. Content owns provider resolution and provenance. VTF owns texture inspection. Generic VGUI owns resource application, scheme composition, localization lookup, controls, images, and animation execution.
- TF2 HUD and GameUI owners map game/application state to resource identities and typed requests. Settings owns cvar and binding state. The browser application authorizes service, external-navigation, storage, audio-resume, and host-close capabilities.
- This leaf never executes commands, reads extracted resource trees, scans installations, substitutes missing resources, embeds content bytes, or reparses KeyValues, VTF, font, or archive formats.
