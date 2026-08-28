# Frame delivery, not just render work

`playwright.delivery-profile.config.ts` provides separate `ordinary` and `traced`
projects for the same authored 15-bot Upward practice entry and forward movement.
Run each separately through the managed profiler's normal `--config` / `--project`
options. Do not run both projects in one three-minute command.

The ordinary project enables no application/Worker profiling globals, CPU/heap
samplers or Chromium trace. A read-only observer records existing completed-frame
attributes and RAF callback delivery times. Its compositor presentation field is
explicitly unmeasured. The traced project uses the existing instrumentation and
independent native compositor trace. Both retain physical-window/input checks.

The managed runner accepts `--application-root <absolute-checkout>` before the
profile name. The application server, source/content/generated-WASM checks and
Playwright working directory belong to that unchanged checkout; the invoking
checkout supplies the harness. Both source identities are checked. This permits
paired tests of one exact main commit without editing its gameplay or test files.

Compare source commit, browser, actual canvas resolution/DPR, camera, configured
content/render level, persisted options, active roster and input policy before
interpreting rates. Simulation ticks and moving bot positions remain outcomes,
not frozen controls. Do not infer production throughput from a local traced run.

Report v4 calls render-submission phase wall time `renderSubmissionElapsed` and the awaited
model transaction `modelPreparationLatency`; neither is a frame interval or bot
CPU cost. Submission elapsed time also includes any waits within that phase.
Delivery timelines retain full 250ms/1s empty/partial buckets, gap percentiles,
50/100/250/500/1000ms exceedances and start/end silence.
Known compositor streams with no in-window events report zero, not unavailable;
independent streams cannot be interleaved into a fabricated faster frame rate.
