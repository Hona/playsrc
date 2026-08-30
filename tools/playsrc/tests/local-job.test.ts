import { expect, test } from "bun:test"
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises"
import { randomUUID } from "node:crypto"
import os from "node:os"
import path from "node:path"
import { availableDevelopmentPort, localJobEnvironment, prepareLocalJob, readLocalTaskResult, runLocalJob, validateRevision } from "../src/local-job"
import { localJobCommand } from "../src/local-job-command"
import { resolveCargoExecutable } from "../src/tf2-wasm-build"

test("configured compiler paths do not depend on SSH PATH/PATHEXT discovery", async () => {
  expect(await resolveCargoExecutable(process.execPath, {})).toBe(process.execPath)
  await expect(resolveCargoExecutable(path.join(path.dirname(process.execPath), "missing-pinned-cargo.exe"), process.env)).rejects.toThrow("pinned Cargo")
})

test("the session bridge preserves normal interactive priority instead of the scheduler background default", async () => {
  const script = await readFile(path.resolve(import.meta.dir, "../windows-job.ps1"), "utf8")
  expect(script).toContain("New-ScheduledTaskSettingsSet -Priority 5")
  expect(script).toContain("processPriority=[string][Diagnostics.Process]::GetCurrentProcess().PriorityClass")
  expect(script).not.toMatch(/New-ScheduledTaskSettingsSet -Priority [0-3]\b/)
})

test("Playwright config arguments reach a Bun script without becoming Bun configuration", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-args-"))
  try {
    const script = path.join(directory, "probe.ts"), configuration = path.join(directory, "playwright.config.ts")
    await writeFile(configuration, "export default {}")
    await writeFile(script, 'console.log("entered");const server=Bun.serve({hostname:"127.0.0.1",port:0,fetch:()=>new Response(null)});await server.stop(true);console.log(JSON.stringify(process.argv.slice(2)))')
    const child = Bun.spawn([process.execPath, script, `--config=${configuration}`, "--project=ordinary"], { stdin: "ignore", stdout: "pipe", stderr: "pipe" })
    const [output, error, exit] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited])
    expect({ exit, error }).toEqual({ exit: 0, error: "" })
    expect(output).toContain(JSON.stringify([`--config=${configuration}`, "--project=ordinary"]))
  } finally { await rm(directory, { recursive: true, force: true }) }
}, 10_000)

test("both development ports can be rebound immediately by a separate native Bun process", async () => {
  for (let attempt = 0; attempt < 8; attempt++) {
    const port = await availableDevelopmentPort()
    const code = 'const p=Number(process.argv[1]);const servers=[p,p+1].map(port=>Bun.serve({hostname:"127.0.0.1",port,fetch:()=>new Response(null)}));await Promise.all(servers.map(server=>server.stop(true)));console.log("bound")'
    const child = Bun.spawn([process.execPath, "-e", code, String(port)], { stdin: "ignore", stdout: "pipe", stderr: "pipe" })
    const [stdout, stderr, exit] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited])
    expect({ exit, stdout: stdout.trim(), stderr }).toEqual({ exit: 0, stdout: "bound", stderr: "" })
  }
}, 15_000)

test("local jobs accept an explicit origin revision, never shell fragments or ambiguous branches", () => {
  for (const ref of ["refs/heads/work/fix", "refs/tags/v0.0.10", "a".repeat(40)]) expect(() => validateRevision(ref, "b".repeat(40))).not.toThrow()
  for (const ref of ["main", "--upload-pack=bad", "refs/heads/../../bad", "refs/heads/foo;bad", "refs/heads/foo.lock", "refs/heads/foo/"]) {
    expect(() => validateRevision(ref, "b".repeat(40))).toThrow()
  }
  expect(() => validateRevision("refs/heads/main", "HEAD")).toThrow()
})

test("local jobs reuse ordinary tests and headed profilers, without inherited remote routing", () => {
  expect(localJobCommand(["test", "tools/playsrc/tests/windows-desktop.test.ts"])).toEqual({ command: ["test", "tools/playsrc/tests/windows-desktop.test.ts"], interactive: false })
  expect(localJobCommand(["profile", "gameplay", "--headed"])).toEqual({ command: ["tools/playsrc/src/profile-runner.ts", "gameplay", "--headed"], interactive: true, controller: true })
  expect(localJobCommand(["prepare-profile", "gameplay"])).toEqual({ command: ["tools/playsrc/src/profile-prepare.ts", "gameplay"], interactive: false, controller: true })
  expect(localJobCommand(["build", "jump_beef"])).toEqual({ command: ["tools/playsrc/src/cli.ts", "dev", "jump_beef", "--prepare-only"], interactive: false })
  for (const args of [["--ready", "profile", "gameplay"], ["test", "../outside.test.ts"], ["test", "--preload=x"], ["profile", "gameplay", "--headless"], ["profile", "bad"], ["deploy"]]) {
    expect(() => localJobCommand(args)).toThrow()
  }
  expect(localJobEnvironment({ PATH: "native-tools", CARGO_HOME: "native-cargo", PLAYSRC_PROFILE_CDP_ENDPOINT: "remote", PLAYSRC_PROFILE_ORIGIN: "https://playsrc.online", PLAYSRC_DEV_PORT: "4173", PROFILE_SKIP: "1", NATIVE_ORIGIN: "remote", VITE_FOO: "bad" }, 49123))
    .toEqual({ PATH: "native-tools", CARGO_HOME: "native-cargo", PLAYSRC_DEV_PORT: "49123" })
})

