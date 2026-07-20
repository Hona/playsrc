# TF2 Startup Presentation Roadmap

## Behavior Family

| Source/TF2 behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| Process startup reads the ordered startup-video list, applies startup suppression policy, and plays each admitted movie once. | One descriptor derives build and provider revisions from `games/tf2/content-build.json`, fixes the selected `media/valve.bik` source and configured browser `media/valve.webm` representation by byte identity, and starts at most once while reporting policy suppression explicitly. | Descriptor identity and malformed vectors pass; installed-Edge acceptance admits audible playback, captures frames after 4.9 and 9.75 seconds, requires `Completed`, reveals Main Menu, and loads `jump_beef`. | Ready |
| Startup playback owns visible presentation while GameUI initializes behind it. | Hidden Main Menu preparation begins with media preparation, remains non-interactive and accessibility-hidden, and is revealed exactly once only after playback completion or admitted Escape skip. | Cold/warm menu schedules and exposure-call transcript tests. | Ready |
| Playback completion, Escape, decode failure, autoplay admission, and teardown are distinct outcomes. | The lifecycle publishes `NotStarted`, `Preparing`, `AwaitingGesture`, `Playing`, `WaitingForMenu`, `Skipped`, `Completed`, `Failed`, and `Destroyed`; failures never become completion. | Boundary, stale callback, failure, gesture, visibility, and repeated-destroy tests. | Ready |
| Startup teardown releases movie and hidden GameUI ownership. | Destroy invalidates the generation and destroys media plus prepared/pending menu ownership exactly once. | Destroy-before/after-prepare and stale-completion tests. | Ready |
