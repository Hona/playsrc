# TF2 GameUI Model

This module owns the immutable TF2 Main Menu, Loading, In Game, Pause, Disconnecting, and Failure state model. It preserves the configured command catalog and emits typed requests without DOM, browser navigation, matchmaking, store, network, map-loader, Simulation, or product-lifecycle authority.

Import `./model` for the state, command, transition, menu, and loading contracts. Run `bun test games/tf2/browser/tests/gameui` from the repository root.
