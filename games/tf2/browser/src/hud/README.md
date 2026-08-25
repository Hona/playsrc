# TF2 HUD Bindings

This module purely maps immutable TF2 gameplay/replay snapshots, the immutable player-class presentation setting, and ordered events to one configured class/model/health/ammo presentation, named HUD animations, presentation notifications, and typed weapon-selection/respawn/scoreboard requests. It contains no DOM, clock, randomness, settings persistence, gameplay mutation, replay advancement, resource parsing or animation execution.

HUD integration rejects a resource closure missing any dynamically selected class, team background, ammo background or health image; it never preserves the configured Scout image as a fallback. Locally owned stock weapons keep the foreign-item `CarryingWeapon` panel hidden.

HUD viewport replacement is one atomic VGUI commit. It recomputes configured proportional and bottom/right-relative geometry, reapplies dynamic bounds adjustments, and ignores duplicate viewport records.

Crosshair presentation resolves the encrypted active-weapon atlas icon or one of eight exact paired authored custom VMT/VTF styles, applies Source stock/custom centering and scale, multiplies the authored pixel color and alpha by archived RGB, and immediately respects observer, weapon, pause, console, menu, and viewport state. Multiplayer preview animates authored frames without committing draft values; the same archived settings authority accepts unbounded manual crosshair ConVars and preserves Apply/Cancel/reset semantics. Chronological ammo validation also retains fire→regenerate and regenerate→fire publications.
