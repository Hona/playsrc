# Run the same checkout locally or on Windows

The Windows development environment must already work (`git`, Bun, Node,
configured TF2 content and toolchains). No remote browser service is needed.
SSH transports commands and reads files. Git transports source. Task Scheduler
is used only when a headed command must cross from SSH into the user's existing
interactive console.

From a configured checkout, prepare any commit reachable from an origin branch:

```sh
bun tools/playsrc/src/local-job.ts prepare refs/heads/my-branch <40-character-commit>
```

This makes a separate detached checkout in the configured `sourceCacheDir`,
copies only the three local configuration roots and runs `bun install
--frozen-lockfile`. It never modifies the original checkout, downloads a different
toolchain, or transfers generated browser/WASM artifacts from another host.
Keep the returned job ID. Preparation opens no window.

To advance that same dedicated checkout, pass its ID as the third argument to
`prepare`. A clean, idle job can switch to the next exact commit without deleting
its native compiler cache. Previous run receipts retain their original commits.
An interrupted dependency installation must finish before the next run.

Prebuild before requesting the short interactive window:

```sh
bun tools/playsrc/src/local-job.ts run <job-id> build jump_beef
```

This uses normal `bun dev jump_beef --prepare-only`: build, verify local server
readiness, close. It opens no browser. Cold compiler work does not need to consume
the gameplay capture window.

Run ordinary tests, directly or through SSH:

```sh
bun tools/playsrc/src/local-job.ts run <job-id> test tools/playsrc/tests/windows-desktop.test.ts
```

In a physical Windows terminal, after a fresh hands-off agreement:

```sh
bun tools/playsrc/src/local-job.ts run <job-id> --ready profile gameplay
```

From SSH, the thin session bridge runs that same command:

```powershell
powershell.exe -NoProfile -File tools/playsrc/windows-job.ps1 -Job <job-id> -Profile gameplay -Ready
```

The launch returns immediately. `-Action Status -Job <job-id>` and `-Action Logs
-Job <job-id>` return a current snapshot or log tail immediately; there is no SSH
wait loop. Helper consoles stay hidden, but the game browser is always headed.

`--ready`/`-Ready` is explicit authorization for this attempt, not a way to
override desktop admission. The ordinary profiler still owns the machine lock,
physical-console checks, headed browser, input/window guards, server, deadlines
and evidence. A free loopback port avoids the developer's existing server.
Profiles use native local builds and localhost, never a production URL, remote
CDP connection, request-interception broker or controller-host asset relay.

Read `command.log` and `result.json` in the returned run directory over SSH/SCP;
ordinary profiler evidence stays in its normal configured cache directory.
Exit failures and source/configuration changes fail the job. Overlapping runs
in one checkout are rejected. A forcibly interrupted job leaves its `running`
marker for inspection; do not remove it until its processes have stopped.
The next status read removes the recorded completed task from the launching
account; the deliberately unelevated task does not unregister itself. A forced
termination may require removing that exact returned task name after inspection.
Do not delete another job's task, locks, browser profiles or user processes.
