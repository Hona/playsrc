# Retained compositor evidence

Upward Training, particle-combat, and class-switch profiles always retain local cross-process Chromium traces, including captures without incidents. Use the existing machine-wide locked, visibly headed commands with a 5–10 second sample; no renderer or simulation settings change.

Evidence lives under `sourceCacheDir/profiles/upward-training-bots/compositor-evidence/`. Each report and `PLAYSRC_COMPOSITOR_EVIDENCE` log identifies a SHA-256 manifest linking the original compressed trace and browser probes. Labels never overwrite these immutable artifacts. Nothing is uploaded automatically; traces can contain local URLs and browser diagnostics.

Replay and verify all three hashes without opening a browser:

```sh
bun tools/playsrc/profile/compositor-evidence.ts /path/to/<sha256>.manifest.json
```

The manifest pins source fingerprints before/after capture, commit, application/Worker/WASM/content generations, browser revision, adapter, physical viewport, categories, clock anchors, and native event indices. GPU operations preserve native promise ownership and record synchronous return separately from asynchronous completion. Missing event families remain explicitly absent, not inferred.

Exact in-page marks join page milliseconds to Chromium microseconds. Incidents strictly over 50/100/250 ms include sample, boundary, and post-sample collection gaps; nothing is deleted to meet a budget. `compositor` measures only the marked gameplay window. `compositorIncludingSetupAndCollection` separately reports the wider CDP window with its own duration. Overlapping work and probes are correlation, **not an automatic root-cause verdict**.

Offline replay also derives `attribution` without changing the retained v1 analysis. It reports gameplay percentiles and separate threshold counts per presentation stream/scope, per-thread union coverage (never summed nested slices), captured native/script source locations, exact native postMessage dispatch-ID edges, and phase/resource probe details. GPU synchronous return is separate from promise settlement. Worker round trips and completed-frame phase totals are not reconstructed into invented stage timestamps. Missing coverage is **unobserved**, not idle; dispatch edges alone do not prove presentation blocking. An unexplained gap retains a null critical path. Source ownership belongs to the manifest's build, not the replay checkout. Detail truncation is explicit; raw indices still address the original evidence.

Bounds per capture: 128 MiB browser trace buffer, 32 MiB compressed stream, 256 MiB decoded JSON, one million events, 32 MiB browser probes, and 16,384 GPU operation probes. Detailed joins cover the 64 largest incidents; counts cover all incidents and the complete raw trace remains available. Stream draining is limited to 15 seconds after sampling. Overflow, missing/drifting marks, changed source, and Chromium data loss produce retained incomplete evidence and a failing profile, never a clean performance claim.

# Worker CPU/task evidence

The all-18-edge `class-switch-high-dpi` path also samples the actual gameplay
Worker through its own CDP target. Its content-addressed `workers.json` artifact
is linked from the compositor manifest. Replay it without another browser run:

`bun tools/playsrc/profile/worker-incident-attribution.ts <sha256.manifest.json>`

Paired Worker marks join monotonic CPU samples to native process/thread/task IDs,
request/response IDs, queue depth, synchronous postMessage duration, transaction
timings, authoritative selected ticks/event batches, browser decode time, GC, and
allocation-counter deltas. No snapshot or WASM memory view is retained. Records
are bounded and missing clocks, overflow, or sampler deadlines are not clean
evidence. Rayon helpers are listed as unsampled; they can remain synchronously
parked in WASM. Sampling is diagnostic overhead, not an optimization benchmark.
Task overlap is not serialization cost or proof of historical incident causation.
Boundary tasks retain their full duration and separately report sample overlap.
The class profile also gates actual gameplay silence inside presentation pairs
that cross sample boundaries. Stopping capture cannot hide the already elapsed
gameplay portion of a stall; collection-only portions remain separately labeled.

## Authoritative gameplay replay

The all-class profile can retain a bounded 4 MiB / 16,384-record Rust-owner journal. Its checkpoint is the deterministic compiled-map initial state, identified by the exact configured BSP, collision world, and resource set; every subsequent team/position/course mutation and admitted host command is recorded from construction onward. Sample markers separate the construction prefix. Merged per-tick commands, complete tick/event hashes, host publication hashes, and snapshot acknowledgements are retained without asset bytes, heap dumps, or engine references.

`bun run replay:gameplay <replay-manifest-sha256>` builds the opt-in CPU/WASM diagnostic with the configured toolchain and verifies the fresh transcript against direct convex sweeps and retained acceleration. `--ticks` also replays the recorded authoritative commands in 2/3/4/6-tick work groups, without changing simulated time or event order. This is not a browser-frame benchmark or a reconstruction of older traces that lack commands. The direct-sweep reference shares the current object hierarchy; counts distinguish object candidates, convex clippings, clip-plane visits, and vertex projections.

Incremental journals and compressed native stream prefixes survive incomplete capture; missing boundaries, loss, overflow, failed reads, or timeout remain failures. Their local progress files never certify a passing measurement. No artifacts are uploaded automatically.

Windows local runs require the runner to be in the active, unlocked physical console session before server preparation and browser launch. `bun tools/playsrc/profile/windows-desktop.ts` reads WTS session evidence without changing settings or starting a browser; unknown, locked, remote, and session-zero states fail closed. This preflight is not proof that the display stays visible: the gameplay visibility/focus checks still apply. Remote CDP adapters must establish that gate on the browser host.

Upward/class-switch/combat reports include bounded, exact-PID process-memory snapshots outside the marked sample. Windows reports working set and private committed bytes separately; macOS/Linux report RSS, not invented private bytes. These are boundary values, **not peaks or private working sets**. Missing/exited processes make totals null and retain an error. Remote CDP never looks up browser PIDs on the runner's machine.
