import { afterEach, describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile, utimes, stat } from "node:fs/promises"
import os from "node:os"
import path from "node:path"
import { headedProfileConfiguration } from "../profile/profile-config"
import { headedProfileTarget } from "../profile/profile-target"
import { applicationBuildIdentity, buildCacheDirectory, rustBuildIdentity } from "../src/build-identity"
import { acquireHeadedProfileLock, parseHeadedProfile, releaseHeadedProfileLock, requireBrowserBudget, ProfileCapacityDeferred, stopOwner } from "../src/profile-runner"
import { ProfileQueueTimeout } from "../src/profile-lock"
import { fileFingerprint } from "../src/file-fingerprint"
import { readWasmManifest, restoreThreadedBuild } from "../src/tf2-wasm-build"
import { prepareProfileBrowser, browserLease, browserLaunchIdentity, acquireBrowserRetirementLock, profileNodeExecutable } from "../src/profile-browser"
import { ProfilePhases } from "../profile/profile-phases"
import { repositoryRoot } from "../src/config"

const directories: string[] = []

afterEach(async () => {
  await Promise.all(directories.splice(0).map((directory) => rm(directory, { recursive: true, force: true })))
})

describe("bounded headed profile orchestration", () => {
  test("accepts exact profile identities and rejects hidden browser execution", () => {
    expect(parseHeadedProfile(["gameplay", "--headed"])).toEqual({ profile: "gameplay", fresh: false, playwright: [] })
    expect(parseHeadedProfile(["map-memory", "--headed"])).toEqual({ profile: "map-memory", fresh: false, playwright: [] })
    expect(parseHeadedProfile(["2fort", "--fresh"])).toEqual({ profile: "2fort", fresh: true, playwright: [] })
    expect(parseHeadedProfile(["2fort-bots", "--fresh", "--output", "/evidence"])).toEqual({
      profile: "2fort-bots",
      fresh: true,
      playwright: ["--output", "/evidence"],
    })
    expect(parseHeadedProfile(["2fort-visual", "--headed"])).toEqual({ profile: "2fort-visual", fresh: false, playwright: [] })
    expect(parseHeadedProfile(["sky-coherence", "--headed"])).toEqual({ profile: "sky-coherence", fresh: false, playwright: [] })
    expect(parseHeadedProfile(["upward-training-bots", "--headed"])).toEqual({ profile: "upward-training-bots", fresh: false, playwright: [] })
    expect(parseHeadedProfile(["class-switch-high-dpi", "--headed"])).toEqual({ profile: "class-switch-high-dpi", fresh: false, playwright: [] })
    expect(parseHeadedProfile(["2fort-match"])).toEqual({
      profile: "2fort-match",
      fresh: false,
      playwright: [],
    })
    expect(() => parseHeadedProfile([])).toThrow("Usage:")
    expect(() => parseHeadedProfile(["unknown"])).toThrow("Usage:")
    expect(() => parseHeadedProfile(["gameplay", "--headless"])).toThrow("never accept headless")
    expect(() => parseHeadedProfile(["gameplay", "--workers=8"])).toThrow("one bounded")
    expect(() => parseHeadedProfile(["gameplay", "--workers", "8"])).toThrow("one bounded")
    expect(() => parseHeadedProfile(["gameplay", "--ui"])).toThrow("one bounded")
  })

  test("keeps reusable development servers visibly headed and below three minutes", () => {
    const config = headedProfileConfiguration({ match: "cold-map.profile.ts", target: "jump_beef" })
    expect(config.timeout).toBe(175_000)
    expect(config.globalTimeout).toBe(175_000)
    expect(config.use?.headless).toBe(false)
    expect(config.workers).toBe(1)
    expect(Array.isArray(config.webServer)).toBe(false)
    expect((config.webServer as { reuseExistingServer: boolean; timeout: number }).reuseExistingServer).toBe(true)
    expect((config.webServer as { timeout: number }).timeout).toBe(175_000)
  })

  test("the supported Node runner can load the actual scenario without opening a browser", async () => {
    const child = Bun.spawn([profileNodeExecutable(), path.join(repositoryRoot, "node_modules/@playwright/test/cli.js"), "test", "--config=playwright.profile.config.ts", "--list"], {
      cwd: repositoryRoot,
      env: { ...Object.fromEntries(Object.entries(process.env).filter(([name]) => !name.startsWith("PROFILE_") && !name.startsWith("PLAYSRC_PROFILE_"))), PROFILE_SCENARIOS: "upward-training-bots" },
      stdout: "pipe", stderr: "pipe",
    })
    const [output, errors, code] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited])
    expect(errors).toBe("")
    expect(code).toBe(0)
    expect(output).toContain("upward-training-bots.profile.ts")
  })

  test("profiles the exact production origin without starting or substituting a development server", () => {
    const previous = process.env.PLAYSRC_PROFILE_ORIGIN
    process.env.PLAYSRC_PROFILE_ORIGIN = "https://playsrc.online"
    try {
      const config = headedProfileConfiguration({ match: "upward-training-bots.profile.ts" })
      expect(config.use?.baseURL).toBe("https://playsrc.online")
      expect(config.use?.headless).toBe(false)
      expect(config.webServer).toBeUndefined()
    } finally {
      if (previous === undefined) delete process.env.PLAYSRC_PROFILE_ORIGIN
      else process.env.PLAYSRC_PROFILE_ORIGIN = previous
    }
  })

  test("selects each exact authored profile map without a duplicate fallback authority", () => {
    expect(headedProfileTarget({})).toBe("jump_beef")
    expect(headedProfileTarget({ PROFILE_CTF_BOTS: "1" })).toBe("ctf_2fort")
    expect(headedProfileTarget({ PROFILE_2FORT_VISUAL: "1" })).toBe("ctf_2fort")
    expect(headedProfileTarget({ PROFILE_2FORT_MEMORY: "1" })).toBe("ctf_2fort")
    expect(headedProfileTarget({ PROFILE_COMBAT_IMPACTS: "1" })).toBe("ctf_2fort")
    expect(headedProfileTarget({ PROFILE_SCENARIOS: "local-practice" })).toBe("ctf_2fort")
    expect(headedProfileTarget({ PROFILE_SCENARIOS: "2fort-match" })).toBe("ctf_2fort")
    expect(headedProfileTarget({ PROFILE_MEDIC_WEAPONS: "1" })).toBe("pl_upward")
    expect(headedProfileTarget({ PROFILE_SKY_COHERENCE: "1" })).toBe("pl_upward")
    expect(headedProfileTarget({ PROFILE_SCENARIOS: "demoman" })).toBe("pl_upward")
    expect(headedProfileTarget({ PROFILE_SCENARIOS: "upward-training-bots" })).toBe("pl_upward")
    expect(headedProfileTarget({}, "pl_upward")).toBe("pl_upward")
  })

  test("reclaims dead lock owners and rejects changed release ownership", async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-profile-lock-"))
    directories.push(directory)
    const pathname = path.join(directory, "chromium-profile.lock")
    await writeFile(pathname, `${JSON.stringify({ token: "stale", pid: 999_999_999 })}\n`)
    const acquired = await acquireHeadedProfileLock(pathname, "gameplay")
    const owner = JSON.parse(await readFile(pathname, "utf8")) as { token: string; pid: number; profile: string }
    expect(owner).toMatchObject({ token: acquired.token, pid: process.pid, profile: "gameplay" })
    await expect(releaseHeadedProfileLock(pathname, "not-the-owner")).rejects.toThrow("ownership changed")
    await releaseHeadedProfileLock(pathname, acquired.token)
    await expect(readFile(pathname)).rejects.toMatchObject({ code: "ENOENT" })
    await expect(acquireHeadedProfileLock(pathname, "gameplay", 180_001)).rejects.toThrow("three-minute bound")
  })

  test("hands a live machine-wide lock directly to the next waiter without a polling delay", async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-profile-handoff-"))
    directories.push(directory)
    const pathname = path.join(directory, "chromium-profile.lock")
    const first = await acquireHeadedProfileLock(pathname, "hud")
    const waiting = acquireHeadedProfileLock(pathname, "gameplay", 2_000)
    await new Promise((resolve) => setTimeout(resolve, 20))
    const released = performance.now()
    await releaseHeadedProfileLock(pathname, first.token)
    const second = await waiting
    expect(performance.now() - released).toBeLessThan(100)
    expect(second.token).not.toBe(first.token)
    await releaseHeadedProfileLock(pathname, second.token)
  })

  test("binds shared build and source snapshots to exact checked repository inputs", async () => {
    const [build, source] = await Promise.all([rustBuildIdentity(), applicationBuildIdentity()])
    expect(build).toMatch(/^[0-9a-f]{64}$/)
    expect(source).toMatch(/^[0-9a-f]{64}$/)
    expect(await rustBuildIdentity()).toBe(build)
    expect(buildCacheDirectory("/configured/cache", build)).toBe(path.join("/configured/cache", "prepared-builds", `${process.platform}-${process.arch}`, build))
    expect(() => buildCacheDirectory("/configured/cache", "invalid")).toThrow("identity is malformed")
  })

  test("eight contenders receive FIFO handoff without simultaneous holders or starvation", async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-fifo-"))
    directories.push(directory)
    const pathname = path.join(directory, "chromium-profile.lock")
    const first = await acquireHeadedProfileLock(pathname, "first")
    const order: number[] = []
    const waiting: Promise<void>[] = []
    let active = 0
    for (let index = 0; index < 8; index++) {
      let announce!: () => void
      const queued = new Promise<void>(resolve => { announce = resolve })
      waiting.push((async () => {
        const lock = await acquireHeadedProfileLock(pathname, String(index), 2_000, { onProgress: announce })
        expect(active++).toBe(0)
        order.push(index)
        await Bun.sleep(2)
        active--
        await releaseHeadedProfileLock(pathname, lock.token)
      })())
      await queued
    }
    await releaseHeadedProfileLock(pathname, first.token)
    await Promise.all(waiting)
    expect(order).toEqual([0, 1, 2, 3, 4, 5, 6, 7])
    expect(await readdir(`${pathname}.queue`)).toEqual([])
  })

  test("the selected queue head claims release without another ticket scan", async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-legacy-handoff-"))
    directories.push(directory)
    const pathname = path.join(directory, "chromium-profile.lock")
    const first = await acquireHeadedProfileLock(pathname, "legacy")
    let announce!: () => void
    const announced = new Promise<void>(resolve => { announce = resolve })
    const next = acquireHeadedProfileLock(pathname, "queued", 1_000, { onProgress: announce })
    await announced
    const started = performance.now()
    await releaseHeadedProfileLock(pathname, first.token)
    const claimed = await next
    expect(performance.now() - started).toBeLessThan(50)
    await releaseHeadedProfileLock(pathname, claimed.token)
  })

  test("reports live holder timeout as capacity, cancels tickets, and preserves the holder", async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-live-lock-"))
    directories.push(directory)
    const pathname = path.join(directory, "chromium-profile.lock")
    const holder = await acquireHeadedProfileLock(pathname, "active-sample")
    let diagnostics: any
    try { await acquireHeadedProfileLock(pathname, "waiter", 25, { onProgress: state => { diagnostics = state } }) }
    catch (error) { expect(error).toBeInstanceOf(ProfileQueueTimeout) }
    expect(diagnostics).toMatchObject({ position: 1, holderAlive: true, holder: { pid: process.pid, profile: "active-sample" } })
    const cancellation = new AbortController()
    const waiting = acquireHeadedProfileLock(pathname, "cancel", 500, {
      signal: cancellation.signal, onProgress: () => cancellation.abort(new Error("cancelled test")),
    })
    await expect(waiting).rejects.toThrow("cancelled test")
    expect(JSON.parse(await readFile(pathname, "utf8")).token).toBe(holder.token)
    expect(await readdir(`${pathname}.queue`)).toEqual([])
    await releaseHeadedProfileLock(pathname, holder.token)
  })

  test("prunes a crashed waiter rather than stranding its successor", async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-dead-waiter-"))
    directories.push(directory)
    const pathname = path.join(directory, "chromium-profile.lock")
    await mkdir(`${pathname}.queue`)
    await writeFile(path.join(`${pathname}.queue`, "000.json"), JSON.stringify({ token: "crashed", pid: 999_999_999 }))
    const holder = await acquireHeadedProfileLock(pathname, "recovered")
    expect(await readdir(`${pathname}.queue`)).toEqual([])
    await releaseHeadedProfileLock(pathname, holder.token)
  })

  test("a genuinely exited holder is recovered across process boundaries", async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-process-lock-"))
    directories.push(directory)
    const pathname = path.join(directory, "chromium-profile.lock")
    const child = Bun.spawn([process.execPath, "-e", `import { acquireHeadedProfileLock } from ${JSON.stringify(import.meta.resolve("../src/profile-lock"))}; await acquireHeadedProfileLock(${JSON.stringify(pathname)}, "child"); console.log("acquired"); setInterval(() => {}, 1000)`], { stdout: "pipe", stderr: "pipe" })
    try {
      const reader = child.stdout.getReader()
      expect(new TextDecoder().decode((await reader.read()).value)).toContain("acquired")
      reader.releaseLock()
      child.kill("SIGKILL")
      await child.exited
      const recovered = await acquireHeadedProfileLock(pathname, "successor", 1_000)
      expect(recovered.observation.recovered).toBe(1)
      await releaseHeadedProfileLock(pathname, recovered.token)
    } finally { child.kill(); await child.exited }
  })

  test("malformed ownership is diagnosed, never stolen as stale", async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-malformed-lock-"))
    directories.push(directory)
    const pathname = path.join(directory, "chromium-profile.lock")
    await writeFile(pathname, "{")
    await utimes(pathname, new Date(0), new Date(0))
    await expect(acquireHeadedProfileLock(pathname, "safe", 100)).rejects.toThrow()
    expect(await readFile(pathname, "utf8")).toBe("{")
    expect(await readdir(`${pathname}.queue`)).toEqual([])
  })

  test("same-length generated WASM edits with restored mtime invalidate byte identity", async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-exact-wasm-"))
    directories.push(directory)
    const filename = path.join(directory, "tf2_wasm_bg.wasm")
    await writeFile(filename, "aaaa")
    const before = await stat(filename)
    const sha256 = await fileFingerprint(filename)
    await writeFile(path.join(directory, ".playsrc-build.json"), JSON.stringify({ schema: "playsrc-threaded-wasm-build-v2", identity: "exact", files: [{ name: "tf2_wasm_bg.wasm", bytes: 4, sha256 }] }))
    expect(await readWasmManifest(directory, "exact")).not.toBeNull()
    expect(await readWasmManifest(directory, "other-source")).toBeNull()
    await writeFile(filename, "bbbb")
    await utimes(filename, before.atime, before.mtime)
    expect(await fileFingerprint(filename)).not.toBe(sha256)
    expect(await readWasmManifest(directory, "exact")).toBeNull()
  })

  test("reuses only a live leased browser with exact launch and executable identity", async () => {
    expect(path.basename(profileNodeExecutable())).toMatch(/^node(?:\.exe)?$/)
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-browser-reuse-"))
    directories.push(directory)
    const filename = path.join(directory, "browser.json")
    const executable = path.join(directory, "executable")
    await writeFile(executable, "browser-binary")
    const launch = { channel: "msedge" }
    await writeFile(filename, JSON.stringify({ token: "leased", pid: process.pid, endpoint: "ws://owned", executable,
      executableSha256: await fileFingerprint(executable), identity: await browserLaunchIdentity(launch) }))
    await browserLease(filename, "leased", 10_000)
    expect(await prepareProfileBrowser(filename, launch, () => 5_000)).toMatchObject({ reused: true, token: "leased", endpoint: "ws://owned" })
  })

  test("restores exact generated builds without sharing mutable worktree inodes", async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-build-restore-"))
    directories.push(directory)
    const config = { sourceCacheDir: path.join(directory, "cache") }
    const identity = "a".repeat(64)
    const cached = path.join(buildCacheDirectory(config.sourceCacheDir, identity), "threaded-wasm")
    await mkdir(cached, { recursive: true })
    const input = path.join(cached, "tf2_wasm_bg.wasm")
    await writeFile(input, "exact")
    await writeFile(path.join(cached, ".playsrc-build.json"), JSON.stringify({ schema: "playsrc-threaded-wasm-build-v2", identity,
      files: [{ name: "tf2_wasm_bg.wasm", bytes: 5, sha256: await fileFingerprint(input) }] }))
    const output = await restoreThreadedBuild(config, identity, directory)
    expect(await readFile(output!, "utf8")).toBe("exact")
    await writeFile(output!, "wrong")
    expect(await readFile(input, "utf8")).toBe("exact")
    expect(await restoreThreadedBuild(config, "b".repeat(64), directory)).toBeNull()
    await restoreThreadedBuild(config, identity, directory)
    expect(await readFile(output!, "utf8")).toBe("exact")
  })

  test("idle browser eviction hands off to an exclusive runner without deadlock", async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-browser-retirement-"))
    directories.push(directory)
    const pathname = path.join(directory, "chromium-profile.lock")
    const browser = path.join(directory, "browser.json")
    const lock = await acquireHeadedProfileLock(pathname, "next-runner")
    const retirement = acquireBrowserRetirementLock(browser, "browser-token")
    await browserLease(browser, "browser-token", 0, lock.token)
    expect(await retirement).toBeUndefined()
    expect(JSON.parse(await readFile(pathname, "utf8")).token).toBe(lock.token)
    expect(await readdir(`${pathname}.queue`)).toEqual([])
    await releaseHeadedProfileLock(pathname, lock.token)
    const idle = await acquireBrowserRetirementLock(browser, "other-browser")
    expect(idle?.token).toBeDefined()
    await releaseHeadedProfileLock(pathname, idle!.token)
  })

  test("development retirement survives an old runner's late heartbeat without killing it", async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-owner-retirement-"))
    directories.push(directory)
    const filename = path.join(directory, "owner.json")
    const lockPath = path.join(directory, "chromium-profile.lock")
    const lock = await acquireHeadedProfileLock(lockPath, "late-heartbeat-test")
    const child = Bun.spawn([process.execPath, "-e", `import{readFile}from"node:fs/promises";console.log("ready");setInterval(async()=>{try{const lease=JSON.parse(await readFile(${JSON.stringify(`${filename}.lease`)},"utf8"));if(lease.expiresAt<=Date.now())process.exit(0)}catch{}},250)`], { stdout: "pipe", stderr: "pipe" })
    let lateHeartbeat: ReturnType<typeof setTimeout> | undefined
    try {
      const reader = child.stdout.getReader()
      await reader.read()
      reader.releaseLock()
      const metadata = { schema: "playsrc-profile-owner-v1" as const, token: "owner", pid: child.pid, identity: "source", target: "map", repository: directory, url: "http://127.0.0.1", startup: {} }
      await writeFile(filename, JSON.stringify(metadata))
      await browserLease(filename, metadata.token, 60_000)
      lateHeartbeat = setTimeout(() => { void browserLease(filename, "owner", 60_000) }, 75)
      await stopOwner(filename, metadata, 1_000)
      expect(await child.exited).toBe(0)
      await expect(readFile(filename)).rejects.toMatchObject({ code: "ENOENT" })
    } finally {
      if (lateHeartbeat) clearTimeout(lateHeartbeat)
      await releaseHeadedProfileLock(lockPath, lock.token)
      child.kill()
      await child.exited
    }
  })

  test("a stuck leased service requires endpoint proof before a PID-only interrupt", async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-stuck-service-"))
    directories.push(directory)
    const filename = path.join(directory, "owner.json")
    const child = Bun.spawn([process.execPath, "-e", 'process.once("SIGTERM",()=>process.exit(0));console.log("ready");setInterval(()=>{},1000)'], { stdout: "pipe", stderr: "pipe" })
    const lockPath = path.join(directory, "chromium-profile.lock")
    const lock = await acquireHeadedProfileLock(lockPath, "service-retirement-test")
    const metadata = { schema: "playsrc-profile-owner-v1" as const, token: "checked-service", pid: child.pid, identity: "exact", target: "map", repository: directory, url: "", startup: {} }
    let matches = false
    const server = Bun.serve({ hostname: "127.0.0.1", port: 0, fetch: () => Response.json({ ...metadata, token: matches ? metadata.token : "foreign-service" }) })
    metadata.url = server.url.toString()
    try {
      const reader = child.stdout.getReader()
      await reader.read()
      reader.releaseLock()
      await writeFile(filename, JSON.stringify(metadata))
      await browserLease(filename, metadata.token, 60_000)
      await expect(stopOwner(filename, { ...metadata, pid: -1 }, 10)).rejects.toThrow("invalid")
      await expect(stopOwner(filename, metadata, 1_200)).rejects.toThrow("remained live")
      expect(child.exitCode).toBeNull()
      matches = true
      await stopOwner(filename, metadata, 2_000)
      expect(await child.exited).toBe(0)
    } finally { await releaseHeadedProfileLock(lockPath, lock.token); server.stop(true); child.kill(); await child.exited }
  })

  test("retains failed operation duration rather than dropping its timing", async () => {
    const phases = new ProfilePhases()
    phases.enter("sample")
    await Bun.sleep(2)
    const report = phases.finish(false)
    expect(report.spans[1]).toMatchObject({ name: "sample", complete: false })
    expect(report.spans[1]!.durationMilliseconds).toBeGreaterThan(0)
  })

  test("defers an exhausted queue/build budget before launching an incomplete sample", () => {
    expect(() => requireBrowserBudget(16_000)).toThrow(ProfileCapacityDeferred)
    expect(() => requireBrowserBudget(30_000)).not.toThrow()
  })

  test("changed browser launch or binary identity cannot reuse an old process", async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-browser-invalidation-"))
    directories.push(directory)
    const filename = path.join(directory, "browser.json")
    const executable = path.join(directory, "binary")
    await writeFile(executable, "first")
    const launch = { channel: "msedge" }
    await writeFile(filename, JSON.stringify({ token: "old", pid: process.pid, endpoint: "ws://owned", executable,
      executableSha256: await fileFingerprint(executable), identity: await browserLaunchIdentity(launch) }))
    await browserLease(filename, "old", 10_000)
    await expect(prepareProfileBrowser(filename, { channel: "chromium" }, () => 0)).rejects.toThrow("still retiring")
    await browserLease(filename, "old", 10_000)
    await writeFile(executable, "other")
    await expect(prepareProfileBrowser(filename, launch, () => 0)).rejects.toThrow("still retiring")
    expect(JSON.parse(await readFile(filename, "utf8")).token).toBe("old")
  })

  test("source edits and new inputs invalidate fingerprints without process restart", async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-source-change-"))
    directories.push(directory)
    const git = async (...args: string[]) => {
      const child = Bun.spawn(["git", ...args], { cwd: directory, stdout: "ignore", stderr: "pipe" })
      if (await child.exited) throw new Error(await new Response(child.stderr).text())
    }
    await git("init")
    await writeFile(path.join(directory, "Cargo.toml"), "[workspace]\n")
    await writeFile(path.join(directory, "input.rs"), "const A: u8 = 1;\n")
    await git("add", ".")
    await git("-c", "user.name=Test", "-c", "user.email=test@example.invalid", "commit", "-m", "fixture")
    const before = await rustBuildIdentity(directory)
    const source = await applicationBuildIdentity(directory)
    await writeFile(path.join(directory, "input.rs"), "const A: u8 = 2;\n")
    expect(await rustBuildIdentity(directory)).not.toBe(before)
    expect(await applicationBuildIdentity(directory)).not.toBe(source)
    const edited = await rustBuildIdentity(directory)
    await writeFile(path.join(directory, "new.rs"), "const B: u8 = 3;\n")
    expect(await rustBuildIdentity(directory)).not.toBe(edited)
  })

  test("queued verification preserves FIFO order without replacing a live holder", async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-profile-fifo-"))
    directories.push(directory)
    const pathname = path.join(directory, "chromium-profile.lock")
    const holder = await acquireHeadedProfileLock(pathname, "holder")
    let firstPublished!: () => void, secondPublished!: () => void
    const firstReady = new Promise<void>(resolve => { firstPublished = resolve })
    const secondReady = new Promise<void>(resolve => { secondPublished = resolve })
    const first = acquireHeadedProfileLock(pathname, "first", 2000, { onProgress: firstPublished })
    await firstReady
    let secondAcquired = false
    const second = acquireHeadedProfileLock(pathname, "second", 2000, { onProgress: secondPublished }).then(value => { secondAcquired = true; return value })
    await secondReady
    expect(JSON.parse(await readFile(pathname, "utf8")).token).toBe(holder.token)
    await releaseHeadedProfileLock(pathname, holder.token)
    const firstLock = await first
    expect(secondAcquired).toBe(false)
    await releaseHeadedProfileLock(pathname, firstLock.token)
    const secondLock = await second
    await releaseHeadedProfileLock(pathname, secondLock.token)
  })

  test("FIFO admission uses shared ticket time instead of incompatible process timer epochs", async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), "playsrc-profile-clock-order-"))
    directories.push(directory)
    const pathname = path.join(directory, "chromium-profile.lock")
    const holder = await acquireHeadedProfileLock(pathname, "holder")
    await mkdir(`${pathname}.queue`, { recursive: true })
    await writeFile(path.join(`${pathname}.queue`, "999999999999999999999999-earlier.json"), JSON.stringify({
      token: "earlier", pid: process.pid, profile: "older-node-process", repository: directory,
      startedAt: new Date(Date.now() - 1000).toISOString(),
    }))
    const cancellation = new AbortController()
    let observe!: (position: number) => void
    const observed = new Promise<number>(resolve => { observe = resolve })
    const waiting = acquireHeadedProfileLock(pathname, "later-bun-process", 2000, {
      signal: cancellation.signal, onProgress: state => observe(state.position),
    })
    expect(await observed).toBe(2)
    cancellation.abort(new Error("queued-order-check-complete"))
    await expect(waiting).rejects.toThrow("queued-order-check-complete")
    expect(JSON.parse(await readFile(pathname, "utf8")).token).toBe(holder.token)
    await releaseHeadedProfileLock(pathname, holder.token)
  })
})
