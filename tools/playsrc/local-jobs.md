# Native delegated jobs

Use an already configured checkout (`git`, Bun, Node and the configured native
content/toolchains). SSH transports commands/files; Git transports source. No
remote browser, installation discovery, security changes or production routing.

```sh
bun tools/playsrc/src/local-job.ts prepare refs/heads/my-branch <40-character-commit>
# Optional final argument: an existing idle job ID, to retain its compiler cache.
bun tools/playsrc/src/local-job.ts run <job> test tools/playsrc/tests/windows-desktop.test.ts
bun tools/playsrc/src/local-job.ts run <job> build jump_beef
bun tools/playsrc/src/local-job.ts run <job> build-stage wasm
bun tools/playsrc/src/local-job.ts run <job> build-stage producer
bun tools/playsrc/src/local-job.ts run <job> build-stage browser
bun tools/playsrc/src/local-job.ts run <job> build-stage resources jump_beef
bun tools/playsrc/src/local-job.ts run <job> profile gameplay
# Explicitly prepare cold profiles without a browser in a separate bounded task:
bun tools/playsrc/src/local-job.ts run <job> prepare-profile gameplay
# Wait for that task, then use the same ordinary profile command above.
```

Preparation creates a detached checkout in `sourceCacheDir/local-jobs`, copies
only the three configured roots and installs frozen dependencies. It opens no
UI and never resets the developer's checkout. A stage prepares only its normal
artifact owner; the final ordinary build still verifies the complete closure.
Builds/tests open no browser. The browser stage installs only the Chromium and
supporting binaries selected by the pinned Playwright package and records the
executable hash; it never launches a browser or selects a fallback channel.

## Windows consent and ownership

Every Windows `run` above returns a scheduled task identity immediately. The
same bridge is available explicitly:

```powershell
powershell.exe -NoProfile -NonInteractive -File tools/playsrc/windows-job.ps1 -Job <job> -Profile gameplay
# Also: -Action Build -Target jump_beef
#       -Action PrepareProfile -Profile gameplay
#       -Action BuildStage -Stage resources -Target jump_beef
#       -Action Test -TestArguments '["tools/playsrc/tests/windows-desktop.test.ts"]'
```

All workloads, including preparation, builds and ordinary tests, queue on the
same checked machine-wide FIFO as profiles. Lock ownership is **not** UI consent.
Builds, build stages, tests and CLI diagnostics use an unelevated Normal-priority
S4U scheduled task: no interactive console, unlocked desktop, mouse/keyboard
idle, consent dialog or completion/failure/cancellation message is required.
Errors still fail the job and remain in results/logs. Preparation and readback
are also silent. Background never means permission to run a headless browser.

Only a validated headed profile uses the interactive scheduled session. A profile
is **composite**, not an all-interactive command: its owned native process starts
silently, borrows the existing FIFO resource reservation, authenticates source,
content and generated inputs, builds/prepares its normal development owner,
starts non-GUI servers and resolves/hashes the browser executable **before** UI.
Preparation failure, cancellation or insufficient remaining browser budget is
silent. No caller readiness flag or manual build-command sequence is needed.
For a cold workload, `PrepareProfile` runs the complete normal non-GUI preparation
in its own 175-second task; it never opens/preloads a browser. A later `Run`
revalidates all inputs and keeps its own unchanged 175-second bound. The result
links the exact preparation task/run and reports full preparation-to-finish wall
time, including queue and the gap between commands. Visible admission/loading
still belongs to the approved interval and is never subtracted from that run.
The same workload classifier is checked before scheduling, by the launcher and
at native dispatch; neither a caller UI flag nor a substituted command can
reclassify a profile. Unknown work fails closed without a dialog. Stages and
profiles borrow checked live ownership rather than recursively acquiring it.
Both the isolated controller and prepared checkout must include this contract.
Configuration/browser/tsconfig overrides, UI/debug modes and browser-opening or
executable reporters cannot replace the owned stage interface. Read-only
`--list`/help requests remain background-only; selection and non-GUI reporters
still use the registered profile.

For an interactive profile, a real native message box identifies the action,
job, task and run:

- **Approve** begins browser admission immediately; **Deny**, close and Escape do not launch a browser.
- No answer for **3 seconds after verified display** authorizes this job (AFK).
- Missing/hidden UI, a locked or mismatched session, helper failure, malformed
  receipts and stale identities **never** become timeout approval.
- One task token is consumed once, including scheduler retries. Use a new task
  for another attempt. Caller-supplied `--ready`/`-Ready` no longer exists.

