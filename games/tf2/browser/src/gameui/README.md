# TF2 GameUI Model

This module owns the immutable TF2 Main Menu, Loading, In Game, single-dashboard Pause, Disconnecting, and Failure state model. Its integration presents the build-selected opaque `background_2fort` standard/widescreen BasePanel image beneath the independently selected character and proportional override, hides it outside the disconnected Main Menu, retains full-viewport bounds across aspect/DPR changes, preserves the configured command catalog, and emits typed requests without browser navigation, matchmaking, store, network, map-loader, Simulation, or product-lifecycle authority.

Import `./model` for the state, command, transition, menu, and loading contracts. Run `bun test games/tf2/browser/tests/gameui` from the repository root.
