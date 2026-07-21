# TF2 Startup Presentation

This browser-neutral module derives its content identity from `games/tf2/content-build.json`, validates the configured Valve startup media, fixes the PC startup plaque's material/texture and native 128×64 lower-right geometry, formats bounded download percentages, and coordinates exact once-per-process playback with hidden Main Menu preparation. A browser adapter supplies verified media bytes, byte progress, autoplay admission, visibility suspension, and the hidden VGUI owner.

Escape is the sole presentation skip request. The controller enters `AwaitingGesture` only when its media adapter returns `gesture-required`; the TF2 web adapter first retries denied audible autoplay as muted playback and unmutes on the first later input without restarting. Decode or playback failure enters `Failed`; no substitute animation or implicit failure skip exists.
