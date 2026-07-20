# TF2 Startup Presentation

This browser-neutral module derives its content identity from `games/tf2/content-build.json`, validates the configured Valve startup media, and coordinates exact once-per-process playback with hidden Main Menu preparation. It emits lifecycle state only; a browser adapter supplies verified media bytes, autoplay/gesture admission, visibility suspension, and the hidden VGUI owner.

Escape is the sole presentation skip request. Autoplay denial enters `AwaitingGesture`; decode or playback failure enters `Failed`. No UI overlay, substitute animation, muted-success path, or implicit failure skip exists.
