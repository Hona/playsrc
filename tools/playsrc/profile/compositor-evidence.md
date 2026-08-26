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