Only the requested message box is topmost, with one ordinary activation attempt.
Its visible native window is checked independently of foreground focus, so the
user can keep typing in another app without turning non-response into a fault.

The prompt is dismissed before browser admission. Approval is **not** evidence
of idle/foreground: the unchanged two-second genuine-idle and native
foreground/unobscured/browser-ownership checks still apply. No game window is
forced forward and no user input/app is suppressed. Direct Windows profile or
stage runners without native job ownership fail closed; use `local-job run`.

Windows 10 or newer is required for atomic Job Object assignment. The total
job budget remains **175 seconds**, including FIFO wait, helper
startup, prompts and cleanup (never more than three minutes waiting for a lock).
Sampling gets only the remaining budget, not reset clocks. Native commands are
created suspended **in** an owned kill-on-close Windows Job Object, then
resumed silently after validated classification. Only its prepared browser stage
can request displayed approval; there is no second lock acquisition or queue
between approval and browser admission.
The native supervisor retains the same kill-on-close process tree throughout
preparation, consent, browser use and background extraction. `ownership.json`
(`PLAYSRC_LOCAL_JOB_OWNER`) authenticates this full lifetime, **not** permission
to use the desktop. Each single-use `desktop/<ordinal>/grant.json` binds a prepared-input
hash and exact child/helper creation identities to this task/run/stage. Changed
inputs refuse launch; the stage cannot be reused by another helper or retry.

Only a successfully completed interactive **stage** gets a completion notification,
after the actual browser/input teardown and `desktop/<ordinal>/released.json`, before slow
background extraction, source verification and report retention. The resource
reservation remains held so extraction cannot contaminate another measurement;
it is not a desktop lease. The notification dismisses after three seconds.
Denial, failure, cancellation, preflight errors and helper faults are logs-only;
there is no failure-only notification helper or retry. Background receipts
require `interactive: false`, an empty `desktop` list and zero UI invocations.

Mac Demoman profiles use a native click for pointer capture: Chromium's emulated
DOM focus does not establish the native content view as first responder. The
helper requires existing event-posting access and rechecks the exact foreground,
unobscured window before clicking. It never activates a window or substitutes a
pointer-lock result. Aim coordinates come from the resulting trusted click.

Profile authors use the existing application fixture and `profileArtifact` for
post-browser analysis/retention. Its worker teardown closes test contexts, the
profile owner closes the real browser, and the native supervisor acknowledges
desktop release before those closures run. Background extraction cannot reopen
the retired endpoint. Later Playwright workers/projects request their own fresh
browser stage; CLI/module initialization and each worker's browser preparation
are silent. The job's original deadline and FIFO ownership span all stages.
`profile runner-handoff` is a short real headed runner
check, with no gameplay benchmark or content build.

## Readback and cancellation

```powershell
powershell.exe -NoProfile -NonInteractive -File tools/playsrc/windows-job.ps1 -Job <job> -Task <returned-task> -Action Status
# -Action Result | Logs | Wait | Artifacts | Cancel
```

These calls never request consent or create a window. Cancel addresses only the
recorded task/run, including a queued task; it does not kill a user process.
Readback binds to that task, never a newer/older job's successful log. Collection
includes source/command identity, native decision, creation-time identities,
helper faults, owned teardown and completion records. Diagnostic pixels stay
in configured cache storage, not source control or automatic artifact uploads.
The next status call retires a finished task from the launching account.

A forcibly interrupted process can leave `running`. Inspect its exact task and
process identities first. `-Action Recover` preserves an interruption record
and clears the marker only when that failed task has no live job process or
completed result. Never delete another task's lock, browser or user processes.

## Silent native regression checks

```powershell
# Short background command; ZERO dialogs, never a browser/performance sample.
powershell.exe -NoProfile -NonInteractive -File tools/playsrc/windows-job.ps1 -Job <job> -Action Diagnostic -Milliseconds 250 -DiagnosticExit 0
# From the noninteractive transport, not inside an already locked job:
bun test tools/playsrc/tests/windows-job-background.test.ts tools/playsrc/tests/windows-job-session.test.ts tools/playsrc/tests/windows-job-lifecycle.test.ts
```

These exercise actual background exit/cancellation/helper-crash cleanup and
interactive session-zero rejection without dialogs. The lifecycle fixture
checks every branch using isolated test-only receipts, not UI evidence or
authorization. The former every-job diagnostic UI launcher has been removed.
