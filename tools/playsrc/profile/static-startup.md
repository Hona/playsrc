# Exact static startup gate

`bun run prepare:static-startup` prepares the three released-map **local candidate**
objects and static site without changing `releases/current.json` or uploading
anything. `bun run prepare:static-startup --approved` instead selects the committed
approved release and verifies its generated-binding closure and WASM interface.
The final JSON line names the package directory and exact WASM file.

Run the headed gate with those absolute paths:

```sh
PLAYSRC_STATIC_PACKAGE=/absolute/static-package \
PLAYSRC_STATIC_WASM=/absolute/selected.wasm \
PLAYSRC_PREVIOUS_STATIC_PACKAGE=/absolute/retained-previous-package \
bun run profile:static-startup
```

The previous directory must retain its original `tf2/index.html` and referenced
assets. The gate uses its entry once, then requires the application's one genuine
generation-recovery navigation to the candidate. It preserves the same browser
profile and verifies `stored` → `hit` for the application's compatible map cache;
it does not claim an HTTP-cache benchmark while request routing is active.

Only normal headed Chrome, an unlocked physical desktop, genuine two-second idle
admission, and the checked machine-wide lock are admitted. No autoplay override,
muted substitute, skipped movie, altered package, or reduced gameplay validation
is permitted. Both cold and warm-upgrade paths need advancing visible movie
pixels, Main Menu, and actual completed playable frames. A separate declared 503
configuration fixture must show the independent boot error UI.

Evidence and `startup-receipt.json` remain under configured `sourceCacheDir`.
Keep screenshots out of source control. The Release workflow requires the compact
receipt JSON for the **exact approved static package**. Deployment rejects missing
or mismatched receipts, and rechecks package bytes before infrastructure/deployment.
Source commits, matching export names, movie/menu-only success, and a newly rebuilt
Vite server are not substitutes for this acceptance. A candidate receipt does not
authorize uploading its objects or deploying it.
