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
bun tools/playsrc/src/local-job.ts run <job> build-stage resources jump_beef
bun tools/playsrc/src/local-job.ts run <job> profile gameplay
```

Preparation creates a detached checkout in `sourceCacheDir/local-jobs`, copies
only the three configured roots and installs frozen dependencies. It opens no
UI and never resets the developer's checkout. A stage prepares only its normal
artifact owner; the final ordinary build still verifies the complete closure.
Builds/tests open no browser.

## Windows consent and ownership

Every Windows `run` above returns a scheduled task identity immediately. The
same bridge is available explicitly:

```powershell
powershell.exe -NoProfile -NonInteractive -File tools/playsrc/windows-job.ps1 -Job <job> -Profile gameplay
# Also: -Action Build -Target jump_beef
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

Only a validated headed profile uses the interactive scheduled session and UI.
The same workload classifier is checked before scheduling, by the launcher and
at native dispatch; neither a caller UI flag nor a substituted command can
reclassify a profile. Unknown work fails closed without a dialog. Stages and
profiles borrow checked live ownership rather than recursively acquiring it.
Both the isolated controller and prepared checkout must include this contract.

For an interactive profile, a real native message box identifies the action,
job, task and run:

- **Approve** dispatches immediately; **Deny**, close and Escape do not launch.
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
resumed after validated classification (and displayed approval for a profile).
Only a successfully completed interactive job gets a completion notification,
after its owned tree is empty and source verification finishes. It dismisses
after three seconds. Denial, failure, cancellation, preflight errors and helper faults are logs-only;
there is no failure-only notification helper or retry. Background receipts
require `interactive: false`, null consent/completion and zero UI invocations.

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
