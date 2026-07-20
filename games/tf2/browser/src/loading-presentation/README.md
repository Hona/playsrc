# TF2 Loading Presentation

This module resolves TF2 map-loading image layers and converts `Tf2GameUiState` into immutable VGUI-resource operations. It uses the configured no-banner multiplayer dialog and owner-reported progress without timers, interpolation, or app-owned percentages.

The mounted BSP PAK is checked first for a map photo. Authoritative absence keeps the configured `stamp_background_map`; it never fabricates a level screenshot. The module emits requests and presentation state only and never loads or tears down gameplay.
