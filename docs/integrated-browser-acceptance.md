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
