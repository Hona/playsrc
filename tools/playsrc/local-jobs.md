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

The unelevated, **Normal-priority** interactive scheduled session owns the UI,
never SSH/session 0. All workloads, including legacy Build and ordinary tests,
queue on the same checked machine-wide FIFO as profiles. The owner holds it
through consent, execution, child teardown and completion notification. Staged
builds/profiles borrow that checked live ownership instead of reacquiring it.
Prepared staged-build checkouts must include this ownership contract.

A real native message box identifies the action/profile, job, task and run:

- **Approve** dispatches immediately; **Deny**, close and Escape do not launch.
- No answer for **3 seconds after verified display** authorizes this job (AFK).
- Missing/hidden UI, a locked or mismatched session, helper failure, malformed
  receipts and stale identities **never** become timeout approval.
- One task token is consumed once, including scheduler retries. Use a new task
  for another attempt. Caller-supplied `--ready`/`-Ready` no longer exists.

The prompt is dismissed before browser admission. Approval is **not** evidence
of idle/foreground: the unchanged two-second genuine-idle and native
foreground/unobscured/browser-ownership checks still apply. No game window is
forced forward and no user input/app is suppressed. Direct Windows profile or
stage runners without native job ownership fail closed; use `local-job run`.

The total job budget remains **175 seconds**, including FIFO wait, helper
startup, prompts and cleanup (never more than three minutes waiting for a lock).
Sampling gets only the remaining budget, not reset clocks. Native commands are
created suspended, assigned to an owned kill-on-close Windows Job Object, then
resumed only after approval. Completion is shown only after the owned tree is
empty and source verification has finished: **completed**, **failed**,
**cancelled** or **denied**, with “hands-off is no longer needed for this job.”
The completion message also dismisses after three seconds. Native receipts
retain notification failures even when a desktop cannot display completion.

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

## Harmless native UI verification

```powershell
# Real prompt; a short native command, never a browser or performance sample.
powershell.exe -NoProfile -NonInteractive -File tools/playsrc/windows-job.ps1 -Job <job> -Action Diagnostic -Milliseconds 250 -DiagnosticExit 0
# Native control delivery against only an exact diagnostic dialog:
powershell.exe -NoProfile -NonInteractive -File tools/playsrc/windows-job-ui-test.ps1 -Job <job> -Case approve
# Cases: deny, close, escape, race, timeout, failure, cancel, queue.
```

The verifier records actual controls/pixels and asserts outcomes, immediate
dispatch, no overlapping queued dialogs, at-most-one launch and no live helper.
It cannot approve a performance workload. Synthetic receipt tests are separate
and are not native UI evidence or task authorization.
