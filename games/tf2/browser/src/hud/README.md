# TF2 HUD Bindings

This module purely maps immutable TF2 gameplay/replay snapshots, the immutable player-class presentation setting, and ordered events to one configured class/model/health/ammo presentation, named HUD animations, presentation notifications, and typed weapon-selection/respawn/scoreboard requests. It contains no DOM, clock, randomness, settings persistence, gameplay mutation, replay advancement, resource parsing or animation execution.

HUD integration rejects a resource closure missing any dynamically selected class, team background, ammo background or health image; it never preserves the configured Scout image as a fallback. Locally owned stock weapons keep the foreign-item `CarryingWeapon` panel hidden.

HUD viewport replacement is one atomic VGUI commit. It recomputes configured proportional and bottom/right-relative geometry, reapplies dynamic bounds adjustments, and ignores duplicate viewport records.
