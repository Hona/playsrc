# Integrated browser acceptance

`bun run profile:acceptance <scenario>` runs one visibly headed Edge scenario
under the shared machine lock and three-minute deadline. Scenarios:
`training-dpr1`, `training-dpr1.25`, `training-dpr1.5`, `training-dpr2`, `2fort`,
`engineer`, `lifecycle`. Sampling is 5–10 real seconds. No quality, bot count,
simulation clock, or pointer-lock emulation is substituted.

Training uses the authored 16-total-player default, cold RED admission and warm
BLU admission, a nine-class native-input cycle, then stock-slot input/screenshots.
The native-input cycle respects Chromium's unlock/relock rate limit; its real
cooldown frames remain in the sample. Stock correctness pacing is outside it.
2Fort uses 23 bots and checks a 12v12 roster. Lifecycle preserves saved settings,
checks HDR across two live map replacements, and samples overhead water.

Reports, screenshots, full Chromium traces and main/Worker CPU profiles remain
in the configured source cache. Trace timestamps, not RAF or queue submission,
measure compositor presentation. Censored start/end gaps are reported separately
and count against the stall gate. First-playable startup timing currently means
an application-completed frame, **not** compositor presentation. RSS is unavailable
when the browser runs on a different host unless
`PLAYSRC_PROFILE_PROCESS_MEMORY_EXECUTABLE` supplies that host's PID-bound byte
counts; local PIDs must never be mistaken for remote browser PIDs.

`bun run profile:acceptance compare before.json after.json` refuses an improvement
claim when browser, GPU, viewport/DPR, settings, cache path or roster differ, or
comparison metadata is missing. A passing Mac sample is not Windows acceptance
and does not resolve an older seconds-per-frame incident. Short runs also do not
establish long-session memory stability. Windows acceptance additionally requires
an independently verified unlocked interactive desktop.

## Retained collision-tail observations

Both runs below used local Edge on Apple M4/Metal (26.5.2), 1280×720 CSS,
DPR 2 / 2560×1440 buffer, the unchanged balanced LDR settings, cold Training
Upward, 15 bots plus the local player (8v8), and the same 18-selection stress
sequence over 10 seconds. This older click-stress sequence is **not** the newer
nine-class native-input acceptance cycle. Detailed bot-class inventory was not
retained by the before fixture, so strict comparison correctly refuses a full
non-regression verdict.

| Observation | Before lazy support projection | After |
| --- | ---: | ---: |
| Presented interval p95 / p99 / max, ms | 16.668 / 33.334 / 149.998 | 16.667 / 33.334 / 133.332 |
| Worker simulation transaction p95 / p99 / max, ms | 7.485 / 22.490 / 168.435 | 4.100 / 6.160 / 14.525 |
| Observed simulation Hz | 62.200 | 66.577 |
| First application-completed playable frame, ms | 8865.340 | 8830.635 |
| Queue writes, bytes (whole sample) | 118747932 | 128360396 |
| Process RSS growth, bytes (whole sample) | 525959168 | 561315840 |

Evidence labels: `integrated-worker-diagnostic-dpr2` and
`integrated-fixed-comparable-dpr2`. More completed frames produce more total
uploads; the RSS growth is retained, not hidden or presented as a memory win.
The initial `integrated-baseline-dpr2` incident had a 366.657 ms presented gap;
its raw trace remains retained. None of these observations resolves the older
505 ms incident, proves long-session memory stability, or predicts Windows.