test.skipIf(process.platform === "win32")("origin checkout is exact and isolated; ordinary test failures and mutations stay red (non-Windows fixture; Windows requires scheduled ownership)", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-local-job-"))
  const source = path.join(directory, "source"), origin = path.join(directory, "origin.git")
  const git = (args: string[], cwd = source) => {
    const result = Bun.spawnSync(["git", ...args], { cwd, stdout: "pipe", stderr: "pipe" })
    if (result.exitCode) throw new Error(result.stderr.toString())
    return result.stdout.toString().trim()
  }
  try {
    await mkdir(source)
    git(["init"])
    git(["config", "user.email", "test@example.invalid"])
    git(["config", "user.name", "Local Job Test"])
    await writeFile(path.join(source, "package.json"), '{"name":"job-fixture","private":true}')
    await writeFile(path.join(source, ".gitignore"), "playsrc.local.json\nnode_modules/\n")
    await writeFile(path.join(source, "pass.test.ts"), 'import {test,expect} from "bun:test"; test("native command",()=>expect(2+2).toBe(4))')
    await writeFile(path.join(source, "fail.test.ts"), 'import {test,expect} from "bun:test"; test("real failure",()=>expect(2+2).toBe(5))')
    const install = Bun.spawnSync([process.execPath, "install"], { cwd: source })
    expect(install.exitCode).toBe(0)
    git(["add", "."]); git(["commit", "-m", "fixture"])
    const commit = git(["rev-parse", "HEAD"])
    git(["branch", "-M", "fixture"])
    git(["clone", "--bare", source, origin])
    git(["remote", "add", "origin", origin])
    const config = { tf2Dir: path.join(directory, "tf"), sourceCacheDir: path.join(directory, "cache"), assetDir: path.join(directory, "assets") }
    for (const root of Object.values(config)) await mkdir(root)
    await writeFile(path.join(source, "playsrc.local.json"), JSON.stringify(config))
    await writeFile(path.join(source, "user-work.txt"), "do not touch")
    const job = await prepareLocalJob("refs/heads/fixture", commit, source)
    expect(git(["rev-parse", "HEAD"], path.join(job.directory, "checkout"))).toBe(commit)
    expect(await readFile(path.join(source, "user-work.txt"), "utf8")).toBe("do not touch")
    const passed = await runLocalJob(job.id, ["test", "pass.test.ts"], source)
    expect(passed.outcome).toBe("passed")
    expect(JSON.parse(await readFile(path.join(passed.run, "result.json"), "utf8")).commit).toBe(commit)
    const token = randomUUID(), launch = path.join(job.directory, `${token}-launch.log`), task = `playsrc-local-job-${token}`
    await writeFile(launch, "Error: rejected before a command started")
    expect(await readLocalTaskResult(job.directory, task)).toEqual({ result: null, launchError: "Error: rejected before a command started" })
    await writeFile(launch, Buffer.from("\uFEFF" + JSON.stringify({ ...passed, task }), "utf16le"))
    expect((await readLocalTaskResult(job.directory, task)).result.commit).toBe(commit)
    await writeFile(launch, JSON.stringify({ ...passed, id: randomUUID() }))
    expect((await readLocalTaskResult(job.directory, task)).result).toBeNull()
    await writeFile(launch, "")
    expect(await readLocalTaskResult(job.directory, task)).toEqual({ result: null, launchError: null })
    const link = path.join(job.directory, `${token}-run.json`)
    await writeFile(link, JSON.stringify({ job: job.id, task, run: passed.run }))
    await writeFile(path.join(passed.run, "result.json"), JSON.stringify({ ...passed, task }))
    await writeFile(launch, "duplicate task invocation rejected")
    expect((await readLocalTaskResult(job.directory, task)).result.run).toBe(passed.run)
    await writeFile(path.join(passed.run, "result.json"), JSON.stringify({ ...passed, task, run: path.join(job.directory, randomUUID()) }))
    expect((await readLocalTaskResult(job.directory, task)).result).toBeNull()
    await rm(link)
    await writeFile(path.join(passed.run, "result.json"), JSON.stringify(passed))
    expect((await runLocalJob(job.id, ["test", "fail.test.ts"], source)).outcome).toBe("failed")
    await mkdir(path.join(job.directory, "checkout", "node_modules"), { recursive: true })
    const nativeCache = path.join(job.directory, "checkout", "node_modules", "native-build-marker")
    await writeFile(nativeCache, "retain native outputs")
    await writeFile(path.join(source, "revision.txt"), "next commit")
    git(["add", "revision.txt"]); git(["commit", "-m", "next revision"]); git(["push", "origin", "fixture"])
    const nextCommit = git(["rev-parse", "HEAD"])
    expect((await prepareLocalJob("refs/heads/fixture", nextCommit, source, job.id)).id).toBe(job.id)
    expect(await readFile(nativeCache, "utf8")).toBe("retain native outputs")
    expect(JSON.parse(await readFile(path.join(passed.run, "result.json"), "utf8")).commit).toBe(commit)
    expect((await runLocalJob(job.id, ["test", "pass.test.ts"], source)).commit).toBe(nextCommit)
    await writeFile(path.join(job.directory, "running"), "another invocation")
    await expect(runLocalJob(job.id, ["test"], source)).rejects.toThrow()
    await rm(path.join(job.directory, "running"))
    await writeFile(path.join(job.directory, "checkout", "pass.test.ts"), "changed")
    expect((await runLocalJob(job.id, ["test"], source)).failure).toContain("changed")
    await expect(prepareLocalJob("refs/heads/fixture", "0".repeat(40), source)).rejects.toThrow()
    expect(git(["status", "--porcelain"])).toBe("?? user-work.txt")
  } finally { await rm(directory, { recursive: true, force: true }) }
}, 30_000)
