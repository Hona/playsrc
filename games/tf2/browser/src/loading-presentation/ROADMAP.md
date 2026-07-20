# TF2 Loading Presentation Roadmap

## Behavior Family

| Source/TF2 behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| TF2 installs the stats-summary panel behind GameUI's loading dialog and selects a map photo only when its material resolves. | Background resolution checks the mounted BSP PAK before the configured GAME providers, retains every checked location, and uses `stamp_background_map` when the map photo is absent. | Official SDK `src/game/client/tf/clientmode_tf.cpp` and `src/game/client/tf/vgui/tf_statsummary.cpp`; present/absent/malformed lookup tests. | Ready |
| The multiplayer PC dialog uses the configured no-banner resource, status label, progress bar, and Cancel command. | The adapter references the immutable loading resource document and emits modal VGUI operations for exact configured controls, status, owner progress, and lower-right geometry. | Configured-resource hashes plus 1,280×720 and 390×844 operation snapshots. | Ready |
| Progress follows owner milestones; repeated/regressed notifications do not create synthetic time progress. | Every loading state maps its current `progress` and `statusText` directly; repeated identical snapshots emit no operations and older generations are rejected. | Full `Tf2LoadingPhase` schedule, repeat, regression, and generation tests. | Ready |
| Cancel, success, disconnect, and generic failure change loading ownership and controls. | Cancel emits a typed disconnect request; in-game/main-menu/disconnecting remove the loading subtree; failure switches to the configured error document and Close action. | Lifecycle transcript and failure/destroy tests. | Ready |
