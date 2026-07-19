# TF2 GameUI Model Roadmap

## Inputs

- Valve Source SDK 2013 commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`: `src/common/GameUI/IGameUI.h`, `src/game/client/cdll_client_int.cpp`, `src/game/client/tf/tf_hud_mainmenuoverride.cpp`, `src/game/client/tf/tf_hud_disconnect_prompt.cpp`, and `src/game/client/tf/vgui/tf_matchmaking_dashboard{,_playlist}.cpp`.
- TF2 app 440 build `24207079`, patch `10822003`: `tf2_misc_dir.vpk` SHA-256 `63f7db0d1c509e303ca9002fee9e3d805e9220ea5afdd639d8a6b68b8a3710b9`; `resource/ui/matchmakingdashboard.res` `edde2b40a83a8513251773799a37967ea8d3c4d578d4a7043c2378e158e29289`; `resource/ui/matchmakingplaylist.res` `86cda545847d2219d67b31ac39e5a26a6f711c1662404e62a26b7871d038b8a7`; `resource/ui/mainmenuoverride.res` `5f628eb8ec62ea557cf49bc13e587b48c5e7ebd480742e1795e1653c5ae8ed92`; `resource/loadingdialog.res` `97b24e2f5096d8776e7067a39cd74a35c60576b403dfb106ed64f42a1c5d77f8`; `resource/tf_english.txt` `ad14a96e3c2cf5b77d81289985541f17d8013514aaf4e1a5088a2f1cbe84a578`; platform `resource/gameui_english.txt` `43730a193cec6892915532f809fa274397a122e827e75820514bced96f1f83f5`.
- One immutable current GameUI state and one typed user command or owner lifecycle notification.

## Outputs

- One immutable Main Menu, Loading, In Game, Pause, Disconnecting, or Failure state.
- At most one typed Console, Options, map load, resume, disconnect, quit, or external-link request. The model never executes a request.
- One explicit applied, ignored, inactive, or illegal disposition retaining the authoritative next state and request absence/presence.

## Invariants

- Matchmaking, community server, training, MvM, item, and store actions remain present with exact configured identity, order, English text, source command, and typed inactive capability.
- Browser navigation, DOM, network, matchmaking, account, economy, content loading, Simulation replacement, teardown, and process lifetime are absent.
- Loading progress never regresses. Repeated milestones advance only through their fixed interpolation cap; milestones without new status text preserve the prior text.
- Disconnecting returns to Main Menu only after owner-confirmed teardown. A disconnect command cannot publish completion.
- An illegal or inactive input preserves the identical state object and emits no request.

## Behavior Family

| Source/TF2 behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| The configured dashboard, playlist, override, and loading controls retain their order, text, source command, context, and owner capability. | Six immutable panel catalogs preserve the exact dashboard Main/Pause rows, conditional-first playlist, account/settings controls, hidden Store disposition, and loading Cancel control. | Configured-content hashes and complete 18-control panel snapshot comparison pass. | Ready |
| GameUI activation/hiding and level-loading notifications distinguish menu, hidden in-game, pause, loading, and failure presentation. | One transition function publishes exactly six discriminated immutable state kinds and rejects every event outside its legal source state. | Every configured button and every lifecycle event are crossed against all six state kinds; 690 assertions pass. | Ready |
| Console, Options, map, Resume, Disconnect, Quit, loading Cancel, and external links route effects to their owning systems. | Active commands emit one typed request; unavailable event/Casual/Competitive/MvM/community/training/items/store owners return typed inactive results with identical state and no request. | Request snapshots, inactive owner matrix, full command transcript, and absence-of-side-effect dependency scan pass. | Ready |
| Local map loading exposes exact map identity, milestone order, repeat interpolation, status retention, completion, cancellation, and primary/extended failure. | The model publishes 25 ordered milestones, fixed percentages and repeat caps, six configured status changes, exact map/failure byte bounds, cancel-to-disconnect, success, and failure. | Full milestone snapshot, 7/239/12-repeat vectors, saturation/regression/malformed inputs, success/failure/cancel, and status-retention tests pass. | Ready |
| Disconnect remains pending while the game/session owner tears down and returns to menu only after completion. | Pause Disconnect and loading Cancel enter immutable Disconnecting with origin/map identity; only `teardown-confirmed` publishes Main Menu. | Resume-hide, pause-disconnect, loading-cancel, duplicate command, early/wrong-state acknowledgement, and complete transcript tests pass. | Ready |
| Published state, configuration, requests, failures, and results are immutable and independent from DOM/product lifecycle. | The standalone module contains only frozen data, validation, request construction, and pure transitions; no concrete owner dependency exists. | Recursive freeze/input non-mutation tests, strict TypeScript 7.0.2 check, 11.36-KB browser bundle, diff check, and public-path privacy scan pass. | Ready |

## Ownership Exclusions

- Generic VGUI owns controls, resource interpretation, paint, focus, input routing, localization, fonts, and DOM adaptation.
- The TF2 UI resource descriptor owns content resolution, KeyValues composition, hashes, resource closures, and platform/language conditions.
- Applications own map catalogs, content acquisition, browser navigation, process/product lifecycle, request confirmation, and session teardown orchestration.
- Simulation and game/session owners load, activate, suspend, resume, disconnect, and destroy gameplay authority.
- Matchmaking, community-server browsing/creation, training, MvM, account items, and store/economy services are unavailable capabilities, not local substitutes.
