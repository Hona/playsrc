# TF2 HUD Bindings

This module purely maps immutable TF2 gameplay/replay snapshots, the immutable player-class presentation setting, and ordered events to one configured class/model/health/ammo presentation, named HUD animations, presentation notifications, and typed weapon-selection/respawn/scoreboard requests. It contains no DOM, clock, randomness, settings persistence, gameplay mutation, replay advancement, resource parsing or animation execution.

HUD integration rejects a resource closure missing any dynamically selected class, team background, ammo background or health image; it never preserves the configured Scout image as a fallback.
