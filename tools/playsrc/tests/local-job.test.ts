import { expect, test } from "bun:test"
import { cp, mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises"
import { randomUUID } from "node:crypto"
import { execFileSync } from "node:child_process"
import os from "node:os"
import path from "node:path"
import { localJobCommand, localJobEnvironment, prepareLocalJob, runLocalJob, validateRevision } from "../src/local-job"
import { resolveCargoExecutable } from "../src/tf2-wasm-build"

test("configured compiler paths do not depend on SSH PATH/PATHEXT discovery", async () => {
  expect(await resolveCargoExecutable(process.execPath, {})).toBe(process.execPath)
  await expect(resolveCargoExecutable(path.join(path.dirname(process.execPath), "missing-pinned-cargo.exe"), process.env)).rejects.toThrow("pinned Cargo")
})

test("local jobs accept an explicit origin revision, never shell fragments or ambiguous branches", () => {
  for (const ref of ["refs/heads/work/fix", "refs/tags/v0.0.10", "a".repeat(40)]) expect(() => validateRevision(ref, "b".repeat(40))).not.toThrow()
  for (const ref of ["main", "--upload-pack=bad", "refs/heads/../../bad", "refs/heads/foo;bad", "refs/heads/foo.lock", "refs/heads/foo/"]) {
    expect(() => validateRevision(ref, "b".repeat(40))).toThrow()
  }
  expect(() => validateRevision("refs/heads/main", "HEAD")).toThrow()
})

test("local jobs reuse ordinary tests and headed profilers, without inherited remote routing", () => {
  expect(localJobCommand(["test", "tools/playsrc/tests/windows-desktop.test.ts"])).toEqual({ command: ["test", "tools/playsrc/tests/windows-desktop.test.ts"], interactive: false })
  expect(localJobCommand(["profile", "gameplay", "--headed"])).toEqual({ command: ["tools/playsrc/src/profile-runner.ts", "gameplay", "--headed"], interactive: true })
  expect(localJobCommand(["build", "jump_beef"])).toEqual({ command: ["tools/playsrc/src/cli.ts", "dev", "jump_beef", "--prepare-only"], interactive: false })
  for (const args of [["test", "../outside.test.ts"], ["test", "--preload=x"], ["profile", "gameplay", "--headless"], ["profile", "bad"], ["deploy"]]) {
    expect(() => localJobCommand(args)).toThrow()
  }
  expect(localJobEnvironment({ PATH: "native-tools", CARGO_HOME: "native-cargo", PLAYSRC_PROFILE_CDP_ENDPOINT: "remote", PLAYSRC_PROFILE_ORIGIN: "https://playsrc.online", PLAYSRC_DEV_PORT: "4173", PROFILE_SKIP: "1", NATIVE_ORIGIN: "remote", VITE_FOO: "bad" }, 49123))
    .toEqual({ PATH: "native-tools", CARGO_HOME: "native-cargo", PLAYSRC_DEV_PORT: "49123" })
})

test("origin checkout is exact and isolated; ordinary test failures and mutations stay red", async () => {
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
    if (process.platform === "win32") {
      await mkdir(path.join(source, "tools", "playsrc"), { recursive: true })
      await cp(path.resolve(import.meta.dir, "../windows-job.ps1"), path.join(source, "tools", "playsrc", "windows-job.ps1"))
    }
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
    await expect(runLocalJob(job.id, ["profile", "gameplay"], false, source)).rejects.toThrow("hands-off")
    const passed = await runLocalJob(job.id, ["test", "pass.test.ts"], false, source)
    expect(passed.outcome).toBe("passed")
    expect(JSON.parse(await readFile(path.join(passed.run, "result.json"), "utf8")).commit).toBe(commit)
    if (process.platform === "win32") {
      const token = randomUUID()
      await writeFile(path.join(job.directory, `${token}-launch.log`), "Error: rejected before a command started")
      const status = execFileSync("powershell.exe", ["-NoProfile", "-NonInteractive", "-File", path.join(source, "tools", "playsrc", "windows-job.ps1"), "-Action", "Result", "-Job", job.id, "-Task", `playsrc-local-job-${token}`], { timeout: 10_000, windowsHide: true, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] })
      const observed = JSON.parse(status)
      expect(observed.result).toBeNull()
      expect(observed.launchError).toContain("rejected before a command")
    }
    expect((await runLocalJob(job.id, ["test", "fail.test.ts"], false, source)).outcome).toBe("failed")
    await mkdir(path.join(job.directory, "checkout", "node_modules"), { recursive: true })
    const nativeCache = path.join(job.directory, "checkout", "node_modules", "native-build-marker")
    await writeFile(nativeCache, "retain native outputs")
    await writeFile(path.join(source, "revision.txt"), "next commit")
    git(["add", "revision.txt"]); git(["commit", "-m", "next revision"]); git(["push", "origin", "fixture"])
    const nextCommit = git(["rev-parse", "HEAD"])
    expect((await prepareLocalJob("refs/heads/fixture", nextCommit, source, job.id)).id).toBe(job.id)
    expect(await readFile(nativeCache, "utf8")).toBe("retain native outputs")
    expect(JSON.parse(await readFile(path.join(passed.run, "result.json"), "utf8")).commit).toBe(commit)
    expect((await runLocalJob(job.id, ["test", "pass.test.ts"], false, source)).commit).toBe(nextCommit)
    await writeFile(path.join(job.directory, "running"), "another invocation")
    await expect(runLocalJob(job.id, ["test"], false, source)).rejects.toThrow()
    await rm(path.join(job.directory, "running"))
    await writeFile(path.join(job.directory, "checkout", "pass.test.ts"), "changed")
    await expect(runLocalJob(job.id, ["test"], false, source)).rejects.toThrow("changed")
    await expect(prepareLocalJob("refs/heads/fixture", "0".repeat(40), source)).rejects.toThrow()
    expect(git(["status", "--porcelain"])).toBe("?? user-work.txt")
  } finally { await rm(directory, { recursive: true, force: true }) }
}, 30_000)
